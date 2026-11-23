//! The wrench (configure, repair, inspect) and the leather belt
//! (rotational network spec 6.6, 7.5, 11.2, 11.3).

use mod_api::{
    BlockClick, BlockState, BlockUpdateFlags, Hand, HostBlockStates, HostSide, InteractionResult,
    ItemKey, ItemPart, ItemStack, LevelRef, PlayerRef, ServerHost, StateConstraint, Text,
    UseOnEvent,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, read_part, with, with_level};
use super::world::{self, set_property};
use super::{effects, keys, num};
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::rot::part::{BELT_MAX_DISTANCE, CRANK_STALL, CRANK_TOP, WHEEL_TOP};
use crate::sim::rot::{BeltEnd, Jam, Kind, Material, Shape};

fn tr(key: &str, args: Vec<String>) -> Text {
    Text::translate(key, args.into_iter().map(Text::literal).collect())
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, line: Text) {
    let _ = player.tell(host, line);
}

fn bar(host: &mut dyn ServerHost, player: PlayerRef, line: Text) {
    let _ = player.action_bar(host, line);
}

/// The player, level and dimension of a use.
fn context(host: &dyn mod_api::Host, player: u128) -> Option<(PlayerRef, LevelRef, Dim)> {
    let p = host.player(player)?;
    let level = p.level(host)?;
    let dim = server::dim_of(host, level)?;
    Some((p, level, dim))
}

// ---- The wrench --------------------------------------------------------------

pub struct Wrench;

#[mod_api::horizon_part]
impl ItemPart for Wrench {
    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        Tooltip("wrench").tooltip(host, event);
    }

    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        wrench(host, event)
    }
}

fn wrench(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Option<InteractionResult> {
    let (player, level, dim) = context(&*host, event.player)?;
    let mut pos = Pos::from_array(event.pos);
    let mut block = block_at(&*host, level, pos)?;
    // A tachometer stands for the part it is on.
    if block == Block::Tachometer {
        let facing = server::property(&*host, level, pos, "facing")
            .as_deref()
            .and_then(Dir::from_name)?;
        pos = pos.offset(facing.opposite());
        block = block_at(&*host, level, pos)?;
    }
    if player.is_secondary_use_active(&*host) == Some(true) {
        for line in inspect(&*host, level, dim, pos, block) {
            tell(host, player, line);
        }
        return Some(InteractionResult::Success);
    }
    let jammed = with_level(dim, |l| l.mech.part(pos).and_then(|p| p.jam));
    if let Some(jam) = jammed {
        return Some(repair(host, player, level, dim, pos, block, jam));
    }
    let clicked = super::dir_of_vec(event.face);
    if configure(host, player, level, pos, block, clicked) {
        effects::sound(host, level, pos, "minecraft:block.chain.place", 0.6, 1.4);
        Some(InteractionResult::Success)
    } else {
        Some(InteractionResult::Pass)
    }
}

/// What a repair costs: item and count (spec 6.6).
pub fn repair_cost(block: Block) -> Vec<(&'static str, u32)> {
    match block {
        Block::ShaftWood
        | Block::BevelGearWood
        | Block::Pulley
        | Block::LargePulley
        | Block::Crank
        | Block::WaterWheel => {
            vec![("minecraft:stick", 1)]
        }
        Block::ShaftIron | Block::Bearing | Block::Clutch | Block::BevelGearIron => {
            vec![("minecraft:iron_nugget", 1)]
        }
        Block::Gearbox => vec![("minecraft:iron_nugget", 2)],
        Block::Grinder => vec![("minecraft:iron_nugget", 2), ("minecraft:flint", 1)],
        Block::Tachometer => vec![],
    }
}

fn cost_key(id: &str) -> Option<ItemKey> {
    let k = keys()?;
    Some(match id {
        "minecraft:stick" => k.stick,
        "minecraft:iron_nugget" => k.iron_nugget,
        "minecraft:flint" => k.flint,
        _ => return None,
    })
}

fn cost_text(cost: &[(&str, u32)]) -> Text {
    let mut t = Text::literal("");
    for (i, (id, n)) in cost.iter().enumerate() {
        if i > 0 {
            t = t.append(Text::literal(", "));
        }
        let name = id.replace(':', ".");
        t = t
            .append(Text::literal(format!("{n} × ")))
            .append(Text::translate(format!("item.{name}"), vec![]));
    }
    t
}

/// How many of `item` the player carries, by slot.
fn carried(host: &dyn mod_api::Host, player: PlayerRef, item: ItemKey) -> Vec<(u32, ItemStack)> {
    player
        .inventory_slots(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, s)| s.item() == item)
        .collect()
}

