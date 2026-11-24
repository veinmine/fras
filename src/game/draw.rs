//! Drawing the parts (rotational network spec 10.6): each part is drawn once
//! with `spin` and `keep`, and the game turns it on the animation clock
//! until its block entity changes. Placeholder art from vanilla sprites,
//! named without a namespace (spike lesson 2).

use std::collections::HashMap;
use std::sync::Mutex;

use mod_api::{
    BlockEntityDrawContext, BlockEntityRenderBoundingBoxEvent, BlockEntitySubmitEvent, BlockState,
    ClientHost, CubeFace, HostBlockStates,
};

use super::blocks::{self, Block};
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::rot::{Shape, Spec};

const LOG: &str = "block/stripped_oak_log";
const LOG_END: &str = "block/stripped_oak_log_top";
const IRON: &str = "block/iron_block";
const PLANKS: &str = "block/spruce_planks";
const OAK: &str = "block/oak_planks";
const HOUSING: &str = "block/polished_andesite";
const BELT: &str = "block/brown_wool";

fn prop(host: &dyn ClientHost, state: BlockState, key: &str) -> Option<String> {
    let states = host.block_states();
    state
        .property(&states, key)
        .ok()
        .flatten()
        .map(str::to_owned)
}

/// The client's memory of each spinning layer: degrees per tick and phase,
/// so a part doesn't jump when its speed changes.
type Spins = HashMap<(Pos, u8), (f32, f64)>;
static SPINS: Mutex<Option<Spins>> = Mutex::new(None);

fn spin(
    draw: &mut dyn BlockEntityDrawContext,
    pos: Pos,
    layer: u8,
    axis: Axis,
    pivot: [f32; 3],
    rpm: f64,
    offset: f32,
) {
    let speed = (rpm * 0.3) as f32;
    let now = draw.animation_time();
    let phase = {
        let mut guard = SPINS.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        if map.len() > 100_000 {
            map.clear();
        }
        let entry = map.entry((pos, layer)).or_insert((speed, 0.0));
        if entry.0 != speed {
            let angle = now * entry.0 as f64 + entry.1;
            *entry = (speed, angle - now * speed as f64);
        }
        (entry.1 % 360.0) as f32
    };
    let u = axis.unit();
    draw.spin(
        [u[0] as f32, u[1] as f32, u[2] as f32],
        pivot,
        speed,
        phase + offset,
    );
}

const CENTER: [f32; 3] = [0.5, 0.5, 0.5];

/// A box given along `axis` and across it (the other two axes in order).
fn span(axis: Axis, along: [f32; 2], u: [f32; 2], v: [f32; 2]) -> ([f32; 3], [f32; 3]) {
    match axis {
        Axis::X => ([along[0], u[0], v[0]], [along[1], u[1], v[1]]),
        Axis::Y => ([u[0], along[0], v[0]], [u[1], along[1], v[1]]),
        Axis::Z => ([u[0], v[0], along[0]], [u[1], v[1], along[1]]),
    }
}

/// Faces of a box along `axis`: `end` on its two ends, `side` around.
fn faces<'a>(axis: Axis, side: &'a str, end: &'a str) -> [Option<CubeFace<'a>>; 6] {
    let s = Some(CubeFace::new(side));
    let e = Some(CubeFace::new(end));
    match axis {
        Axis::Y => [e, e, s, s, s, s],
        Axis::Z => [s, s, e, e, s, s],
        Axis::X => [s, s, s, s, e, e],
    }
}

/// Each face's texture rectangle from the corner of its sprite, at most a
/// whole sprite. A box's own UVs come from its position in the block, which
/// for a box reaching outside the block points outside its sprite (into the
/// atlas's neighbours).
fn uvs(from: [f32; 3], to: [f32; 3]) -> [[f32; 4]; 6] {
    let d = [
        (to[0] - from[0]).abs().min(16.0),
        (to[1] - from[1]).abs().min(16.0),
        (to[2] - from[2]).abs().min(16.0),
    ];
    let r = |w: f32, h: f32| [0.0, 0.0, w, h];
    [
        r(d[0], d[2]), // down
        r(d[0], d[2]), // up
        r(d[0], d[1]), // north
        r(d[0], d[1]), // south
        r(d[2], d[1]), // west
        r(d[2], d[1]), // east
    ]
}

