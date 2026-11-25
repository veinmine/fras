//! The formed boiler and tank model (multiblocks spec 6.1), built from quads
//! by a model loader: each block draws only its outer faces, and the edges
//! that run along the multiblock's long axis are rounded (three facets), so
//! the whole reads as one drum or barrel. Every vertex lies inside the
//! block and every texture coordinate inside its sprite; no two quads share
//! a plane.
//!
//! The model files (one per joined-faces mask and axis, written by
//! `tools/gen_content.py`) name the loader `fras:shell` with `mask` (bit
//! `Dir::index` set for each joined face) and `axis`.

use mod_api::{Direction, ModelLoaderQuad, Tag};

use crate::sim::geom::{Axis, Dir};

pub const LOADER: &str = "fras:shell";

/// The rounded edges' radius (block units) and their facets.
const R: f32 = 5.0 / 16.0;
const FACETS: usize = 3;

type V3 = [f32; 3];

fn unit(d: Dir) -> V3 {
    let s = d.step();
    [s[0] as f32, s[1] as f32, s[2] as f32]
}

/// A face's two in-plane directions (positive axes), ordered so that
/// u × v points out of the face.
fn uv_axes(d: Dir) -> (Axis, Axis) {
    match d {
        Dir::East => (Axis::Y, Axis::Z),
        Dir::West => (Axis::Z, Axis::Y),
        Dir::Up => (Axis::Z, Axis::X),
        Dir::Down => (Axis::X, Axis::Z),
        Dir::South => (Axis::X, Axis::Y),
        Dir::North => (Axis::Y, Axis::X),
    }
}

fn axis_dir(a: Axis, positive: bool) -> Dir {
    if positive { a.positive() } else { a.negative() }
}

fn direction(d: Dir) -> Direction {
    super::direction(d)
}

/// Texture coordinates for a point on a face (always inside 0..1).
fn tex(p: V3, d: Dir) -> [f32; 2] {
    match d.axis() {
        Axis::X => [p[2], 1.0 - p[1]],
        Axis::Z => [p[0], 1.0 - p[1]],
        Axis::Y => [p[0], p[2]],
    }
}

fn quad(
    positions: [V3; 4],
    uvs: [[f32; 2]; 4],
    texture: &str,
    cull: Option<Dir>,
) -> ModelLoaderQuad {
    ModelLoaderQuad {
        positions,
        uvs,
        texture: texture.to_string(),
        cull: cull.map(direction),
        tint_index: -1,
        shade: true,
    }
}

/// A convex polygon (counterclockwise from outside) as a fan of quads.
fn fan(out: &mut Vec<ModelLoaderQuad>, poly: &[V3], face: Dir, texture: &str, cull: Option<Dir>) {
    if poly.len() < 3 {
        return;
    }
    let mut i = 1;
    while i + 1 < poly.len() {
        let c = if i + 2 < poly.len() {
            poly[i + 2]
        } else {
            poly[i + 1]
        };
        let pts = [poly[0], poly[i], poly[i + 1], c];
        out.push(quad(pts, pts.map(|p| tex(p, face)), texture, cull));
        i += 2;
    }
}

