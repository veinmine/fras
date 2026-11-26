//! Work done on the level from the server tick: reading parts into the
//! network, bearing mounts, clutches, large pulleys' room and water wheels.

use mod_api::{BlockUpdateFlags, Host, HostBlockStates, LevelRef, ServerHost, SupportType};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, read_part, with, with_level};
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::rot::Shape;
use crate::sim::rot::part::{WHEEL_MAX, WHEEL_PER_BLOCK};

/// Set one block state property at `pos`. True if the block changed.
pub fn set_property(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    key: &str,
    value: &str,
) -> bool {
    let Some(state) = level.block_state(&*host, pos.to_array()) else {
        return false;
    };
    let new = {
        let states = host.block_states();
        match state.with(&states, key, value) {
            Ok(new) => new,
            Err(_) => return false,
        }
    };
    if new == state {
        return false;
    }
    level
        .set_block(
            host,
            pos.to_array(),
            new,
            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
        )
        .unwrap_or(false)
}

/// Break a block as if mined, dropping it.
pub fn break_block(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    let _ = level.destroy_block(host, pos.to_array(), true, None, false);
}

/// The queued work of one level.
pub fn queued(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let (pending, clear, bearings, clutches, larges, gearboxes) = with_level(dim, |l| {
        (
            std::mem::take(&mut l.pending),
            std::mem::take(&mut l.clear_belts),
            std::mem::take(&mut l.bearings),
            std::mem::take(&mut l.clutches),
            std::mem::take(&mut l.large_pulleys),
            std::mem::take(&mut l.gearboxes),
        )
    });
    // Belts whose other end broke (queued without a level: check it is ours).
    let orphans: Vec<(Pos, Pos)> = with(|s| std::mem::take(&mut s.orphan_belts));
    let mut keep = Vec::new();
    for (pos, broken) in orphans {
        if !level.is_loaded(&*host, pos.to_array()) {
            keep.push((pos, broken));
            continue;
        }
        let data = host
            .block_entity(level, pos.to_array())
            .and_then(|e| e.data(&*host));
        if data
            .as_ref()
            .and_then(blocks::belt)
            .is_some_and(|b| b.partner == broken)
        {
            clear_belt(host, level, dim, pos);
        }
    }
    if !keep.is_empty() {
        with(|s| s.orphan_belts.extend(keep));
    }
    for pos in clear {
        clear_belt(host, level, dim, pos);
    }
    for pos in clutches {
        if block_at(&*host, level, pos) == Some(Block::Clutch) {
            let powered = level.has_neighbor_signal(&*host, pos.to_array());
            set_property(
                host,
                level,
                pos,
                "powered",
                if powered { "true" } else { "false" },
            );
        }
    }
    for (pos, placed) in gearboxes {
        gearbox_signal(host, level, pos, placed);
    }
    for pos in bearings {
        check_bearing(host, level, pos);
    }
    for pos in larges {
        if block_at(&*host, level, pos).is_some_and(Block::is_large_pulley)
            && !large_pulley_fits(&*host, level, pos)
        {
            break_block(host, level, pos);
        }
    }
    for (pos, cause) in pending {
        match read_part(&*host, level, pos) {
            None => with_level(dim, |l| {
                l.mech.remove(pos);
                l.cranks.remove(&pos);
                l.wheel_torque.remove(&pos);
            }),
            Some((block, spec, nbt)) => with_level(dim, |l| {
                match l.mech.part(pos) {
                    Some(p) if p.loaded => l.mech.update(pos, spec, cause),
                    Some(_) => l.mech.insert(pos, spec, blocks::saved(&nbt), false),
                    None => l.mech.insert(pos, spec, blocks::saved(&nbt), cause),
                }
                if block == Block::WaterWheel {
                    l.wheels.insert(pos);
                    l.wheel_torque.entry(pos).or_insert(0.0);
                }
            }),
        }
    }
}

/// Take a pulley's belt off (its partner went away).
fn clear_belt(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, pos: Pos) {
    if let Some(entity) = host.block_entity(level, pos.to_array())
        && let Some(mut data) = entity.data(&*host)
    {
        blocks::put_belt(&mut data, None);
        let _ = entity.replace_data(host, data);
    }
    with_level(dim, |l| {
        l.pending.insert(pos, true);
    });
}