/// Take `count` of `item` from the player's inventory. False (and nothing
/// taken) if they don't have that many.
fn take(host: &mut dyn ServerHost, player: PlayerRef, item: ItemKey, count: u32) -> bool {
    let slots = carried(&*host, player, item);
    if slots.iter().map(|(_, s)| s.count()).sum::<u32>() < count {
        return false;
    }
    let mut left = count;
    for (slot, stack) in slots {
        if left == 0 {
            break;
        }
        let n = stack.count().min(left);
        left -= n;
        let _ = player.set_inventory_slot(host, slot, stack.shrink(n));
    }
    true
}

fn creative(host: &dyn mod_api::Host, player: PlayerRef) -> bool {
    player.has_infinite_materials(host) == Some(true)
}

fn repair(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
    jam: Jam,
) -> InteractionResult {
    let cost = repair_cost(block);
    if !creative(&*host, player) {
        let missing = cost.iter().any(|(id, n)| {
            cost_key(id).is_none_or(|k| {
                carried(&*host, player, k)
                    .iter()
                    .map(|(_, s)| s.count())
                    .sum::<u32>()
                    < *n
            })
        });
        if missing {
            let line = Text::translate("fras.repair.missing", vec![cost_text(&cost)]);
            bar(host, player, line);
            return InteractionResult::Fail;
        }
        for (id, n) in &cost {
            if let Some(k) = cost_key(id) {
                take(host, player, k, *n);
            }
        }
    }
    with_level(dim, |l| l.mech.repair(pos));
    effects::sound(host, level, pos, "minecraft:block.anvil.use", 0.5, 1.5);
    let line = Text::translate(
        "fras.repair.done",
        vec![Text::translate(format!("fras.jam.{}", jam.name()), vec![])],
    );
    bar(host, player, line);
    InteractionResult::Success
}

/// Right-click with the wrench: rotate, cycle, toggle or flip.
fn configure(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    block: Block,
    clicked: Option<Dir>,
) -> bool {
    let prop = |host: &dyn mod_api::Host, key: &str| server::property(host, level, pos, key);
    match block {
        Block::Bearing => {
            let axis = prop(&*host, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::Y);
            let current = prop(&*host, "mount").as_deref().and_then(Dir::from_name);
            let valid = world::mounts(&*host, level, pos, axis);
            if valid.len() < 2 {
                bar(host, player, tr("fras.wrench.no_other_mount", vec![]));
                return false;
            }
            let i = current
                .and_then(|c| valid.iter().position(|d| *d == c))
                .map_or(0, |i| (i + 1) % valid.len());
            set_property(host, level, pos, "mount", valid[i].name())
        }
        Block::BevelGearWood | Block::BevelGearIron => {
            let input = prop(&*host, "input")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::West);
            let output = prop(&*host, "output")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::South);
            match clicked {
                // A face across the input that isn't an output: the output moves there.
                Some(f) if f.axis() != input.axis() && f.axis() != output.axis() => {
                    set_property(host, level, pos, "output", f.name())
                }
                _ => {
                    let tee = prop(&*host, "tee").as_deref() == Some("true");
                    set_property(host, level, pos, "tee", if tee { "false" } else { "true" })
                }
            }
        }
        Block::Gearbox => {
            let slow = prop(&*host, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::North);
            set_property(host, level, pos, "facing", slow.opposite().name())
        }
        Block::Crank => {
            let socket = prop(&*host, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::North);
            let next = Dir::from_index((socket.index() + 1) % 6).unwrap_or(Dir::Down);
            set_property(host, level, pos, "facing", next.name())
        }
        Block::Grinder => {
            let socket = prop(&*host, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::North);
            set_property(host, level, pos, "facing", socket.clockwise().name())
        }
        Block::Pulley | Block::LargePulley => {
            let Some(entity) = host.block_entity(level, pos.to_array()) else {
                return false;
            };
            let Some(belt) = entity.data(&*host).as_ref().and_then(blocks::belt) else {
                bar(host, player, tr("fras.wrench.no_belt", vec![]));
                return false;
            };
            let crossed = !belt.crossed;
            for (a, b) in [(pos, belt.partner), (belt.partner, pos)] {
                let mut c = mod_api::Compound::new();
                blocks::put_bool(&mut c, "belt_crossed", crossed);
                let _ = b;
                server::merge(host, level, a, c);
            }
            let dim = server::dim_of(&*host, level);
            if let Some(dim) = dim {
                with_level(dim, |l| {
                    l.pending.insert(pos, true);
                    l.pending.insert(belt.partner, true);
                });
            }
            let key = if crossed {
                "fras.wrench.crossed"
            } else {
                "fras.wrench.open"
            };
            bar(host, player, tr(key, vec![]));
            true
        }
        _ => false,
    }
}