fn boxed(
    draw: &mut dyn BlockEntityDrawContext,
    b: ([f32; 3], [f32; 3]),
    faces: [Option<CubeFace<'_>>; 6],
) {
    let uv = uvs(b.0, b.1);
    let mut faces = faces;
    for (face, uv) in faces.iter_mut().zip(uv) {
        if let Some(f) = face
            && f.uv.is_none()
        {
            *f = f.with_uv(uv);
        }
    }
    if !draw.cube(b.0, b.1, faces, 0xFFFFFF) {
        eprintln!("[fras] a sprite was refused while drawing");
    }
}

/// An axle along `axis` through the block's centre, half width `w` texels.
fn axle(
    draw: &mut dyn BlockEntityDrawContext,
    axis: Axis,
    along: [f32; 2],
    w: f32,
    side: &str,
    end: &str,
) {
    boxed(
        draw,
        span(axis, along, [8.0 - w, 8.0 + w], [8.0 - w, 8.0 + w]),
        faces(axis, side, end),
    );
}

/// The texel range of a stub on `face`, from the casing (`inner`) outward.
fn stub_range(face: Dir, inner: f32) -> [f32; 2] {
    if face.is_positive() {
        [16.0 - inner, 16.0]
    } else {
        [0.0, inner]
    }
}

/// Spinning stubs on each port of a cased part, each at its own speed.
fn stubs(draw: &mut dyn BlockEntityDrawContext, pos: Pos, spec: &Spec, rpm: f64, big: Option<Dir>) {
    for (i, (face, gain)) in spec.ports().into_iter().enumerate() {
        let axis = face.axis();
        let w = if Some(face) == big { 3.0 } else { 2.0 };
        spin(draw, pos, 10 + i as u8, axis, CENTER, rpm * gain, 0.0);
        axle(draw, axis, stub_range(face, 2.5), w, LOG, LOG_END);
        draw.still();
    }
}

pub fn submit(block: Block, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
    let nbt = &event.entity.nbt;
    let pos = Pos::from_array(event.pos);
    let state = event.state;
    let get = |key: &str| prop(host, state, key);
    let jammed = blocks::get_str(nbt, "jam").is_some_and(|j| !j.is_empty());
    let rpm = if jammed {
        0.0
    } else {
        blocks::get_f64(nbt, "rpm").unwrap_or(0.0)
    };
    let sag = blocks::get_bool(nbt, "sag");
    let spec = block.spec(&get, nbt);
    let axis = spec.as_ref().and_then(|s| s.axis()).unwrap_or(Axis::Y);
    let draw = &mut *event.draw;
    match block {
        Block::ShaftWood | Block::ShaftIron => {
            let (w, side, end) = if block == Block::ShaftIron {
                (1.5, IRON, IRON)
            } else {
                (2.0, LOG, LOG_END)
            };
            draw.push();
            if sag {
                // A sagging span hangs a little and wobbles about its line.
                if axis == Axis::Y {
                    draw.translate([1.0 / 16.0, 0.0, 0.0]);
                } else {
                    draw.translate([0.0, -1.5 / 16.0, 0.0]);
                }
            }
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], w, side, end);
            draw.still();
            draw.pop();
        }
        Block::Bearing => {
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], 1.5, IRON, IRON);
            draw.still();
            boxed(
                draw,
                span(axis, [4.0, 12.0], [3.0, 13.0], [3.0, 13.0]),
                faces(axis, HOUSING, HOUSING),
            );
            if let Some(mount) = get("mount").as_deref().and_then(Dir::from_name) {
                let range = |c: Axis| -> [f32; 2] {
                    if c == axis {
                        [5.0, 11.0]
                    } else if c == mount.axis() {
                        if mount.is_positive() {
                            [13.0, 16.0]
                        } else {
                            [0.0, 3.0]
                        }
                    } else {
                        [6.0, 10.0]
                    }
                };
                let (x, y, z) = (range(Axis::X), range(Axis::Y), range(Axis::Z));
                boxed(
                    draw,
                    ([x[0], y[0], z[0]], [x[1], y[1], z[1]]),
                    faces(axis, HOUSING, HOUSING),
                );
            }
        }
        Block::Clutch => {
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], 2.0, LOG, LOG_END);
            draw.still();
            let collar = if get("powered").as_deref() == Some("true") {
                "block/redstone_block"
            } else {
                IRON
            };
            boxed(
                draw,
                span(axis, [6.0, 10.0], [4.0, 12.0], [4.0, 12.0]),
                faces(axis, collar, collar),
            );
        }
        Block::Pulley | Block::LargePulley => {
            let r = pulley_radius(block == Block::LargePulley);
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], 2.0, LOG, LOG_END);
            draw.still();
            let sides = pulley_sides(block == Block::LargePulley);
            disc(draw, pos, axis, rpm, r, sides, [5.0, 11.0]);
            if let Some(belt) = blocks::belt(nbt)
                && (belt.partner.x, belt.partner.y, belt.partner.z) > (pos.x, pos.y, pos.z)
            {
                // The belt lies a hair outside the corners of the turning rim.
                let lie = |large: bool| (pulley_corner(large) + 0.15) / 16.0;
                draw_belt(
                    draw,
                    pos,
                    belt.partner,
                    axis,
                    lie(block == Block::LargePulley),
                    lie(blocks::get_bool(nbt, "belt_big")),
                    belt.crossed,
                );
            }
        }
        Block::BevelGearWood | Block::BevelGearIron | Block::Gearbox => {
            if let Some(spec) = &spec {
                let big = match spec.shape {
                    Shape::Gearbox { slow, .. } => Some(slow),
                    _ => None,
                };
                stubs(draw, pos, spec, rpm, big);
            }
        }
        Block::Crank => {
            let Some(Shape::Socket(face)) = spec.as_ref().map(|s| s.shape) else {
                return;
            };
            let p = face.is_positive();
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(
                draw,
                axis,
                if p { [6.0, 16.0] } else { [0.0, 10.0] },
                1.5,
                LOG,
                LOG_END,
            );
            boxed(
                draw,
                span(
                    axis,
                    if p { [3.0, 6.0] } else { [10.0, 13.0] },
                    [6.5, 15.0],
                    [6.5, 9.5],
                ),
                faces(axis, OAK, OAK),
            );
            boxed(
                draw,
                span(
                    axis,
                    if p { [0.0, 3.0] } else { [13.0, 16.0] },
                    [12.0, 15.0],
                    [6.5, 9.5],
                ),
                faces(axis, LOG, LOG_END),
            );
            draw.still();
        }
        Block::WaterWheel => {
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], 2.0, LOG, LOG_END);
            // The hub, a little wider than the spokes so their ends hide in it.
            boxed(
                draw,
                span(axis, [4.5, 11.5], [3.5, 12.5], [3.5, 12.5]),
                faces(axis, LOG, LOG_END),
            );
            wheel_arms(draw, axis, 0.0);
            draw.still();
            // The other four arms, turned 45° and a hair thinner so no faces meet
            // in one plane.
            spin(draw, pos, 1, axis, CENTER, rpm, 45.0);
            wheel_arms(draw, axis, 0.1);
            draw.still();
        }
        Block::Grinder => {
            let Some(Shape::Socket(face)) = spec.as_ref().map(|s| s.shape) else {
                return;
            };
            // The gear on its socket.
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            let range = if face.is_positive() {
                [16.0, 16.5]
            } else {
                [-0.5, 0.0]
            };
            boxed(
                draw,
                span(axis, range, [3.0, 13.0], [3.0, 13.0]),
                faces(axis, IRON, IRON),
            );
            draw.still();
            // Two rollers in the mouth turning toward each other.
            let across = match axis {
                Axis::X => Axis::Z,
                _ => Axis::X,
            };
            for (i, c) in [5.0f32, 11.0].into_iter().enumerate() {
                let mut pivot = [0.0f32; 3];
                let set = |pivot: &mut [f32; 3], a: Axis, v: f32| pivot[a as usize] = v;
                set(&mut pivot, axis, 0.5);
                set(&mut pivot, across, c / 16.0);
                set(&mut pivot, Axis::Y, 13.0 / 16.0);
                let sign = if i == 0 { 1.0 } else { -1.0 };
                spin(draw, pos, 1 + i as u8, axis, pivot, rpm * sign * 2.0, 0.0);
                let r = 2.5;
                let (lo, hi) = ([c - r, 13.0 - r], [c + r, 13.0 + r]);
                let b = match axis {
                    Axis::X => ([3.0, lo[1], lo[0]], [13.0, hi[1], hi[0]]),
                    _ => ([lo[0], lo[1], 3.0], [hi[0], hi[1], 13.0]),
                };
                boxed(draw, b, faces(axis, IRON, "block/smooth_stone"));
                draw.still();
            }
        }
        Block::Tachometer => {
            let facing = get("facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::Up);
            let needle = blocks::get_f64(nbt, "needle").unwrap_or(0.0);
            let red = blocks::get_f64(nbt, "red").unwrap_or(0.0);
            let yellow = blocks::get_f64(nbt, "yellow").unwrap_or(0.0);
            let scale = if red > 0.0 { red * 1.25 } else { 128.0 };
            let frac = (needle / scale).clamp(0.0, 1.0) as f32;
            let sprite = if red > 0.0 && needle >= red {
                "block/redstone_block"
            } else if yellow > 0.0 && needle > 0.05 && needle < yellow {
                "block/gold_block"
            } else {
                "block/black_concrete"
            };
            draw.push();
            // Turn the face-up dial to its facing (as the blockstate turns the model).
            match facing {
                Dir::Up => {}
                Dir::Down => draw.rotate_about([1.0, 0.0, 0.0], 180.0, CENTER),
                d => {
                    let y = match d {
                        Dir::South => 180.0,
                        Dir::East => -90.0,
                        Dir::West => 90.0,
                        _ => 0.0,
                    };
                    draw.rotate_about([0.0, 1.0, 0.0], y, CENTER);
                    draw.rotate_about([1.0, 0.0, 0.0], -90.0, CENTER);
                }
            }
            draw.rotate_about([0.0, 1.0, 0.0], 135.0 - 270.0 * frac, CENTER);
            let n = Some(CubeFace::new(sprite));
            boxed(draw, ([7.5, 2.0, 3.5], [8.5, 2.6, 8.5]), [n, n, n, n, n, n]);
            draw.pop();
        }
    }
    draw.keep();
}

