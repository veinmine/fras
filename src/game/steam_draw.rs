//! Drawing the steam parts that move or show a reading (steam and heat spec
//! 9.3 and 10, multiblocks spec 6.3): the flywheel (drawn once, turned by
//! `spin`), the connecting rod (drawn every frame from the flywheel's
//! angle), the manual valve's handwheel, the pressure gauge and the sight
//! glass. Round things are smooth polygons built from quads.

use mod_api::{BlockEntityDrawContext, BlockEntitySubmitEvent, ClientHost, CubeFace};

use super::blocks::{self, Block};
use super::draw::{CENTER, angle, axle, boxed, faces, iron, prop, span, spin};
use crate::sim::geom::{Axis, Dir, Pos};

const DARK: &str = "block/anvil";
const STEEL: &str = "block/smooth_stone";
const BRASS: &str = "block/gold_block";
const WHITE: &str = "block/white_concrete";

type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
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

fn norm(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    scale(a, 1.0 / l)
}

fn unit(axis: Axis) -> V3 {
    let u = axis.unit();
    [u[0] as f32, u[1] as f32, u[2] as f32]
}

/// The two directions across `axis`, ordered so e1 × e2 = the axis.
fn across(axis: Axis) -> (V3, V3) {
    let (a, b) = match axis {
        Axis::X => (Axis::Y, Axis::Z),
        Axis::Y => (Axis::Z, Axis::X),
        Axis::Z => (Axis::X, Axis::Y),
    };
    (unit(a), unit(b))
}

/// A quad's texture rectangle, `w` by `h` texels, never past the sprite.
fn uv(w: f32, h: f32) -> [f32; 4] {
    [0.0, 0.0, w.clamp(0.5, 16.0), h.clamp(0.5, 16.0)]
}

