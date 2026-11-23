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

fn boxed(
    draw: &mut dyn BlockEntityDrawContext,
    b: ([f32; 3], [f32; 3]),
    faces: [Option<CubeFace<'_>>; 6],
) {
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
            let r = if block == Block::LargePulley {
                24.0
            } else {
                7.0
            };
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, [0.0, 16.0], 2.0, LOG, LOG_END);
            boxed(
                draw,
                span(axis, [5.0, 11.0], [8.0 - r, 8.0 + r], [8.0 - r, 8.0 + r]),
                faces(axis, PLANKS, PLANKS),
            );
            draw.still();
            if let Some(belt) = blocks::belt(nbt)
                && (belt.partner.x, belt.partner.y, belt.partner.z) > (pos.x, pos.y, pos.z)
            {
                let other = if blocks::get_bool(nbt, "belt_big") {
                    24.0
                } else {
                    7.0
                };
                draw_belt(
                    draw,
                    pos,
                    belt.partner,
                    axis,
                    r / 16.0,
                    other / 16.0,
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
            boxed(
                draw,
                span(axis, [4.0, 12.0], [4.0, 12.0], [4.0, 12.0]),
                faces(axis, OAK, LOG_END),
            );
            wheel_cross(draw, axis);
            draw.still();
            spin(draw, pos, 0, axis, CENTER, rpm, 45.0);
            wheel_cross(draw, axis);
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

/// Four spokes and paddles in the wheel's plane (the other four are this
/// drawn again a quarter of a quarter turn on).
fn wheel_cross(draw: &mut dyn BlockEntityDrawContext, axis: Axis) {
    boxed(
        draw,
        span(axis, [6.0, 10.0], [-16.0, 32.0], [7.0, 9.0]),
        faces(axis, LOG, LOG),
    );
    boxed(
        draw,
        span(axis, [6.0, 10.0], [7.0, 9.0], [-16.0, 32.0]),
        faces(axis, LOG, LOG),
    );
    for (u, v) in [
        ([-16.0, -13.0], [2.0, 14.0]),
        ([29.0, 32.0], [2.0, 14.0]),
        ([2.0, 14.0], [-16.0, -13.0]),
        ([2.0, 14.0], [29.0, 32.0]),
    ] {
        boxed(draw, span(axis, [2.0, 14.0], u, v), faces(axis, OAK, OAK));
    }
}

/// The belt: two straight runs between the pulleys' rims (none of the
/// obstruction checks of later versions).
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
    let cross = [
        a[1] * d[2] - a[2] * d[1],
        a[2] * d[0] - a[0] * d[2],
        a[0] * d[1] - a[1] * d[0],
    ];
    let len = (cross.iter().map(|c| c * c).sum::<f32>()).sqrt().max(1e-6);
    let n = cross.map(|c| c / len);
    let at = |c: [f32; 3], s: f32, r: f32| {
        [
            c[0] + n[0] * s * r,
            c[1] + n[1] * s * r,
            c[2] + n[2] * s * r,
        ]
    };
    let hw = 2.5 / 16.0;
    let off = |p: [f32; 3], s: f32| {
        [
            p[0] + a[0] * hw * s,
            p[1] + a[1] * hw * s,
            p[2] + a[2] * hw * s,
        ]
    };
    let runs = if crossed {
        [
            (at(c1, 1.0, r1), at(c2, -1.0, r2)),
            (at(c1, -1.0, r1), at(c2, 1.0, r2)),
        ]
    } else {
        [
            (at(c1, 1.0, r1), at(c2, 1.0, r2)),
            (at(c1, -1.0, r1), at(c2, -1.0, r2)),
        ]
    };
    for (p1, p2) in runs {
        let quad = [off(p1, -1.0), off(p2, -1.0), off(p2, 1.0), off(p1, 1.0)];
        draw.quad(BELT, quad, [0.0, 0.0, 16.0, 16.0], 0xFFFFFF);
        let back = [quad[3], quad[2], quad[1], quad[0]];
        draw.quad(BELT, back, [0.0, 0.0, 16.0, 16.0], 0xFFFFFF);
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