/// Radius of a pulley's rim (to the middle of its flat sides), texels.
fn pulley_radius(large: bool) -> f32 {
    if large { 22.0 } else { 7.0 }
}

/// A pulley's number of sides.
fn pulley_sides(large: bool) -> usize {
    if large { 12 } else { 8 }
}

/// Radius of a pulley's rim to its corners, texels.
fn pulley_corner(large: bool) -> f32 {
    pulley_radius(large) / (std::f32::consts::PI / pulley_sides(large) as f32).cos()
}

/// A pulley's disc: a regular polygon (`sides`) whose flat sides sit at
/// radius `r` texels, `along` the axle, drawn as quads so its rim is a true
/// polygon rather than overlapping squares.
fn disc(
    draw: &mut dyn BlockEntityDrawContext,
    pos: Pos,
    axis: Axis,
    rpm: f64,
    r: f32,
    sides: usize,
    along: [f32; 2],
) {
    // The two axes across the axle, ordered so e1 × e2 = the axle.
    let (e1, e2) = match axis {
        Axis::X => (Axis::Y, Axis::Z),
        Axis::Y => (Axis::Z, Axis::X),
        Axis::Z => (Axis::X, Axis::Y),
    };
    let point = |u: f32, v: f32, w: f32| {
        let mut p = [0.5f32; 3];
        p[e1 as usize] += u / 16.0;
        p[e2 as usize] += v / 16.0;
        p[axis as usize] = w / 16.0;
        p
    };
    let corner = r / (std::f32::consts::PI / sides as f32).cos();
    // Start half a side round, so a flat side faces each axis.
    let ring: Vec<(f32, f32)> = (0..sides)
        .map(|k| {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / sides as f32;
            (corner * a.cos(), corner * a.sin())
        })
        .collect();
    let face_uv = [0.0, 0.0, (2.0 * r).min(16.0), (2.0 * r).min(16.0)];
    let edge = 2.0 * corner * (std::f32::consts::PI / sides as f32).sin();
    let rim_uv = [0.0, 0.0, edge.min(16.0), (along[1] - along[0]).min(16.0)];
    spin(draw, pos, 20, axis, CENTER, rpm, 0.0);
    // The two faces, as fans of quads round the centre (each over three
    // corners of the polygon); counterclockwise seen from the front.
    for k in (0..sides).step_by(2) {
        let (a, b, c) = (ring[k], ring[(k + 1) % sides], ring[(k + 2) % sides]);
        let w = along[1];
        draw.quad(
            PLANKS,
            [
                point(0.0, 0.0, w),
                point(a.0, a.1, w),
                point(b.0, b.1, w),
                point(c.0, c.1, w),
            ],
            face_uv,
            0xFFFFFF,
        );
        let w = along[0];
        draw.quad(
            PLANKS,
            [
                point(0.0, 0.0, w),
                point(c.0, c.1, w),
                point(b.0, b.1, w),
                point(a.0, a.1, w),
            ],
            face_uv,
            0xFFFFFF,
        );
    }
    // The rim.
    for k in 0..sides {
        let (a, b) = (ring[k], ring[(k + 1) % sides]);
        draw.quad(
            PLANKS,
            [
                point(a.0, a.1, along[1]),
                point(a.0, a.1, along[0]),
                point(b.0, b.1, along[0]),
                point(b.0, b.1, along[1]),
            ],
            rim_uv,
            0xFFFFFF,
        );
    }
    draw.still();
}