/// A convex end cap cut into full-width strips across `va` (a fan from one
/// corner leaves thin wedges that shade unevenly and show seams).
fn strips(
    out: &mut Vec<ModelLoaderQuad>,
    poly: &[V3],
    ua: Axis,
    va: Axis,
    point: &dyn Fn(f32, f32) -> V3,
    face: Dir,
    texture: &str,
) {
    let uv: Vec<(f32, f32)> = poly
        .iter()
        .map(|p| (p[ua as usize], p[va as usize]))
        .collect();
    let mut levels: Vec<f32> = uv.iter().map(|&(_, v)| v).collect();
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    // The cap's extent in u at height v.
    let span = |v: f32| {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for i in 0..uv.len() {
            let (a, b) = (uv[i], uv[(i + 1) % uv.len()]);
            let (vmin, vmax) = (a.1.min(b.1), a.1.max(b.1));
            if v < vmin - 1e-5 || v > vmax + 1e-5 {
                continue;
            }
            let us: Vec<f32> = if (b.1 - a.1).abs() < 1e-6 {
                vec![a.0, b.0]
            } else {
                let t = ((v - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
                vec![a.0 + t * (b.0 - a.0)]
            };
            for u in us {
                lo = lo.min(u);
                hi = hi.max(u);
            }
        }
        (lo, hi)
    };
    for w in levels.windows(2) {
        let (v0, v1) = (w[0], w[1]);
        let (a0, b0) = span(v0);
        let (a1, b1) = span(v1);
        let pts = [point(a0, v0), point(b0, v0), point(b1, v1), point(a1, v1)];
        out.push(quad(pts, pts.map(|p| tex(p, face)), texture, Some(face)));
    }
}

/// Whether the block edge between faces `a` and `b` is rounded: both outer,
/// and the edge runs along the long axis.
fn rounded(mask: u8, long: Axis, a: Dir, b: Dir) -> bool {
    let open = |d: Dir| mask & (1 << d.index()) == 0;
    a.axis() != long && b.axis() != long && a.axis() != b.axis() && open(a) && open(b)
}

/// The arc round the edge between faces `a` and `b` at position `along` on
/// the long axis: from the point on face `a` to the point on face `b`.
fn arc(long: Axis, a: Dir, b: Dir, along: f32) -> Vec<V3> {
    let (ua, ub) = (unit(a), unit(b));
    // The edge's corner in the cross-section, and the arc's centre inside it.
    let mut corner = [0.0f32; 3];
    corner[a.axis() as usize] = if a.is_positive() { 1.0 } else { 0.0 };
    corner[b.axis() as usize] = if b.is_positive() { 1.0 } else { 0.0 };
    corner[long as usize] = along;
    let centre = [
        corner[0] - R * (ua[0] + ub[0]),
        corner[1] - R * (ua[1] + ub[1]),
        corner[2] - R * (ua[2] + ub[2]),
    ];
    (0..=FACETS)
        .map(|i| {
            let t = std::f32::consts::FRAC_PI_2 * i as f32 / FACETS as f32;
            let (c, s) = (t.cos(), t.sin());
            [
                centre[0] + R * (ua[0] * c + ub[0] * s),
                centre[1] + R * (ua[1] * c + ub[1] * s),
                centre[2] + R * (ua[2] * c + ub[2] * s),
            ]
        })
        .collect()
}

/// The quads of one block of a formed multiblock: `mask` has bit
/// `Dir::index` set for each face joined to a block of the same multiblock.
pub fn quads(mask: u8, long: Axis, side: &str, end: &str) -> Vec<ModelLoaderQuad> {
    let mut out = Vec::new();
    let open = |d: Dir| mask & (1 << d.index()) == 0;
    for d in Dir::ALL {
        if !open(d) {
            continue;
        }
        let (ua, va) = uv_axes(d);
        let n = d.axis();
        let base = if d.is_positive() { 1.0 } else { 0.0 };
        let point = |u: f32, v: f32| {
            let mut p = [0.0f32; 3];
            p[n as usize] = base;
            p[ua as usize] = u;
            p[va as usize] = v;
            p
        };
        // The face's four corners counterclockwise in (u, v), each with the
        // faces across its incoming and outgoing edges.
        let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        // The face across each edge: v=0, u=1, v=1, u=0 (in walking order).
        let edge_face = [
            axis_dir(va, false),
            axis_dir(ua, true),
            axis_dir(va, true),
            axis_dir(ua, false),
        ];
        if n == long {
            // An end: corners where two rounded edges meet are rounded off.
            let mut poly = Vec::new();
            for (k, &(u, v)) in corners.iter().enumerate() {
                let incoming = edge_face[(k + 3) % 4];
                let outgoing = edge_face[k];
                if rounded(mask, long, incoming, outgoing) {
                    let mut pts = arc(long, incoming, outgoing, base);
                    // The arc runs from the incoming edge's face to the outgoing's.
                    let first = pts[0];
                    let on_incoming = (first[incoming.axis() as usize]
                        - if incoming.is_positive() { 1.0 } else { 0.0 })
                    .abs()
                        < 1e-6;
                    if !on_incoming {
                        pts.reverse();
                    }
                    poly.extend(pts);
                } else {
                    poly.push(point(u, v));
                }
            }
            strips(&mut out, &poly, ua, va, &point, d, end);
        } else {
            // A side: shrunk away from its rounded edges.
            let cut = |f: Dir| if rounded(mask, long, d, f) { R } else { 0.0 };
            let u0 = cut(axis_dir(ua, false));
            let u1 = 1.0 - cut(axis_dir(ua, true));
            let v0 = cut(axis_dir(va, false));
            let v1 = 1.0 - cut(axis_dir(va, true));
            let poly = [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)];
            let full = u0 == 0.0 && v0 == 0.0 && u1 == 1.0 && v1 == 1.0;
            fan(&mut out, &poly, d, side, if full { Some(d) } else { None });
        }
    }
    // The rounded edges: strips along the long axis.
    let across: Vec<Dir> = Dir::ALL.into_iter().filter(|d| d.axis() != long).collect();
    for (i, &a) in across.iter().enumerate() {
        for &b in &across[i + 1..] {
            if !rounded(mask, long, a, b) {
                continue;
            }
            let lo = arc(long, a, b, 0.0);
            let hi = arc(long, a, b, 1.0);
            for k in 0..FACETS {
                // Each corner with its index round the arc.
                let mut pts = [
                    (lo[k], k),
                    (lo[k + 1], k + 1),
                    (hi[k + 1], k + 1),
                    (hi[k], k),
                ];
                // Outward: the strip's normal must point away from the arc's centre.
                let nrm = cross(sub(pts[1].0, pts[0].0), sub(pts[3].0, pts[0].0));
                let (na, nb) = (unit(a), unit(b));
                let out_dir = [na[0] + nb[0], na[1] + nb[1], na[2] + nb[2]];
                if dot(nrm, out_dir) < 0.0 {
                    pts = [pts[0], pts[3], pts[2], pts[1]];
                }
                let positions = pts.map(|(p, _)| p);
                let uvs = pts.map(|(p, j)| [p[long as usize], j as f32 / FACETS as f32]);
                out.push(quad(positions, uvs, side, None));
            }
        }
    }
    out
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Read a model's loader config: its joined-faces mask and long axis.
pub fn config(tag: &Tag) -> Option<(u8, Axis)> {
    let Tag::Compound(c) = tag else {
        return None;
    };
    let mask = match c.get(&"mask".into())? {
        Tag::Int(v) => *v,
        Tag::Byte(v) => *v as i32,
        Tag::Short(v) => *v as i32,
        Tag::Long(v) => *v as i32,
        Tag::Double(b) => f64::from_bits(*b) as i32,
        Tag::Float(b) => f32::from_bits(*b) as i32,
        _ => return None,
    };
    let axis = match c.get(&"axis".into()) {
        Some(Tag::String(s)) => s.to_string().ok().as_deref().and_then(Axis::from_name),
        _ => None,
    }
    .unwrap_or(Axis::Y);
    Some(((mask & 63) as u8, axis))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal(q: &ModelLoaderQuad) -> V3 {
        let p = q.positions;
        cross(sub(p[1], p[0]), sub(p[2], p[0]))
    }

    #[test]
    fn every_vertex_and_uv_stays_in_the_block_and_sprite() {
        for axis in Axis::ALL {
            for mask in 0..64u8 {
                for q in quads(mask, axis, "side", "end") {
                    for p in q.positions {
                        assert!(p.iter().all(|c| (-1e-5..=1.0 + 1e-5).contains(c)), "{p:?}");
                    }
                    for uv in q.uvs {
                        assert!(
                            uv.iter().all(|c| (-1e-5..=1.0 + 1e-5).contains(c)),
                            "{uv:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn faces_point_outward() {
        // A lone block standing on Y: every quad faces away from the centre.
        for axis in Axis::ALL {
            for q in quads(0, axis, "side", "end") {
                let n = normal(&q);
                let c = q.positions.iter().fold([0.0; 3], |a, p| {
                    [a[0] + p[0] / 4.0, a[1] + p[1] / 4.0, a[2] + p[2] / 4.0]
                });
                let out = sub(c, [0.5, 0.5, 0.5]);
                assert!(dot(n, out) > 0.0, "{axis:?} {:?}", q.positions);
            }
        }
    }

    #[test]
    fn inner_faces_are_not_drawn() {
        // A middle block of a column along Y: no top or bottom.
        let mask = (1 << Dir::Up.index()) | (1 << Dir::Down.index());
        for q in quads(mask, Axis::Y, "side", "end") {
            assert!(q.texture == "side");
        }
        // Fully enclosed: nothing.
        assert!(quads(63, Axis::Y, "side", "end").is_empty());
    }

    #[test]
    fn no_two_quads_share_a_plane_and_overlap() {
        // Every quad's centre is distinct (a cheap check for doubled faces).
        for axis in Axis::ALL {
            for mask in 0..64u8 {
                let qs = quads(mask, axis, "side", "end");
                let mut centres: Vec<[i64; 3]> = qs
                    .iter()
                    .map(|q| {
                        let c = q
                            .positions
                            .iter()
                            .fold([0.0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
                        c.map(|v| (v * 1000.0).round() as i64)
                    })
                    .collect();
                let n = centres.len();
                centres.sort();
                centres.dedup();
                assert_eq!(centres.len(), n, "mask {mask} {axis:?}");
            }
        }
    }

    #[test]
    fn no_quad_is_a_sliver() {
        // Every quad has four distinct corners and some width (no wedges
        // meeting in a point, as a fan from one corner gives).
        for axis in Axis::ALL {
            for mask in 0..64u8 {
                for q in quads(mask, axis, "side", "end") {
                    let p = q.positions;
                    for i in 0..4 {
                        for j in i + 1..4 {
                            let d = sub(p[i], p[j]);
                            assert!(dot(d, d) > 1e-8, "{mask} {axis:?} {p:?}");
                        }
                    }
                    let edges = (0..4).map(|i| {
                        let d = sub(p[(i + 1) % 4], p[i]);
                        dot(d, d).sqrt()
                    });
                    let longest = edges.fold(0.0f32, f32::max);
                    let area = (dot(normal(&q), normal(&q))).sqrt();
                    assert!(area / longest > 0.02, "{mask} {axis:?} {p:?}");
                }
            }
        }
    }

    #[test]
    fn end_caps_are_strips_not_wedges() {
        for axis in Axis::ALL {
            for mask in 0..64u8 {
                for q in quads(mask, axis, "side", "end") {
                    if q.texture != "end" {
                        continue;
                    }
                    let p = q.positions;
                    for k in 0..4 {
                        let d = sub(p[(k + 1) % 4], p[k]);
                        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                        assert!(
                            l > 1e-4,
                            "a cap quad has a corner twice (mask {mask}, {axis:?})"
                        );
                    }
                }
            }
        }
    }
}