/// A gearbox reverses its ratio each time its redstone signal comes or goes:
/// its slow face moves to the other side. Just placed, it only notes the
/// signal it was placed into.
fn gearbox_signal(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, placed: bool) {
    if block_at(&*host, level, pos) != Some(Block::Gearbox) {
        return;
    }
    let powered = level.has_neighbor_signal(&*host, pos.to_array());
    let was = server::property(&*host, level, pos, "powered").as_deref() == Some("true");
    if powered == was {
        return;
    }
    let value = if powered { "true" } else { "false" };
    if placed {
        set_property(host, level, pos, "powered", value);
        return;
    }
    let slow = server::property(&*host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::North);
    set_properties(
        host,
        level,
        pos,
        &[("powered", value), ("facing", slow.opposite().name())],
    );
}

/// Set several block state properties at `pos` in one change.
pub fn set_properties(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    props: &[(&str, &str)],
) -> bool {
    let Some(state) = level.block_state(&*host, pos.to_array()) else {
        return false;
    };
    let new = {
        let states = host.block_states();
        let mut new = state;
        for (key, value) in props {
            match new.with(&states, key, value) {
                Ok(s) => new = s,
                Err(_) => return false,
            }
        }
        new
    };
    if new == state {
        return false;
    }
    level
        .set_block(
            host,
            pos.to_array(),
            new,
            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
        )
        .unwrap_or(false)
}

// ---- Bearings ---------------------------------------------------------------

/// Whether the block beside a bearing (toward `toward`) can hold it: a sturdy
/// face (as a lantern needs), a fence, a wall, iron bars, or a chain above.
pub fn holds(host: &dyn Host, level: LevelRef, pos: Pos, toward: Dir) -> bool {
    let n = pos.offset(toward);
    if level.block_face_sturdy(
        host,
        n.to_array(),
        super::face(toward.opposite()),
        SupportType::Center,
    ) == Some(true)
    {
        return true;
    }
    let Some(block) = level.block(host, n.to_array()) else {
        return false;
    };
    let id = block.id(host);
    let path = id.split(':').nth(1).unwrap_or(&id);
    path.ends_with("_fence")
        || path.ends_with("_wall")
        || path == "iron_bars"
        || path.ends_with("_bars")
        || (toward == Dir::Up && path.ends_with("chain"))
}

/// The mounts a bearing along `axis` can take, in the order of choice:
/// below, the sides, above.
pub fn mounts(host: &dyn Host, level: LevelRef, pos: Pos, axis: Axis) -> Vec<Dir> {
    let mut order = vec![Dir::Down];
    order.extend(Dir::HORIZONTAL);
    order.push(Dir::Up);
    order
        .into_iter()
        .filter(|d| d.axis() != axis && holds(host, level, pos, *d))
        .collect()
}

fn check_bearing(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    if block_at(&*host, level, pos) != Some(Block::Bearing) {
        return;
    }
    let axis = server::property(&*host, level, pos, "axis")
        .as_deref()
        .and_then(Axis::from_name)
        .unwrap_or(Axis::Y);
    let current = server::property(&*host, level, pos, "mount")
        .as_deref()
        .and_then(Dir::from_name);
    let valid = mounts(&*host, level, pos, axis);
    if current.is_some_and(|c| valid.contains(&c)) {
        return;
    }
    match valid.first() {
        Some(d) => {
            set_property(host, level, pos, "mount", d.name());
        }
        // Nothing holds it: it drops, and the line splits there (spec 5).
        None => break_block(host, level, pos),
    }
}

// ---- Large pulleys and water wheels ----------------------------------------

/// The 8 positions around `pos` in the plane across `axis`.
pub fn ring(pos: Pos, axis: Axis) -> Vec<Pos> {
    let others: Vec<Axis> = Axis::ALL.into_iter().filter(|a| *a != axis).collect();
    let (u, v) = (others[0].unit(), others[1].unit());
    let mut out = Vec::new();
    for du in -1..=1 {
        for dv in -1..=1 {
            if du == 0 && dv == 0 {
                continue;
            }
            out.push(Pos::new(
                pos.x + du * u[0] + dv * v[0],
                pos.y + du * u[1] + dv * v[1],
                pos.z + du * u[2] + dv * v[2],
            ));
        }
    }
    out
}

fn large_pulley_fits(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    let axis = server::property(host, level, pos, "axis")
        .as_deref()
        .and_then(Axis::from_name)
        .unwrap_or(Axis::Y);
    let states = host.block_states();
    ring(pos, axis).into_iter().all(|q| {
        level
            .block_state(host, q.to_array())
            .is_some_and(|s| states.replaceable(s) == Some(true))
    })
}