// ---- Inspection ----------------------------------------------------------------

/// The inspection overlay's lines (spec 11.3).
pub fn inspect(
    host: &dyn mod_api::Host,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
) -> Vec<Text> {
    let mut lines = vec![
        Text::literal("— ")
            .append(Text::translate(
                format!("block.fras.{}", block.name()),
                vec![],
            ))
            .append(Text::literal(" —")),
    ];
    let (part, reading, wheel) = with_level(dim, |l| {
        (
            l.mech.part(pos).cloned(),
            l.mech.reading(pos),
            l.wheel_torque.get(&pos).copied(),
        )
    });
    let Some(part) = part else {
        lines.push(tr("fras.inspect.not_connected", vec![]));
        return lines;
    };
    if let Some(jam) = part.jam {
        lines.push(Text::translate(
            "fras.inspect.jammed",
            vec![
                Text::translate(format!("fras.jam.{}", jam.name()), vec![]),
                cost_text(&repair_cost(block)),
            ],
        ));
        return lines;
    }
    let Some(r) = reading else {
        return lines;
    };
    let direction = if r.rpm.abs() < 0.05 {
        "fras.inspect.still"
    } else if r.rpm > 0.0 {
        "fras.inspect.positive"
    } else {
        "fras.inspect.negative"
    };
    lines.push(Text::translate(
        "fras.inspect.speed",
        vec![
            Text::literal(num(r.rpm.abs())),
            Text::translate(direction, vec![]),
        ],
    ));
    if r.paused {
        lines.push(tr("fras.inspect.paused", vec![]));
    }
    if let Some(red) = r.red_line {
        lines.push(tr("fras.inspect.red_line", vec![num(red.abs())]));
    }
    if let Some(min) = r.min_working {
        lines.push(tr("fras.inspect.min_working", vec![num(min.abs())]));
    }
    if r.stall > 0.0 && r.drawn > r.stall && r.rpm.abs() < 0.5 {
        lines.push(tr("fras.inspect.stalled", vec![num(r.drawn), num(r.stall)]));
    } else {
        let percent = if r.stall > 0.0 {
            r.drawn / r.stall * 100.0
        } else {
            0.0
        };
        lines.push(tr(
            "fras.inspect.load",
            vec![num(percent), num(r.drawn), num(r.give)],
        ));
    }
    if let Some(limit) = part.torque_limit() {
        lines.push(tr(
            "fras.inspect.torque",
            vec![num(part.torque.abs()), num(limit)],
        ));
    }
    if let Some((len, limit)) = part.span {
        let key = if part.sag {
            "fras.inspect.span_sag"
        } else {
            "fras.inspect.span"
        };
        lines.push(tr(key, vec![len.to_string(), limit.to_string()]));
    }
    match part.spec.kind {
        Kind::Crank => lines.push(tr(
            "fras.inspect.source",
            vec![
                num(CRANK_STALL),
                num(CRANK_TOP),
                num(part.drive.stall * part.drive.dir.abs()),
            ],
        )),
        Kind::WaterWheel => lines.push(tr(
            "fras.inspect.wheel",
            vec![num(wheel.unwrap_or(0.0).abs()), num(WHEEL_TOP)],
        )),
        Kind::Grinder => {
            let key = if part.load <= 0.0 {
                "fras.inspect.machine_idle"
            } else if r.rpm.abs() < crate::sim::rot::part::GRINDER_MIN_RPM {
                "fras.inspect.machine_slow"
            } else {
                "fras.inspect.machine_working"
            };
            lines.push(tr(key, vec![num(part.load)]));
        }
        _ => {}
    }
    if part.over_t > 0.0 {
        lines.push(tr(
            "fras.inspect.overspeed",
            vec![num(((1.0 - part.over_t) * 5.0).max(0.0))],
        ));
    }
    if part.shear_t > 0.0 {
        lines.push(tr(
            "fras.inspect.overload",
            vec![num(((1.0 - part.shear_t) * 5.0).max(0.0))],
        ));
    }
    lines.push(tr("fras.inspect.parts", vec![r.parts.to_string()]));
    let _ = (host, level, Material::Wood, Shape::Socket(Dir::Up));
    lines
}

