//! Drawing belts, bucket elevators, diverters and the 0.4 machines
//! (logistics 11, machines 4.5 to 8.5). The frames are block models; this
//! draws what moves: rollers, the items riding a line (on the line's
//! controller block, from its snapshot, extrapolated each frame), paddle
//! drums, the hammer's head, the lathe's chuck, the saw blade, the items
//! on the machines' built-in belts, and the trommel's drum and what tumbles
//! in it.

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
    let upper = get("half").as_deref() == Some("upper");
    // A machine's top and a trommel's middle draw at the speed their part
    // last reported (machines spec 2.3).
    let rpm = if jammed {
        0.0
    } else if upper || block == Block::Trommel {
        blocks::get_f64(&nbt, "m_rpm").unwrap_or(0.0)
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
        b if b.is_line_machine() && !upper => {
            // The drive: a hub turning in its socket.
            let socket = get("socket")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::Down);
            let axis = socket.axis();
            let range = if socket.is_positive() {
                [8.0, 16.0]
            } else {
                [0.0, 8.0]
            };
            spin(draw, pos, 0, axis, CENTER, rpm, 0.0);
            axle(draw, axis, range, 2.0, IRON, IRON);
            draw.still();
        }
        Block::Washer => {
            // The paddle drum across the belt.
            let axis = facing.clockwise().axis();
            spin(draw, pos, 0, axis, [0.5, 11.0 / 16.0, 0.5], rpm, 0.0);
            boxed(
                draw,
                span(axis, [2.0, 14.0], [10.0, 12.0], [7.0, 16.0]),
                faces(axis, DARK, DARK),
            );
            boxed(
                draw,
                span(axis, [2.0, 14.0], [7.0, 15.0], [10.0, 12.0]),
                faces(axis, DARK, DARK),
            );
            draw.still();
            let tank = blocks::get_f64(&nbt, "tank").unwrap_or(0.0);
            if tank > 1.0 {
                let h =
                    6.5 + 3.0 * (tank / crate::sim::machine::WASHER_TANK).clamp(0.0, 1.0) as f32;
                let n = Some(CubeFace::new(WATER));
                boxed(
                    draw,
                    ([1.5, 6.0, 2.5], [14.5, h, 13.5]),
                    [None, n, None, None, None, None],
                );
            }
            moving |= station_items(draw, host, &nbt, facing);
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
            draw.push();
            draw.translate([0.0, lift, 0.0]);
            let n = Some(CubeFace::new(IRON));
            boxed(
                draw,
                ([5.0, 6.5, 5.0], [11.0, 11.5, 11.0]),
                [n, n, n, n, n, n],
            );
            draw.pop();
            let axis = facing.clockwise().axis();
            spin(draw, pos, 0, axis, [0.5, 14.0 / 16.0, 0.5], rpm, 0.0);
            boxed(
                draw,
                span(axis, [1.0, 15.0], [13.0, 15.0], [7.0, 9.0]),
                faces(axis, LOG, LOG_END),
            );
            draw.still();
            moving |= station_items(draw, host, &nbt, facing);
        }
        Block::Lathe => {
            // The chuck, across the belt over the work point.
            let axis = facing.clockwise().axis();
            spin(draw, pos, 1, axis, [0.5, 11.0 / 16.0, 0.5], rpm, 0.0);
            boxed(
                draw,
                span(axis, [4.0, 7.0], [9.0, 13.0], [6.0, 10.0]),
                faces(axis, IRON, IRON),
            );
            draw.still();
            moving |= station_items(draw, host, &nbt, facing);
        }
        Block::Sawmill => {
            // The blade stands over the belt, in line with it.
            let axis = facing.clockwise().axis();
            spin(draw, pos, 0, axis, [0.5, 12.0 / 16.0, 0.5], rpm * 3.0, 0.0);
            let n = Some(CubeFace::new(IRON));
            boxed(
                draw,
                span(axis, [7.75, 8.25], [6.5, 17.5], [2.0, 14.0]),
                [n, n, n, n, n, n],
            );
            draw.still();
            moving |= station_items(draw, host, &nbt, facing);
        }
        Block::InlineGrinder => {
            // The millstone over the work point.
            spin(draw, pos, 0, Axis::Y, [0.5, 0.5, 0.5], rpm, 0.0);
            let n = Some(CubeFace::new("block/smooth_stone"));
            boxed(
                draw,
                ([3.0, 8.0, 3.0], [13.0, 11.5, 13.0]),
                [n, n, n, n, n, n],
            );
            draw.still();
            moving |= station_items(draw, host, &nbt, facing);
        }
        Block::Trommel if get("part").as_deref() == Some("middle") => {
            // The drum spans all three blocks along its axis.
            let axis = if get("axis").as_deref() == Some("z") {
                Axis::Z
            } else {
                Axis::X
            };
            spin(draw, pos, 0, axis, [0.5, 8.5 / 16.0, 0.5], rpm, 0.0);
            let bars = faces(axis, "block/iron_bars", IRON);
            for (u, v) in [
                ([3.5, 4.0], [3.5, 13.5]),
                ([12.5, 13.0], [3.5, 13.5]),
                ([3.5, 13.0], [3.5, 4.0]),
                ([3.5, 13.0], [13.0, 13.5]),
            ] {
                let (u, v) = if axis == Axis::X { (v, u) } else { (u, v) };
                boxed(draw, span(axis, [-16.0, 32.0], u, v), bars);
            }
            draw.still();
            moving |= drum_items(draw, host, &nbt, axis);
        }
        _ => {}
    }
    if !moving {
        draw.keep();
    }
}