/// Whether placing `state` at `pos` would put a block into a water wheel's
/// paddles (the 8 positions around its hub). Liquids may go there.
pub fn in_wheel(
    host: &dyn Host,
    level: LevelRef,
    pos: mod_api::BlockPos,
    state: mod_api::BlockState,
) -> bool {
    let id = state.block().id(host);
    if matches!(
        id.as_str(),
        "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
    ) {
        return false;
    }
    let pos = Pos::new(pos.x, pos.y, pos.z);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                let hub = Pos::new(pos.x + dx, pos.y + dy, pos.z + dz);
                if block_at(host, level, hub) != Some(Block::WaterWheel) {
                    continue;
                }
                let axis = server::property(host, level, hub, "axis")
                    .as_deref()
                    .and_then(Axis::from_name)
                    .unwrap_or(Axis::X);
                if ring(hub, axis).contains(&pos) {
                    return true;
                }
            }
        }
    }
    false
}

/// Water wheels whose water may have changed: read their torque again.
pub fn wheels(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let wheels = with_level(dim, |l| std::mem::take(&mut l.wheels));
    for pos in wheels {
        let torque = match read_part(&*host, level, pos) {
            Some((Block::WaterWheel, spec, _)) => match spec.shape {
                Shape::Inline(axis) => wheel_torque(&*host, level, pos, axis),
                _ => 0.0,
            },
            _ => continue,
        };
        with_level(dim, |l| {
            l.wheel_torque.insert(pos, torque);
        });
    }
}

/// Torque from flowing water in the wheel's 8 paddle positions, about the
/// wheel's positive axis (spec 8.2).
pub fn wheel_torque(host: &dyn Host, level: LevelRef, pos: Pos, axis: Axis) -> f64 {
    let a = axis.unit();
    let mut sum = 0.0;
    for q in ring(pos, axis) {
        let Some(f) = water_flow(host, level, q) else {
            continue;
        };
        let o = [
            (q.x - pos.x) as f64,
            (q.y - pos.y) as f64,
            (q.z - pos.z) as f64,
        ];
        sum += paddle(o, f, [a[0] as f64, a[1] as f64, a[2] as f64]);
    }
    sum.clamp(-WHEEL_MAX, WHEEL_MAX)
}

/// The flow of the water at `q`, if it moves. Vanilla gives falling water in
/// open air no flow vector (only beside a wall does it point down), so a
/// falling block counts as flowing straight down. Sources and still water
/// give nothing (spec 8.2).
fn water_flow(host: &dyn Host, level: LevelRef, q: Pos) -> Option<[f64; 3]> {
    let state = level.fluid_state(host, q.to_array())?;
    if state.amount == 0 || state.source {
        return None;
    }
    if !state.fluid.id(host).ends_with("water") {
        return None;
    }
    if state.falling {
        return Some([0.0, -1.0, 0.0]);
    }
    let flow = level.fluid_flow(host, q.to_array())?;
    Some([flow.x, flow.y, flow.z])
}

/// One paddle's push: its offset from the hub `o`, the water's flow `f`,
/// the wheel's axis `a`. Signed by the flow along the rim; falling water on
/// the top row counts double (an overshot wheel).
pub fn paddle(o: [f64; 3], f: [f64; 3], a: [f64; 3]) -> f64 {
    if f.iter().map(|c| c * c).sum::<f64>() < 1e-6 {
        return 0.0;
    }
    let cross = [
        o[1] * f[2] - o[2] * f[1],
        o[2] * f[0] - o[0] * f[2],
        o[0] * f[1] - o[1] * f[0],
    ];
    let t = cross[0] * a[0] + cross[1] * a[1] + cross[2] * a[2];
    if t.abs() < 1e-3 {
        return 0.0;
    }
    let falling_on_top = o[1] > 0.5 && f[1] < -0.5;
    WHEEL_PER_BLOCK * t.signum() * if falling_on_top { 2.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_under_a_wheel_turns_it() {
        // Wheel along X; water flows north (-Z) under it (o = 0,-1,0).
        let a = [1.0, 0.0, 0.0];
        let under = paddle([0.0, -1.0, 0.0], [0.0, 0.0, -1.0], a);
        assert_eq!(under, WHEEL_PER_BLOCK);
        // The same flow over the top pushes the other way: they cancel.
        let over = paddle([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], a);
        assert_eq!(under + over, 0.0);
        // Still water gives nothing; falling water on the top row counts double.
        assert_eq!(paddle([0.0, -1.0, 0.0], [0.0; 3], a), 0.0);
        let falling = paddle([0.0, 1.0, 1.0], [0.0, -1.0, 0.0], a);
        assert_eq!(falling.abs(), 2.0 * WHEEL_PER_BLOCK);
    }

    #[test]
    fn the_ring_is_the_plane_across_the_axis() {
        let r = ring(Pos::new(0, 0, 0), Axis::X);
        assert_eq!(r.len(), 8);
        assert!(r.iter().all(|p| p.x == 0));
    }
}