// ---- Bearing swap ------------------------------------------------------------

/// Right-click a bearing onto a shaft: the bearing takes its place and the
/// shaft is given back (spec 7.2).
pub fn swap_bearing(
    host: &mut dyn ServerHost,
    click: &BlockClick<'_>,
) -> Option<InteractionResult> {
    let k = keys()?;
    let (player, level, _dim) = context(&*host, click.player)?;
    let pos = Pos::from_array(click.pos);
    let shaft = block_at(&*host, level, pos)?;
    let axis = server::property(&*host, level, pos, "axis")?;
    let state = {
        let states = host.block_states();
        BlockState::of(
            &states,
            k.bearing_block,
            &[StateConstraint::new("axis", axis)],
        )
        .ok()?
    };
    // The shaft's jam or speed goes with it; the bearing joins fresh.
    let placed = level
        .set_block(
            host,
            pos.to_array(),
            state,
            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
        )
        .unwrap_or(false);
    if !placed {
        return Some(InteractionResult::Fail);
    }
    if !creative(&*host, player) {
        let shaft_item = if shaft == Block::ShaftIron {
            k.iron_shaft
        } else {
            k.wooden_shaft
        };
        if let Some(stack) = ItemStack::new(shaft_item, 1) {
            let _ = player.give_stack(host, stack);
        }
        if let Some(stack) = click.stack {
            let slot = hand_slot(&*host, player, click.hand);
            let _ = player.set_inventory_slot(host, slot, stack.clone().shrink(1));
        }
    }
    effects::sound(host, level, pos, "minecraft:block.chain.place", 0.8, 1.0);
    Some(InteractionResult::Success)
}

fn hand_slot(host: &dyn mod_api::Host, player: PlayerRef, hand: Hand) -> u32 {
    match hand {
        Hand::Main => player.selected_slot(host).unwrap_or(0) as u32,
        Hand::Off => 40,
    }
}

// ---- The leather belt --------------------------------------------------------

pub struct LeatherBelt;

#[mod_api::horizon_part]
impl ItemPart for LeatherBelt {
    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        Tooltip("leather_belt").tooltip(host, event);
    }

    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        belt(host, event)
    }
}

/// What linking two pulleys needs: why it can't, or the belt's cost.
pub fn belt_check(a: Pos, axis_a: Axis, b: Pos, axis_b: Axis) -> Result<u32, &'static str> {
    if a == b {
        return Err("fras.belt.same");
    }
    if axis_a != axis_b {
        return Err("fras.belt.not_parallel");
    }
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let along = axis_a.unit();
    if d[0] * along[0] + d[1] * along[1] + d[2] * along[2] != 0 {
        return Err("fras.belt.not_in_plane");
    }
    let dist = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64).sqrt();
    if dist > BELT_MAX_DISTANCE as f64 + 1e-9 {
        return Err("fras.belt.too_far");
    }
    Ok((dist / 4.0).ceil().max(1.0) as u32)
}

