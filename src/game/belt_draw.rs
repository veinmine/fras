//! Drawing belts, bucket elevators, diverters and the 0.4 machines
//! (logistics 11, machines 4.5 to 8.5). The frames are block models; this
//! draws what moves: rollers, the items riding a line (on the line's
//! controller block, from its snapshot, extrapolated each frame), paddle
//! drums, the hammer's head, the lathe's work, the saw blade and the
//! trommel's drum.

use mod_api::{
    BlockEntityDrawContext, BlockEntityRenderBoundingBoxEvent, BlockEntitySubmitEvent, ClientHost,
    Compound, CubeFace, ItemDisplayContext, Tag,
};

use super::blocks::{self, Block};
use super::draw::{CENTER, axle, boxed, faces, prop, span, spin};
use crate::sim::belt;
use crate::sim::geom::{Axis, Dir, Pos};

const LOG: &str = "block/stripped_oak_log";
const LOG_END: &str = "block/stripped_oak_log_top";
const IRON: &str = "block/iron_block";
const DARK: &str = "block/dark_oak_planks";
const WATER: &str = "block/blue_concrete";

pub fn handles(block: Block) -> bool {
    block.is_belt() || block == Block::BucketElevator || block.is_machine04()
}

pub fn submit(block: Block, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
    let nbt = event.entity.nbt.clone();
    let pos = Pos::from_array(event.pos);
    let state = event.state;
    let get = |key: &str| prop(host, state, key);
    let jammed = blocks::get_str(&nbt, "jam").is_some_and(|j| !j.is_empty());
    let rpm = if jammed {
        0.0
    } else {
        blocks::get_f64(&nbt, "rpm").unwrap_or(0.0)
    };
    let facing = get("facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::North);
    let draw = &mut *event.draw;
    let mut moving = false;
    match block {
        Block::ConveyorBelt | Block::Diverter => {
            // Rollers at both ends, across the heading, turning with a drive.
            let across = facing.clockwise().axis();
            let slope = get("slope").unwrap_or_default();
            if slope.is_empty() || slope == "flat" {
                for (i, c) in [1.5f32, 14.5].into_iter().enumerate() {
                    let mut pivot = [0.5f32, 4.5 / 16.0, 0.5];
                    pivot[facing.axis() as usize] = c / 16.0;
                    spin(draw, pos, i as u8, across, pivot, rpm * 4.0, 0.0);
                    let mut from = [0.0f32, 3.0, 0.0];
                    let mut to = [16.0f32, 6.0, 16.0];
                    from[facing.axis() as usize] = c - 1.5;
                    to[facing.axis() as usize] = c + 1.5;
                    from[across as usize] = 1.0;
                    to[across as usize] = 15.0;
                    boxed(draw, (from, to), faces(across, LOG, LOG_END));
                    draw.still();
                }
            }
            if block == Block::Diverter {
                // The flap: up while powered, flat otherwise.
                let left = get("flap").as_deref() != Some("right");
                let side = if left {
                    facing.clockwise().opposite()
                } else {
                    facing.clockwise()
                };
                let powered = get("powered").as_deref() == Some("true");
                let mut from = [2.0f32, 6.0, 2.0];
                let mut to = [14.0f32, if powered { 14.0 } else { 7.0 }, 14.0];
                let (lo, hi) = if side.is_positive() {
                    (14.0, 15.5)
                } else {
                    (0.5, 2.0)
                };
                from[side.axis() as usize] = lo;
                to[side.axis() as usize] = hi;
                let n = Some(CubeFace::new(IRON));
                boxed(draw, (from, to), [n, n, n, n, n, n]);
            }
            if blocks::get_bool(&nbt, "bl_ctl") {
                moving |= line_items(draw, host, &nbt, facing, false);
            }
        }
        Block::BucketElevator => {
            if get("part").as_deref() == Some("head") {
                spin(draw, pos, 0, facing.clockwise().axis(), CENTER, rpm, 0.0);
                axle(
                    draw,
                    facing.clockwise().axis(),
                    [1.0, 15.0],
                    3.0,
                    IRON,
                    IRON,
                );
                draw.still();
            }
            if blocks::get_bool(&nbt, "bl_ctl") {
                moving |= line_items(draw, host, &nbt, facing, true);
            }
        }
        Block::Washer => {
            // The paddle drum, along the socket's axis.
            let socket = facing;
            let axis = socket.axis();
            spin(draw, pos, 0, axis, [0.5, 10.0 / 16.0, 0.5], rpm, 0.0);
            boxed(
                draw,
                span(axis, [2.0, 14.0], [7.0, 9.0], [4.0, 16.0]),
                faces(axis, DARK, DARK),
            );
            boxed(
                draw,
                span(axis, [2.0, 14.0], [4.0, 12.0], [9.0, 11.0]),
                faces(axis, DARK, DARK),
            );
            draw.still();
            let tank = blocks::get_f64(&nbt, "tank").unwrap_or(0.0);
            if tank > 1.0 {
                let h =
                    4.0 + 6.0 * (tank / crate::sim::machine::WASHER_TANK).clamp(0.0, 1.0) as f32;
                let n = Some(CubeFace::new(WATER));
                boxed(
                    draw,
                    ([2.5, 4.0, 2.5], [13.5, h, 13.5]),
                    [None, n, None, None, None, None],
                );
            }
            shown(draw, host, &nbt, [0.5, 0.8, 0.5], 0.4, rpm, false);
        }
        Block::TripHammer => {
            // The head rises on the cam and drops twice a revolution: drawn
            // each frame while working.
            let working = blocks::get_bool(&nbt, "working");
            let lift = if working {
                moving = true;
                let t = draw.animation_time() as f32;
                let turns = t * rpm.abs() as f32 / 1200.0 * 2.0;
                turns.fract() * 4.0 / 16.0
            } else {
                2.0 / 16.0
            };
            let on_belt = blocks::get_bool(&nbt, "m_hold");
            let bottom = if on_belt { -9.0 } else { 6.0 };
            draw.push();
            draw.translate([0.0, lift, 0.0]);
            let n = Some(CubeFace::new(IRON));
            boxed(
                draw,
                ([5.0, bottom, 5.0], [11.0, bottom + 5.0, 11.0]),
                [n, n, n, n, n, n],
            );
            draw.pop();
            let axis = facing.axis();
            spin(draw, pos, 0, axis, [0.5, 14.0 / 16.0, 0.5], rpm, 0.0);
            boxed(
                draw,
                span(axis, [1.0, 15.0], [7.0, 9.0], [13.0, 15.0]),
                faces(axis, LOG, LOG_END),
            );
            draw.still();
            if !on_belt {
                shown(draw, host, &nbt, [0.5, 0.42, 0.5], 0.35, 0.0, true);
            }
        }
        Block::Lathe => {
            let axis = facing.axis();
            shown_spinning(draw, host, &nbt, pos, axis, rpm);
            spin(draw, pos, 1, axis, CENTER, rpm, 0.0);
            let range = if facing.is_positive() {
                [13.0, 16.0]
            } else {
                [0.0, 3.0]
            };
            boxed(
                draw,
                span(axis, range, [4.0, 12.0], [4.0, 12.0]),
                faces(axis, IRON, IRON),
            );
            draw.still();
        }
        Block::Sawmill => {
            // The blade stands out of the table, across the socket's axis.
            let axis = facing.axis();
            spin(draw, pos, 0, axis, [0.5, 12.0 / 16.0, 0.5], rpm * 3.0, 0.0);
            let n = Some(CubeFace::new(IRON));
            boxed(
                draw,
                span(axis, [7.75, 8.25], [2.0, 14.0], [6.0, 18.0]),
                [n, n, n, n, n, n],
            );
            draw.still();
        }
        Block::Trommel if get("part").as_deref() != Some("end") => {
            {
                // The drum spans both blocks along the facing.
                let axis = facing.axis();
                let (lo, hi) = if facing.is_positive() {
                    (1.0, 31.0)
                } else {
                    (-15.0, 15.0)
                };
                let mut pivot = [0.5f32, 0.55, 0.5];
                pivot[axis as usize] = (lo + hi) / 32.0;
                spin(draw, pos, 0, axis, pivot, rpm, 0.0);
                boxed(
                    draw,
                    span(axis, [lo, hi], [3.0, 13.0], [4.0, 14.0]),
                    faces(axis, "block/iron_bars", IRON),
                );
                draw.still();
            }
        }
        _ => {}
    }
    if !moving {
        draw.keep();
    }
}

/// A machine's work item, still or spinning about the vertical.
fn shown(
    draw: &mut dyn BlockEntityDrawContext,
    host: &dyn ClientHost,
    nbt: &Compound,
    at: [f32; 3],
    size: f32,
    rpm: f64,
    flat: bool,
) {
    let Some(stack) = super::machines::shown_item(host, nbt) else {
        return;
    };
    draw.push();
    if rpm.abs() > 0.1 && blocks::get_bool(nbt, "working") {
        draw.spin([0.0, 1.0, 0.0], at, (rpm * 0.3) as f32, 0.0);
    }
    draw.translate(at);
    if flat {
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
    }
    draw.scale([size, size, size]);
    let _ = draw.item(&stack, ItemDisplayContext::Fixed, None, 0);
    draw.still();
    draw.pop();
}

/// The lathe's work between centres, turning about the socket's axis.
fn shown_spinning(
    draw: &mut dyn BlockEntityDrawContext,
    host: &dyn ClientHost,
    nbt: &Compound,
    pos: Pos,
    axis: Axis,
    rpm: f64,
) {
    let Some(stack) = super::machines::shown_item(host, nbt) else {
        return;
    };
    let at = [0.5f32, 0.7, 0.5];
    draw.push();
    if blocks::get_bool(nbt, "working") {
        spin(draw, pos, 0, axis, at, rpm, 0.0);
    }
    draw.translate(at);
    if axis == Axis::X {
        draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
    }
    draw.scale([0.4, 0.4, 0.4]);
    let _ = draw.item(&stack, ItemDisplayContext::Fixed, None, 0);
    draw.still();
    draw.pop();
}

/// Draw the items riding a line, from the controller's snapshot, where
/// they are this frame. True while anything moves (no keeping).
fn line_items(
    draw: &mut dyn BlockEntityDrawContext,
    host: &dyn ClientHost,
    nbt: &Compound,
    heading: Dir,
    elevator: bool,
) -> bool {
    let Some(Tag::List(items)) = nbt.get(&"bl_items".into()) else {
        return false;
    };
    if items.is_empty() {
        return false;
    }
    let path: Vec<i32> = match nbt.get(&"bl_path".into()) {
        Some(Tag::IntArray(p)) => p.clone(),
        _ => return false,
    };
    let n = path.len() / 4;
    if n == 0 {
        return false;
    }
    let mut entries = Vec::with_capacity(items.len());
    let mut stacks = Vec::with_capacity(items.len());
    for t in items {
        if let Tag::Compound(c) = t
            && let Some(stack) = super::belts::stack_from_nbt(host, c)
        {
            entries.push((
                blocks::get_f64(c, "x").unwrap_or(0.0),
                blocks::get_bool(c, "h"),
            ));
            stacks.push(stack);
        }
    }
    let v = blocks::get_f64(nbt, "bl_v").unwrap_or(0.0);
    let t0 = blocks::get_f64(nbt, "bl_t").unwrap_or(0.0);
    let ticks = (draw.game_time() as f64 - t0).clamp(0.0, 40.0) as u32;
    let xs = belt::extrapolate_partial(&entries, v, n as f64, ticks, draw.partial() as f64);
    let [hx, _, hz] = heading.step();
    for (k, (x, stack)) in xs.iter().zip(&stacks).enumerate() {
        let i = (x.floor() as usize).min(n - 1);
        let f = (x - i as f64) as f32;
        let (bx, by, bz, slope) = (
            path[i * 4] as f32,
            path[i * 4 + 1] as f32,
            path[i * 4 + 2] as f32,
            path[i * 4 + 3],
        );
        let p = if elevator {
            [bx + 0.5, by + f, bz + 0.5]
        } else {
            let rise = match slope {
                1 => f,
                2 => 1.0 - f,
                _ => 0.0,
            };
            [
                bx + 0.5 + hx as f32 * (f - 0.5),
                by + belt::BELT_TOP as f32 + 0.1 + rise,
                bz + 0.5 + hz as f32 * (f - 0.5),
            ]
        };
        draw.push();
        draw.translate(p);
        draw.scale([0.5, 0.5, 0.5]);
        let _ = draw.item(stack, ItemDisplayContext::Ground, None, k as i32);
        draw.pop();
    }
    true
}

/// Culling bounds: a line's controller draws items along its whole path;
/// the trommel's drum reaches into its second block; the hammer's head
/// into the belt below.
pub fn bounds(block: Block, event: &mut BlockEntityRenderBoundingBoxEvent<'_>) {
    let b = &mut *event.bounds;
    let nbt = &event.entity.nbt;
    if blocks::get_bool(nbt, "bl_ctl")
        && let Some(Tag::IntArray(path)) = nbt.get(&"bl_path".into())
    {
        for c in path.chunks(4) {
            for i in 0..3 {
                b[i] = b[i].min(c[i] as f32 - 0.5);
                b[i + 3] = b[i + 3].max(c[i] as f32 + 1.5);
            }
        }
    }
    match block {
        Block::Trommel => {
            *b = [
                b[0].min(-1.0),
                b[1],
                b[2].min(-1.0),
                b[3].max(2.0),
                b[4],
                b[5].max(2.0),
            ];
        }
        Block::TripHammer => b[1] = b[1].min(-1.0),
        Block::Sawmill => b[4] = b[4].max(1.2),
        _ => {}
    }
}