/// Four arms of the wheel: a spoke from the hub out into each paddle (it
/// stops inside the paddle, so no faces meet), and the paddle. `t` thins
/// them, so the two sets drawn 45° apart never share a plane.
fn wheel_arms(draw: &mut dyn BlockEntityDrawContext, axis: Axis, t: f32) {
    let spoke = |a: f32, b: f32| [a, b];
    let w = [7.0 + t, 9.0 - t];
    for (u, v) in [
        (spoke(-14.5, 4.0), w),
        (spoke(12.0, 30.5), w),
        (w, spoke(-14.5, 4.0)),
        (w, spoke(12.0, 30.5)),
    ] {
        boxed(
            draw,
            span(axis, [6.5 + t, 9.5 - t], u, v),
            faces(axis, LOG, LOG),
        );
    }
    for (u, v) in [
        ([-16.0, -13.0], [2.0, 14.0]),
        ([29.0, 32.0], [2.0, 14.0]),
        ([2.0, 14.0], [-16.0, -13.0]),
        ([2.0, 14.0], [29.0, 32.0]),
    ] {
        let u = [u[0] + t, u[1] - t];
        let v = [v[0] + t, v[1] - t];
        boxed(
            draw,
            span(axis, [2.0 + t, 14.0 - t], u, v),
            faces(axis, OAK, OAK),
        );
    }
}