/// The items on a machine's built-in belt (machines spec 2.2), from the
/// last snapshot, moved on to this frame: each stops at the work point
/// until worked, at the output end, or behind the one ahead. True while
/// anything moves.
fn station_items(
    draw: &mut dyn BlockEntityDrawContext,
    host: &dyn ClientHost,
    nbt: &Compound,
    facing: Dir,
) -> bool {
    use crate::sim::machine::station;
    let Some(Tag::List(list)) = nbt.get(&"st".into()) else {
        return false;
    };
    let v = blocks::get_f64(nbt, "st_v").unwrap_or(0.0);
    let t0 = match nbt.get(&"st_t".into()) {
        Some(Tag::Long(t)) => *t,
        _ => 0,
    };
    let dt = ((draw.game_time() - t0) as f64).clamp(0.0, 200.0) + draw.partial() as f64;
    let mut items: Vec<(f64, bool, mod_api::ItemStack)> = list
        .iter()
        .filter_map(|t| match t {
            Tag::Compound(c) => {
                let Some(Tag::Compound(i)) = c.get(&"i".into()) else {
                    return None;
                };
                let stack = super::belts::stack_from_nbt(host, i)?;
                Some((
                    blocks::get_f64(c, "x").unwrap_or(0.0),
                    blocks::get_bool(c, "d"),
                    stack,
                ))
            }
            _ => None,
        })
        .collect();
    // Front first.
    items.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut ahead = f64::INFINITY;
    let mut moving = false;
    let [fx, _, fz] = facing.step();
    for (k, (x, done, stack)) in items.iter().enumerate() {
        let stop = if *done { 1.0 } else { station::WORK_POINT };
        let limit = stop.min(ahead - station::SPACING).max(*x);
        let now = (x + v * dt).min(limit);
        moving |= v > 0.0 && now < limit;
        ahead = now;
        let p = [
            0.5 + fx as f32 * (now - 0.5) as f32,
            belt::BELT_TOP as f32 + 0.1,
            0.5 + fz as f32 * (now - 0.5) as f32,
        ];
        draw.push();
        draw.translate(p);
        draw.scale([0.5, 0.5, 0.5]);
        let _ = draw.item(stack, ItemDisplayContext::Ground, None, k as i32);
        draw.pop();
    }
    moving
}

/// What tumbles in a trommel's drum, drawn in its middle block: each item
/// moves from the end it came in toward the block it falls out of (8.4).
fn drum_items(
    draw: &mut dyn BlockEntityDrawContext,
    host: &dyn ClientHost,
    nbt: &Compound,
    axis: Axis,
) -> bool {
    use crate::sim::machine::{Size, Tumbling};
    let Some(Tag::List(list)) = nbt.get(&"drum".into()) else {
        return false;
    };
    let v = blocks::get_f64(nbt, "st_v").unwrap_or(0.0);
    let t0 = match nbt.get(&"st_t".into()) {
        Some(Tag::Long(t)) => *t,
        _ => 0,
    };
    let dt = ((draw.game_time() - t0) as f64).clamp(0.0, 200.0) + draw.partial() as f64;
    let mut moving = false;
    for (k, t) in list.iter().enumerate() {
        let Tag::Compound(c) = t else {
            continue;
        };
        let Some(Tag::Compound(i)) = c.get(&"i".into()) else {
            continue;
        };
        let Some(stack) = super::belts::stack_from_nbt(host, i) else {
            continue;
        };
        let size = match blocks::get_i32(c, "s").unwrap_or(2) {
            0 => Size::Fine,
            1 => Size::Medium,
            _ => Size::Oversize,
        };
        let mut tum = Tumbling {
            item: (),
            size,
            progress: blocks::get_f64(c, "p").unwrap_or(0.0),
            from_low: blocks::get_bool(c, "lo"),
        };
        let falls = tum.falls_at();
        let now = (tum.progress + v * dt).min(falls).max(tum.progress);
        moving |= v > 0.0 && now < falls;
        tum.progress = now;
        // Blocks from the low end; the middle block starts one block in.
        let along = (tum.along() - 1.0) as f32;
        // Tumbling: a little lift and sway as the drum turns.
        let wobble = ((now * 40.0 + k as f64 * 1.7).sin() * 0.08) as f32;
        let mut p = [0.5f32, 0.35 + wobble.abs(), 0.5];
        p[axis as usize] = along;
        let across = if axis == Axis::X { 2 } else { 0 };
        p[across] += wobble;
        draw.push();
        draw.translate(p);
        draw.scale([0.4, 0.4, 0.4]);
        let _ = draw.item(&stack, ItemDisplayContext::Ground, None, k as i32);
        draw.pop();
    }
    moving
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
/// the trommel's drum reaches into the blocks on either side of its middle.
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
        Block::Sawmill => b[4] = b[4].max(1.2),
        _ => {}
    }
}