fn len(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// A box from `a` to `b` (block units) whose cross-section is `w` by `t`
/// texels, `w` along `side`: six quads, counterclockwise from outside.
fn beam(
    draw: &mut dyn BlockEntityDrawContext,
    sprite: &str,
    a: V3,
    b: V3,
    side: V3,
    w: f32,
    t: f32,
) {
    let d = norm(sub(b, a));
    let s = norm(side);
    let n = norm(cross(d, s));
    let (hw, ht) = (w / 32.0, t / 32.0);
    let c = |p: V3, i: f32, j: f32| add(add(p, scale(s, hw * i)), scale(n, ht * j));
    let a0 = [
        c(a, -1.0, -1.0),
        c(a, 1.0, -1.0),
        c(a, 1.0, 1.0),
        c(a, -1.0, 1.0),
    ];
    let b0 = [
        c(b, -1.0, -1.0),
        c(b, 1.0, -1.0),
        c(b, 1.0, 1.0),
        c(b, -1.0, 1.0),
    ];
    let l = len(sub(b, a)) * 16.0;
    // Four sides along the beam.
    for k in 0..4 {
        let k2 = (k + 1) % 4;
        let wide = if k % 2 == 0 { w } else { t };
        draw.quad(
            sprite,
            [a0[k], a0[k2], b0[k2], b0[k]],
            uv(wide, l),
            0xFFFFFF,
        );
    }
    // The two ends.
    draw.quad(sprite, [a0[3], a0[2], a0[1], a0[0]], uv(w, t), 0xFFFFFF);
    draw.quad(sprite, [b0[0], b0[1], b0[2], b0[3]], uv(w, t), 0xFFFFFF);
}

/// A ring (or, with `inner` 0, a disc) of `sides` sides about `center`
/// across `axis`, from `w0` to `w1` along it (block units); radii in texels.
#[allow(clippy::too_many_arguments)]
fn ring(
    draw: &mut dyn BlockEntityDrawContext,
    sprite: &str,
    center: V3,
    axis: Axis,
    inner: f32,
    outer: f32,
    w0: f32,
    w1: f32,
    sides: usize,
) {
    let (e1, e2) = across(axis);
    let a = unit(axis);
    let at = |r: f32, k: usize, w: f32| {
        let t = std::f32::consts::TAU * k as f32 / sides as f32;
        add(
            add(center, scale(a, w)),
            add(scale(e1, r / 16.0 * t.cos()), scale(e2, r / 16.0 * t.sin())),
        )
    };
    let edge = |r: f32| 2.0 * r * (std::f32::consts::PI / sides as f32).sin();
    let depth = (w1 - w0).abs() * 16.0;
    for k in 0..sides {
        let k2 = (k + 1) % sides;
        // Outer rim.
        draw.quad(
            sprite,
            [
                at(outer, k, w1),
                at(outer, k, w0),
                at(outer, k2, w0),
                at(outer, k2, w1),
            ],
            uv(edge(outer), depth),
            0xFFFFFF,
        );
        if inner > 0.0 {
            // Inner rim, facing in.
            draw.quad(
                sprite,
                [
                    at(inner, k2, w1),
                    at(inner, k2, w0),
                    at(inner, k, w0),
                    at(inner, k, w1),
                ],
                uv(edge(inner), depth),
                0xFFFFFF,
            );
        }
        let band = outer - inner;
        // Front (toward +axis) and back faces.
        draw.quad(
            sprite,
            [
                at(inner, k, w1),
                at(outer, k, w1),
                at(outer, k2, w1),
                at(inner, k2, w1),
            ],
            uv(edge(outer), band),
            0xFFFFFF,
        );
        draw.quad(
            sprite,
            [
                at(inner, k2, w0),
                at(outer, k2, w0),
                at(outer, k, w0),
                at(inner, k, w0),
            ],
            uv(edge(outer), band),
            0xFFFFFF,
        );
    }
}

fn facing(host: &dyn ClientHost, state: mod_api::BlockState, default: Dir) -> Dir {
    prop(host, state, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(default)
}

/// The flywheel's radius to its crank pin, block units, and the rod's plane
/// offset along the axle.
pub const PIN_R: f32 = 0.5;
// The rod's plane, 4.5 texels off the wheel's centre: clear of the rim
// (2 texels deep) with the crosshead's half-width to spare.
const ROD_W: f32 = 12.5 / 16.0;

pub fn submit(block: Block, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
    let nbt = &event.entity.nbt;
    let pos = Pos::from_array(event.pos);
    let state = event.state;
    super::draw::set_rust(host, state);
    let draw = &mut *event.draw;
    match block {
        Block::Flywheel => {
            let axis = prop(host, state, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X);
            let jammed = blocks::get_str(nbt, "jam").is_some_and(|j| !j.is_empty());
            let rpm = if jammed {
                0.0
            } else {
                blocks::get_f64(nbt, "rpm").unwrap_or(0.0)
            };
            flywheel(draw, pos, axis, rpm);
            draw.keep();
        }
        Block::SteamCylinder => {
            // Every frame: the rod follows the flywheel two blocks ahead.
            let f = facing(host, state, Dir::North);
            if prop(host, state, "formed").as_deref() == Some("true") {
                rod(draw, pos, f);
            }
        }
        Block::ManualValve => {
            let axis = prop(host, state, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X);
            let opening = blocks::get_i32(nbt, "opening").unwrap_or(10).clamp(0, 10);
            handwheel(draw, axis, opening);
            draw.keep();
        }
        Block::PressureGauge => {
            let f = facing(host, state, Dir::Up);
            let bar = blocks::get_f64(nbt, "bar").unwrap_or(0.0);
            let red = blocks::get_f64(nbt, "red").unwrap_or(0.0);
            gauge(draw, f, bar, red);
            draw.keep();
        }
        Block::SightGlass => {
            let f = facing(host, state, Dir::North);
            let level = blocks::get_f64(nbt, "level").unwrap_or(0.0);
            let warn = blocks::get_i32(nbt, "lwarn").unwrap_or(0);
            sight_glass(draw, f, level, warn);
            draw.keep();
        }
        Block::Governor => {
            let socket = prop(host, state, "socket")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::Up);
            let jammed = blocks::get_str(nbt, "jam").is_some_and(|j| !j.is_empty());
            let rpm = if jammed {
                0.0
            } else {
                blocks::get_f64(nbt, "rpm").unwrap_or(0.0)
            };
            let ball = blocks::get_i32(nbt, "ball").unwrap_or(0).clamp(0, 7);
            flyballs(draw, pos, socket, rpm, ball);
            draw.keep();
        }
        Block::Bellows => {
            let socket = facing(host, state, Dir::North).opposite();
            let jammed = blocks::get_str(nbt, "jam").is_some_and(|j| !j.is_empty());
            let rpm = if jammed {
                0.0
            } else {
                blocks::get_f64(nbt, "rpm").unwrap_or(0.0)
            };
            let axis = socket.axis();
            spin(draw, pos, 1, axis, CENTER, rpm, 0.0);
            let range = if socket.is_positive() {
                [12.0, 16.0]
            } else {
                [0.0, 4.0]
            };
            axle(
                draw,
                axis,
                range,
                2.0,
                "block/stripped_oak_log",
                "block/stripped_oak_log_top",
            );
            draw.keep();
        }
        _ => {}
    }
}

/// The governor's spindle on its socket side and the two flyballs, out by
/// `step` (0 to 7), turning with the shaft.
fn flyballs(draw: &mut dyn BlockEntityDrawContext, pos: Pos, socket: Dir, rpm: f64, step: i32) {
    let axis = socket.axis();
    let out = |a: f32, b: f32| {
        if socket.is_positive() {
            [a, b]
        } else {
            [16.0 - b, 16.0 - a]
        }
    };
    spin(draw, pos, 1, axis, CENTER, rpm, 0.0);
    // Spindle from the body to the socket face.
    axle(draw, axis, out(11.0, 16.0), 1.0, STEEL, STEEL);
    // Balls: out from the spindle by 1.5 to 5 texels, near the socket end.
    let r = 1.5 + step as f32 * 0.5;
    let along = out(12.0, 14.5);
    for sign in [-1.0f32, 1.0] {
        let c = 8.0 + sign * (r + 1.25);
        let arm_lo = if sign > 0.0 { 8.0 } else { c };
        let arm_hi = if sign > 0.0 { c } else { 8.0 };
        boxed(
            draw,
            span(axis, out(13.0, 13.5), [arm_lo, arm_hi], [7.75, 8.25]),
            faces(axis, iron(), iron()),
        );
        boxed(
            draw,
            span(axis, along, [c - 1.25, c + 1.25], [6.75, 9.25]),
            faces(axis, BRASS, BRASS),
        );
    }
    draw.still();
}

/// The 3×3 wheel: a smooth 32-sided rim, six spokes, the hub and its axle,
/// and the crank pin standing out of the wheel's +axis face.
fn flywheel(draw: &mut dyn BlockEntityDrawContext, pos: Pos, axis: Axis, rpm: f64) {
    let a = unit(axis);
    let (e1, e2) = across(axis);
    spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
    axle(draw, axis, [0.0, 16.0], 2.0, iron(), iron());
    // The rim: 24 texels out, 4 deep, 3 thick across.
    // Dark iron, rusted or painted with the part (rust spec 7.3).
    let dark = super::draw::rusted_or(DARK);
    ring(
        draw,
        dark,
        CENTER,
        axis,
        20.0,
        23.5,
        -2.0 / 16.0,
        2.0 / 16.0,
        32,
    );
    // The hub.
    ring(
        draw,
        iron(),
        CENTER,
        axis,
        0.0,
        4.0,
        -3.0 / 16.0,
        3.0 / 16.0,
        16,
    );
    // Six spokes from the hub into the rim (they end inside it, no faces meet).
    for k in 0..6 {
        let t = std::f32::consts::TAU * (k as f32 + 0.5) / 6.0;
        let dir = add(scale(e1, t.cos()), scale(e2, t.sin()));
        let side = add(scale(e1, -t.sin()), scale(e2, t.cos()));
        let from = add(CENTER, scale(dir, 3.0 / 16.0));
        let to = add(CENTER, scale(dir, 21.0 / 16.0));
        beam(draw, dark, from, to, side, 2.5, 2.5);
    }
    // The crank pin, at angle 0 on e1, standing out to the rod's plane.
    let pin = add(CENTER, scale(e1, PIN_R));
    beam(
        draw,
        STEEL,
        add(pin, scale(a, 1.5 / 16.0)),
        add(pin, scale(a, ROD_W - 0.5 + 1.5 / 16.0)),
        e2,
        2.0,
        2.0,
    );
    // The crank web, just off the hub's face, out to the pin.
    beam(
        draw,
        iron(),
        add(CENTER, scale(a, 3.6 / 16.0)),
        add(pin, scale(a, 3.6 / 16.0)),
        e2,
        3.0,
        1.0,
    );
    draw.still();
}

/// The connecting rod and piston rod of a cylinder facing `f`: from the
/// cylinder's front through a crosshead to the flywheel's crank pin.
fn rod(draw: &mut dyn BlockEntityDrawContext, pos: Pos, f: Dir) {
    let hub = pos.offset_by(f, 2);
    let axis = match f.axis() {
        Axis::X => Axis::Z,
        _ => Axis::X,
    };
    let a = unit(axis);
    let (e1, e2) = across(axis);
    let theta = angle(hub, 0, draw.animation_time())
        .unwrap_or(0.0)
        .to_radians() as f32;
    // Hub centre and pin in this block's frame.
    let d = f.step();
    let hub_c = add(
        CENTER,
        [2.0 * d[0] as f32, 2.0 * d[1] as f32, 2.0 * d[2] as f32],
    );
    let plane = scale(a, ROD_W - 0.5);
    let pin = add(
        add(hub_c, plane),
        add(
            scale(e1, PIN_R * theta.cos()),
            scale(e2, PIN_R * theta.sin()),
        ),
    );
    // The crosshead slides on the line from the cylinder to the hub.
    let toward = [d[0] as f32, d[1] as f32, d[2] as f32];
    let rel = sub(pin, add(hub_c, plane));
    let along = -(rel[0] * toward[0] + rel[1] * toward[1] + rel[2] * toward[2]);
    let off = len(sub(rel, scale(toward, -along)));
    const ROD: f32 = 1.0;
    let x = along + (ROD * ROD - off * off).max(0.0).sqrt();
    let cross_head = sub(add(hub_c, plane), scale(toward, x));
    let front = add(add(CENTER, plane), scale(toward, 0.5));
    // Piston rod from the cylinder's front gland to the crosshead.
    beam(draw, STEEL, front, cross_head, [0.0, 1.0, 0.0], 1.5, 1.5);
    // The crosshead block.
    let up = [0.0, 1.0, 0.0];
    beam(
        draw,
        iron(),
        sub(cross_head, scale(toward, 1.5 / 16.0)),
        add(cross_head, scale(toward, 1.5 / 16.0)),
        up,
        3.0,
        3.0,
    );
    // The connecting rod.
    beam(draw, iron(), cross_head, pin, a, 2.0, 2.5);
}

/// The handwheel on top of a manual valve, turned by its opening.
fn handwheel(draw: &mut dyn BlockEntityDrawContext, axis: Axis, opening: i32) {
    // The stem points up, or along +X when the valve itself is vertical.
    let stem = if axis == Axis::Y { Axis::X } else { Axis::Y };
    let s = unit(stem);
    let top = add(CENTER, scale(s, 6.0 / 16.0));
    draw.push();
    draw.rotate_about(s, opening as f32 * 27.0, CENTER);
    beam(
        draw,
        STEEL,
        add(CENTER, scale(s, 3.0 / 16.0)),
        top,
        unit(axis),
        2.0,
        2.0,
    );
    ring(
        draw,
        "block/red_concrete",
        top,
        stem,
        4.5,
        6.0,
        -0.75 / 16.0,
        0.75 / 16.0,
        20,
    );
    let (e1, e2) = across(stem);
    for k in 0..3 {
        let t = std::f32::consts::TAU * k as f32 / 3.0;
        let dir = add(scale(e1, t.cos()), scale(e2, t.sin()));
        let side = add(scale(e1, -t.sin()), scale(e2, t.cos()));
        beam(
            draw,
            "block/red_concrete",
            top,
            add(top, scale(dir, 5.0 / 16.0)),
            side,
            1.0,
            1.0,
        );
    }
    draw.pop();
}

/// Turn a face-up drawing to `f` (as the blockstates turn the models).
fn face_to(draw: &mut dyn BlockEntityDrawContext, f: Dir) {
    match f {
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
}

/// The pressure gauge's dial: 0 to 8 bar over 270°, the needle red above
/// the red line and yellow from 90% of it (spec 2, 10.1).
fn gauge(draw: &mut dyn BlockEntityDrawContext, f: Dir, bar: f64, red: f64) {
    draw.push();
    face_to(draw, f);
    // The red arc of the dial from the red line up.
    if red > 0.0 {
        let frac = (red / 8.0).clamp(0.0, 1.0) as f32;
        let a0 = 135.0 - 270.0 * frac;
        let mark = Some(CubeFace::new("block/redstone_block"));
        draw.push();
        draw.rotate_about([0.0, 1.0, 0.0], a0, CENTER);
        boxed(
            draw,
            ([7.6, 2.05, 2.5], [8.4, 2.3, 4.0]),
            [mark, mark, mark, mark, mark, mark],
        );
        draw.pop();
    }
    let frac = (bar / 8.0).clamp(0.0, 1.0) as f32;
    let sprite = if red > 0.0 && bar > red {
        "block/redstone_block"
    } else if red > 0.0 && bar >= red * 0.9 {
        BRASS
    } else {
        "block/black_concrete"
    };
    draw.rotate_about([0.0, 1.0, 0.0], 135.0 - 270.0 * frac, CENTER);
    let n = Some(CubeFace::new(sprite));
    boxed(draw, ([7.5, 2.3, 3.5], [8.5, 2.8, 8.5]), [n, n, n, n, n, n]);
    draw.pop();
}

/// The water column in a sight glass on face `f`: its height the level,
/// with a yellow or red frame when low on a boiler (spec 2, 7.4).
fn sight_glass(draw: &mut dyn BlockEntityDrawContext, f: Dir, level: f64, warn: i32) {
    draw.push();
    // Draw as if on the north face, then turn.
    let y = match f {
        Dir::South => 180.0,
        Dir::East => -90.0,
        Dir::West => 90.0,
        _ => 0.0,
    };
    draw.rotate_about([0.0, 1.0, 0.0], y, CENTER);
    let frame = match warn {
        2 => Some("block/redstone_block"),
        1 => Some(BRASS),
        _ => None,
    };
    if let Some(sprite) = frame {
        let fc = Some(CubeFace::new(sprite));
        boxed(
            draw,
            ([5.5, 1.0, 13.6], [10.5, 1.5, 15.0]),
            [fc, fc, fc, fc, fc, fc],
        );
        boxed(
            draw,
            ([5.5, 14.5, 13.6], [10.5, 15.0, 15.0]),
            [fc, fc, fc, fc, fc, fc],
        );
    }
    let h = 13.0 * level.clamp(0.0, 1.0) as f32;
    if h > 0.05 {
        let water = Some(CubeFace::new("block/water_still"));
        let b = ([6.6, 1.5, 14.1], [9.4, 1.5 + h, 14.9]);
        if !draw.cube(
            b.0,
            b.1,
            [water, water, water, water, water, water]
                .map(|f| f.map(|f| f.with_uv([0.0, 0.0, 2.8, h.min(16.0)]))),
            0x3F76E4,
        ) {
            eprintln!("[fras] a sprite was refused while drawing");
        }
    }
    draw.pop();
    let _ = (WHITE, span, faces);
}

/// Bounds for the flywheel (its rim reaches a block and a half out) and the
/// cylinder (its rod reaches the hub).
pub fn bounds(block: Block, b: &mut [f32; 6]) {
    match block {
        Block::Flywheel => {
            *b = [-1.0, -1.0, -1.0, 2.0, 2.0, 2.0];
        }
        Block::SteamCylinder => {
            *b = [-2.0, -1.0, -2.0, 3.0, 2.0, 3.0];
        }
        Block::Governor => {
            *b = [-0.5, -0.5, -0.5, 1.5, 1.5, 1.5];
        }
        _ => {}
    }
}