fn belt(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Option<InteractionResult> {
    let (player, level, dim) = context(&*host, event.player)?;
    let pos = Pos::from_array(event.pos);
    let (block, spec, nbt) = read_part(&*host, level, pos)?;
    if !matches!(block, Block::Pulley | Block::LargePulley) {
        return None;
    }
    if blocks::belt(&nbt).is_some() {
        bar(host, player, tr("fras.belt.taken", vec![]));
        return Some(InteractionResult::Fail);
    }
    let start = with(|s| s.belt_starts.get(&event.player).copied());
    let start = start.filter(|(d, p)| *d == dim && *p != pos);
    let Some((_, a)) = start else {
        with(|s| s.belt_starts.insert(event.player, (dim, pos)));
        bar(host, player, tr("fras.belt.start", vec![]));
        return Some(InteractionResult::Success);
    };
    // The first pulley must still be a free pulley.
    let first = read_part(&*host, level, a).filter(|(b, _, n)| {
        matches!(b, Block::Pulley | Block::LargePulley) && blocks::belt(n).is_none()
    });
    let Some((block_a, spec_a, _)) = first else {
        with(|s| s.belt_starts.insert(event.player, (dim, pos)));
        bar(host, player, tr("fras.belt.start", vec![]));
        return Some(InteractionResult::Success);
    };
    let (Some(axis_a), Some(axis_b)) = (spec_a.axis(), spec.axis()) else {
        return Some(InteractionResult::Fail);
    };
    let cost = match belt_check(a, axis_a, pos, axis_b) {
        Ok(c) => c,
        Err(key) => {
            bar(host, player, tr(key, vec![]));
            return Some(InteractionResult::Fail);
        }
    };
    if !creative(&*host, player) {
        let have = event.stack.count();
        if have < cost {
            bar(host, player, tr("fras.belt.needs", vec![cost.to_string()]));
            return Some(InteractionResult::Fail);
        }
        let slot = hand_slot(&*host, player, event.hand);
        let _ = player.set_inventory_slot(host, slot, event.stack.clone().shrink(cost));
    }
    for (here, there, big) in [
        (a, pos, block == Block::LargePulley),
        (pos, a, block_a == Block::LargePulley),
    ] {
        let mut c = mod_api::Compound::new();
        blocks::put_belt(
            &mut c,
            Some(BeltEnd {
                partner: there,
                crossed: false,
            }),
        );
        blocks::put_bool(&mut c, "belt_big", big);
        // Both ends carry the cost: breaking either gives it back and clears
        // the other end, so it comes back once.
        blocks::put_i32(&mut c, "belt_cost", cost as i32);
        server::merge(host, level, here, c);
    }
    with(|s| s.belt_starts.remove(&event.player));
    with_level(dim, |l| {
        l.pending.insert(a, true);
        l.pending.insert(pos, true);
    });
    effects::sound(
        host,
        level,
        pos,
        "minecraft:item.armor.equip_leather",
        0.8,
        1.0,
    );
    bar(host, player, tr("fras.belt.linked", vec![]));
    Some(InteractionResult::Success)
}

// ---- Tooltips ------------------------------------------------------------------

/// One line under the item's name; Shift for details (spec 11.4).
pub struct Tooltip(pub &'static str);

/// GLFW's Shift modifier bit.
const SHIFT: u32 = 0x0001;
const GRAY: u32 = 0xA8A8_A8FF;

#[mod_api::horizon_part]
impl ItemPart for Tooltip {
    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        let key = format!("tooltip.fras.{}", self.0);
        event
            .lines
            .push((Text::translate(key.clone(), vec![]), GRAY));
        let details = format!("{key}.details");
        if host.translate(&details).is_none() {
            return;
        }
        if host.modifiers().bits & SHIFT != 0 {
            event.lines.push((Text::translate(details, vec![]), GRAY));
        } else {
            event
                .lines
                .push((Text::translate("tooltip.fras.shift", vec![]), 0x6060_60FF));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belts_link_parallel_pulleys_in_one_plane() {
        let a = Pos::new(0, 0, 0);
        assert_eq!(belt_check(a, Axis::X, Pos::new(0, -4, 0), Axis::X), Ok(1));
        assert_eq!(belt_check(a, Axis::X, Pos::new(0, -5, 3), Axis::X), Ok(2));
        assert_eq!(
            belt_check(a, Axis::X, Pos::new(1, -4, 0), Axis::X),
            Err("fras.belt.not_in_plane")
        );
        assert_eq!(
            belt_check(a, Axis::X, Pos::new(0, -4, 0), Axis::Z),
            Err("fras.belt.not_parallel")
        );
        assert_eq!(
            belt_check(a, Axis::Y, Pos::new(17, 0, 0), Axis::Y),
            Err("fras.belt.too_far")
        );
        assert_eq!(belt_check(a, Axis::Y, Pos::new(16, 0, 0), Axis::Y), Ok(4));
    }

    #[test]
    fn every_part_has_a_repair_cost() {
        for b in Block::ALL {
            assert_eq!(repair_cost(b).is_empty(), b == Block::Tachometer);
        }
    }
}