/// The belt: one strip, lying on each pulley's rim round its far half and
/// running straight between them (none of the obstruction checks of later
/// versions). `r1` and `r2` are the radii it lies at, in blocks.
fn draw_belt(
    draw: &mut dyn BlockEntityDrawContext,
    pos: Pos,
    partner: Pos,
    axis: Axis,
    r1: f32,
    r2: f32,
    crossed: bool,
) {
    let a = axis.unit().map(|c| c as f32);
    let c1 = [0.5f32, 0.5, 0.5];
    let d = [
        (partner.x - pos.x) as f32,
        (partner.y - pos.y) as f32,
        (partner.z - pos.z) as f32,
    ];
    let c2 = [c1[0] + d[0], c1[1] + d[1], c1[2] + d[2]];
    let dl = d.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
    let dh = d.map(|c| c / dl);
    // Across the line between the pulleys, in their plane.
    let n = [
        a[1] * dh[2] - a[2] * dh[1],
        a[2] * dh[0] - a[0] * dh[2],
        a[0] * dh[1] - a[1] * dh[0],
    ];
    let at = |c: [f32; 3], dir: [f32; 3], r: f32| {
        [c[0] + dir[0] * r, c[1] + dir[1] * r, c[2] + dir[2] * r]
    };
    let neg = |v: [f32; 3]| v.map(|c| -c);
    let hw = 2.5 / 16.0;
    let edge = |p: [f32; 3], s: f32| {
        [
            p[0] + a[0] * hw * s,
            p[1] + a[1] * hw * s,
            p[2] + a[2] * hw * s,
        ]
    };
    // One strip from p to q: a single quad per block of length (drawn from
    // both sides), so the texture repeats instead of stretching.
    let mut strip = |p: [f32; 3], q: [f32; 3]| {
        let run = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        let length = run.iter().map(|c| c * c).sum::<f32>().sqrt();
        let pieces = length.ceil().max(1.0) as usize;
        for k in 0..pieces {
            let f0 = k as f32 / pieces as f32;
            let f1 = (k + 1) as f32 / pieces as f32;
            let lerp = |f: f32| [p[0] + run[0] * f, p[1] + run[1] * f, p[2] + run[2] * f];
            let (q1, q2) = (lerp(f0), lerp(f1));
            let uv = [0.0, 0.0, 5.0, (16.0 * length / pieces as f32).min(16.0)];
            draw.quad(
                BELT,
                [edge(q1, 1.0), edge(q1, -1.0), edge(q2, -1.0), edge(q2, 1.0)],
                uv,
                0xFFFFFF,
            );
        }
    };
    // The straight runs: open, or crossed (each run from one side to the other).
    let s2 = if crossed { -1.0 } else { 1.0 };
    strip(at(c1, n, r1), at(c2, n.map(|c| c * s2), r2));
    strip(at(c1, neg(n), r1), at(c2, neg(n).map(|c| c * s2), r2));
    // Round each pulley's far half, from one run's end to the other's.
    const STEPS: usize = 8;
    for (c, r, away) in [(c1, r1, neg(dh)), (c2, r2, dh)] {
        let point = |k: usize| {
            let t = std::f32::consts::PI * (k as f32 / STEPS as f32 - 0.5);
            let dir = [
                away[0] * t.cos() + n[0] * t.sin(),
                away[1] * t.cos() + n[1] * t.sin(),
                away[2] * t.cos() + n[2] * t.sin(),
            ];
            at(c, dir, r)
        };
        for k in 0..STEPS {
            strip(point(k), point(k + 1));
        }
    }
}

/// Culling bounds for parts that draw beyond their block.
pub fn bounds(block: Block, event: &mut BlockEntityRenderBoundingBoxEvent<'_>) {
    let b = &mut *event.bounds;
    match block {
        Block::WaterWheel | Block::LargePulley => {
            *b = [
                b[0].min(-1.5),
                b[1].min(-1.5),
                b[2].min(-1.5),
                b[3].max(2.5),
                b[4].max(2.5),
                b[5].max(2.5),
            ];
        }
        _ => {}
    }
    if matches!(block, Block::Pulley | Block::LargePulley)
        && let Some(belt) = blocks::belt(&event.entity.nbt)
    {
        let d = [
            belt.partner.x - event.pos[0],
            belt.partner.y - event.pos[1],
            belt.partner.z - event.pos[2],
        ];
        for i in 0..3 {
            b[i] = b[i].min(d[i] as f32 - 1.5);
            b[i + 3] = b[i + 3].max(d[i] as f32 + 2.5);
        }
    }
}
