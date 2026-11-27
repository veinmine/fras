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
use super::{effects, keys, num, pipes, steam, steam_inspect};
use crate::sim::fluid::valve;
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::rot::part::{BELT_MAX_DISTANCE, CRANK_STALL, CRANK_TOP, WHEEL_TOP};
use crate::sim::rot::{BeltEnd, Jam, Kind, Material, Shape};

pub(crate) fn tr(key: &str, args: Vec<String>) -> Text {
    Text::translate(key, args.into_iter().map(Text::literal).collect())
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, line: Text) {
    let _ = player.tell(host, line);
}

pub(crate) fn bar(host: &mut dyn ServerHost, player: PlayerRef, line: Text) {
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
        wrench(host, event.player, event.pos, event.face)
    }
}

/// The wrench used on `pos`, clicked on `face`. The parts' own block hook
/// calls this too, so a machine never opens its menu for the wrench.
pub fn wrench(
    host: &mut dyn ServerHost,
    player: u128,
    pos: [i32; 3],
    face: [i32; 3],
) -> Option<InteractionResult> {
    let (player, level, dim) = context(&*host, player)?;
    let mut pos = Pos::from_array(pos);
    let mut block = block_at(&*host, level, pos)?;
    // A tachometer or gauge stands for the part it is on.
    let gauge_default = match block {
        Block::Tachometer => Some(Dir::North),
        Block::PressureGauge => Some(Dir::Up),
        Block::SightGlass => Some(Dir::North),
        _ => None,
    };
    if let Some(default) = gauge_default {
        let facing = server::property(&*host, level, pos, "facing")
            .as_deref()
            .and_then(Dir::from_name)
            .unwrap_or(default);
        pos = pos.offset(facing.opposite());
        block = block_at(&*host, level, pos)?;
    }
    let sneaking = player.is_secondary_use_active(&*host) == Some(true);
    // Belts and elevators: the line answers (logistics 3.2, 3.5, 8.1, 9, 11).
    if block.is_belt() || block == Block::BucketElevator {
        return Some(belt_wrench(host, player, level, dim, pos, block, sneaking));
    }
    // A line machine's top and a trommel's other blocks stand for the
    // block that is the part on the network (machines spec 2.3, 8.2).
    if block.is_machine04()
        && let Some(part) = super::machines::part_pos(&*host, level, pos, block)
    {
        pos = part;
    }
    if sneaking {
        for line in inspect(&*host, level, dim, pos, block) {
            tell(host, player, line);
        }
        return Some(InteractionResult::Success);
    }
    // A rusted part is scraped first (rust spec 8).
    if let Some(r) = super::rust::scrape(host, player, level, dim, pos, block) {
        return Some(r);
    }
    if block.is_plant()
        && server::property(&*host, level, pos, "damaged").as_deref() == Some("true")
    {
        return Some(repair_plant(host, player, level, dim, pos, block));
    }
    let jammed = with_level(dim, |l| l.mech.part(pos).and_then(|p| p.jam));
    if let Some(jam) = jammed {
        // A flywheel is only freed once its rim is clear (multiblocks spec 4.1).
        if jam == Jam::Blocked && steam::flywheel_blocker(&*host, level, pos).is_some() {
            bar(host, player, tr("fras.wrench.still_blocked", vec![]));
            return Some(InteractionResult::Fail);
        }
        return Some(repair(host, player, level, dim, pos, block, jam));
    }
    let clicked = super::dir_of_vec(face);
    if configure(host, player, level, dim, pos, block, clicked) {
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
        // 0.2 (steam spec 12, fluids spec 4.4 and 7.1)
        Block::CopperBoiler => vec![("minecraft:copper_ingot", 1)],
        Block::IronBoiler | Block::StorageTank => vec![("fras:iron_plate", 1)],
        Block::CopperPipe => vec![("minecraft:copper_nugget", 1)],
        Block::IronPipe
        | Block::ManualValve
        | Block::RedstoneValve
        | Block::SteamRelease
        | Block::EmergencyValve => vec![("minecraft:iron_nugget", 1)],
        Block::SteamCylinder => vec![("minecraft:iron_nugget", 2)],
        Block::Flywheel => vec![("minecraft:iron_nugget", 4)],
        Block::Pump => vec![("minecraft:iron_nugget", 2), ("minecraft:copper_ingot", 1)],
        // 0.3 (regulation spec 7)
        Block::Governor => vec![("minecraft:iron_nugget", 2)],
        Block::Bellows => vec![("minecraft:leather", 1)],
        Block::FurnaceDamper => vec![("minecraft:iron_nugget", 1)],
        Block::HeatExchanger | Block::Drain => vec![("minecraft:iron_nugget", 1)],
        Block::CopperRadiator | Block::IronRadiator => vec![],
        Block::Tachometer
        | Block::SteamDome
        | Block::Firebox
        | Block::PressureGauge
        | Block::SightGlass => vec![],
        Block::CreativeEngine
        | Block::LoadBank
        | Block::CreativeSteam
        | Block::CreativeWater
        | Block::CreativeDrain
        | Block::CreativeFirebox => vec![],
        // 0.4 (logistics 3.5, 8.2; machines 13.1; progression 4.4). A belt
        // takes leather or wool, which `belts::repair` handles itself.
        Block::ConveyorBelt | Block::Diverter => vec![("minecraft:leather", 1)],
        Block::BucketElevator => vec![("minecraft:iron_nugget", 2)],
        Block::IronPulley | Block::LargeIronPulley => vec![("minecraft:iron_nugget", 1)],
        b => b.machine().map(|m| m.repair().to_vec()).unwrap_or_default(),
    }
}

fn cost_key(id: &str) -> Option<ItemKey> {
    let k = keys()?;
    Some(match id {
        "minecraft:stick" => k.stick,
        "minecraft:iron_nugget" => k.iron_nugget,
        "minecraft:flint" => k.flint,
        "minecraft:copper_ingot" => k.copper_ingot,
        "minecraft:copper_nugget" => k.copper_nugget,
        "fras:iron_plate" => k.iron_plate,
        "minecraft:leather" => k.leather,
        _ => return None,
    })
}

pub fn cost_text(cost: &[(&str, u32)]) -> Text {
    let mut t = Text::literal("");
    for (i, (id, n)) in cost.iter().enumerate() {
        if i > 0 {
            t = t.append(Text::literal(", "));
        }
        let key = match id.strip_prefix('#') {
            Some(tag) => format!("fras.tag.{}", tag.replace(':', ".")),
            None => format!("item.{}", id.replace(':', ".")),
        };
        t = t
            .append(Text::literal(format!("{n} × ")))
            .append(Text::translate(key, vec![]));
    }
    t
}

/// How many of `item` the player carries, by slot.
pub(crate) fn carried(
    host: &dyn mod_api::Host,
    player: PlayerRef,
    item: ItemKey,
) -> Vec<(u32, ItemStack)> {
    player
        .inventory_slots(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, s)| s.item() == item)
        .collect()
}

/// Take `count` of `item` from the player's inventory. False (and nothing
/// taken) if they don't have that many.
pub(crate) fn take(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    item: ItemKey,
    count: u32,
) -> bool {
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

pub(crate) fn creative(host: &dyn mod_api::Host, player: PlayerRef) -> bool {
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
    if !pay(host, player, block) {
        return InteractionResult::Fail;
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

/// Mend a damaged steam part: a burst pipe or vessel is whole again, a
/// vessel starts empty and cold (steam spec 12, fluids spec 4.4).
fn repair_plant(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
) -> InteractionResult {
    if !pay(host, player, block) {
        return InteractionResult::Fail;
    }
    set_property(host, level, pos, "damaged", "false");
    if block.is_shell() {
        with_level(dim, |l| l.steam.works.reset(pos));
    }
    steam::queue(dim, pos, false);
    effects::sound(host, level, pos, "minecraft:block.anvil.use", 0.5, 1.5);
    bar(host, player, tr("fras.repair.mended", vec![]));
    InteractionResult::Success
}

/// The player's stacks that count as `id`: an item, or a tag FRAS's costs
/// name (`#minecraft:planks`, `#minecraft:wool`, read by the item's name).
pub(crate) fn carried_id(
    host: &dyn mod_api::Host,
    player: PlayerRef,
    id: &str,
) -> Vec<(u32, ItemStack)> {
    let suffix = match id {
        "#minecraft:planks" => Some("_planks"),
        "#minecraft:wool" => Some("_wool"),
        _ => None,
    };
    match suffix {
        Some(suffix) => player
            .inventory_slots(host)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, s)| {
                let name = s.item().id(host);
                name.starts_with("minecraft:") && name.ends_with(suffix)
            })
            .collect(),
        None => cost_key(id).map_or_else(Vec::new, |k| carried(host, player, k)),
    }
}

/// Take `count` of what `id` names from the player. False (and nothing
/// taken) if they don't have that many.
pub(crate) fn take_id(host: &mut dyn ServerHost, player: PlayerRef, id: &str, count: u32) -> bool {
    let slots = carried_id(&*host, player, id);
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

/// Take a part's repair cost from the player (nothing in creative). False,
/// with a message, if they don't carry it.
fn pay(host: &mut dyn ServerHost, player: PlayerRef, block: Block) -> bool {
    pay_cost(host, player, &repair_cost(block))
}

/// Take `cost` from the player (nothing in creative). False, with a
/// message, if they don't carry it.
pub(crate) fn pay_cost(host: &mut dyn ServerHost, player: PlayerRef, cost: &[(&str, u32)]) -> bool {
    if !creative(&*host, player) {
        let missing = cost.iter().any(|(id, n)| {
            carried_id(&*host, player, id)
                .iter()
                .map(|(_, s)| s.count())
                .sum::<u32>()
                < *n
        });
        if missing {
            let line = Text::translate("fras.repair.missing", vec![cost_text(cost)]);
            bar(host, player, line);
            return false;
        }
        for (id, n) in cost {
            take_id(host, player, id, *n);
        }
    }
    true
}

/// Right-click with the wrench: rotate, cycle, toggle or flip.
fn configure(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    dim: Dim,
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
        b if b.is_machine04() => super::machines::cycle_socket(host, level, dim, pos, b),
        b if b.is_pulley() => {
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
            with_level(dim, |l| {
                l.pending.insert(pos, true);
                l.pending.insert(belt.partner, true);
            });
            let key = if crossed {
                "fras.wrench.crossed"
            } else {
                "fras.wrench.open"
            };
            bar(host, player, tr(key, vec![]));
            true
        }
        Block::CopperPipe | Block::IronPipe => {
            let Some(face) = clicked else {
                return false;
            };
            let key = pipes::toggle(host, level, dim, pos, face);
            bar(host, player, tr(key, vec![]));
            key == "fras.wrench.pipe_on" || key == "fras.wrench.pipe_off"
        }
        Block::ManualValve | Block::RedstoneValve => {
            let axis = prop(&*host, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X);
            // The pipes on the old ends let go; the new ends connect.
            for d in [axis.positive(), axis.positive().opposite()] {
                let q = pos.offset(d);
                if block_at(&*host, level, q).is_some_and(Block::is_pipe) {
                    set_property(host, level, q, d.opposite().name(), "false");
                }
            }
            let next = match axis {
                Axis::X => Axis::Y,
                Axis::Y => Axis::Z,
                Axis::Z => Axis::X,
            };
            let done = set_property(host, level, pos, "axis", next.name());
            pipes::connect_around(host, level, dim, pos);
            steam::queue(dim, pos, false);
            done
        }
        Block::EmergencyValve => {
            let nbt = host
                .block_entity(level, pos.to_array())
                .and_then(|e| e.data(&*host))
                .unwrap_or_default();
            let set = blocks::get_f64(&nbt, "set").unwrap_or(valve::DEFAULT_SET);
            let next = valve::next_set(set);
            let mut c = mod_api::Compound::new();
            blocks::put_f64(&mut c, "set", next);
            server::merge(host, level, pos, c);
            steam::queue(dim, pos, false);
            bar(
                host,
                player,
                tr("fras.wrench.set_pressure", vec![format!("{next:.1}")]),
            );
            true
        }
        Block::Flywheel => {
            let nbt = host
                .block_entity(level, pos.to_array())
                .and_then(|e| e.data(&*host))
                .unwrap_or_default();
            let reverse = !blocks::get_bool(&nbt, "reverse");
            let mut c = mod_api::Compound::new();
            blocks::put_bool(&mut c, "reverse", reverse);
            server::merge(host, level, pos, c);
            steam::queue(dim, pos, false);
            let key = if reverse {
                "fras.wrench.reverse"
            } else {
                "fras.wrench.forward"
            };
            bar(host, player, tr(key, vec![]));
            true
        }
        Block::Pump => {
            let socket = prop(&*host, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::North);
            let next = Dir::from_index((socket.index() + 1) % 6).unwrap_or(Dir::Down);
            let done = set_property(host, level, pos, "facing", next.name());
            pipes::connect_around(host, level, dim, pos);
            done
        }
        // ---- 0.3 ----
        Block::Governor => {
            // The socket turns among the four faces across the pipe.
            let axis = prop(&*host, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X);
            let socket = prop(&*host, "socket")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::Up);
            let faces: Vec<Dir> = Dir::ALL.into_iter().filter(|d| d.axis() != axis).collect();
            let i = faces
                .iter()
                .position(|d| *d == socket)
                .map_or(0, |i| (i + 1) % faces.len());
            set_property(host, level, pos, "socket", faces[i].name())
        }
        Block::FurnaceDamper => {
            let nbt = host
                .block_entity(level, pos.to_array())
                .and_then(|e| e.data(&*host))
                .unwrap_or_default();
            let set =
                blocks::get_f64(&nbt, "set").unwrap_or(crate::sim::steam::damper::DEFAULT_SET);
            let next = crate::sim::steam::damper::next_set(set);
            let mut c = mod_api::Compound::new();
            blocks::put_f64(&mut c, "set", next);
            server::merge(host, level, pos, c);
            steam::queue(dim, pos, false);
            bar(
                host,
                player,
                tr("fras.wrench.damper_set", vec![format!("{next:.1}")]),
            );
            true
        }
        Block::HeatExchanger => {
            // Turn it end for end: inlet and outlet swap.
            let f = prop(&*host, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::Up);
            let next = Dir::from_index((f.index() + 1) % 6).unwrap_or(Dir::Down);
            let done = set_property(host, level, pos, "facing", next.name());
            pipes::connect_around(host, level, dim, pos);
            done
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
    if let Some(line) = super::rust::line(host, level, dim, pos, block) {
        lines.push(line);
    }
    if block == Block::Crank
        && let Some(line) = super::golem::line(dim, pos)
    {
        lines.push(line);
    }
    let (part, reading, wheel, slipping) = with_level(dim, |l| {
        (
            l.mech.part(pos).cloned(),
            l.mech.reading(pos),
            l.wheel_torque.get(&pos).copied(),
            l.mech.belt_slipping(pos),
        )
    });
    let Some(part) = part else {
        if block.is_plant() {
            steam_inspect::lines(host, level, dim, pos, block, &mut lines);
        } else {
            lines.push(tr("fras.inspect.not_connected", vec![]));
        }
        return lines;
    };
    if let Some(jam) = part.jam {
        // Rusted through: the rust line already says to scrape it with an
        // iron plate (rust spec 8); the wrench's repair cost doesn't apply.
        if jam != Jam::Rusted {
            lines.push(Text::translate(
                "fras.inspect.jammed",
                vec![
                    Text::translate(format!("fras.jam.{}", jam.name()), vec![]),
                    cost_text(&repair_cost(block)),
                ],
            ));
        }
        if jam == Jam::Blocked
            && let Some(q) = steam::flywheel_blocker(host, level, pos)
        {
            lines.push(tr(
                "fras.inspect.blocked_at",
                vec![q.x.to_string(), q.y.to_string(), q.z.to_string()],
            ));
        }
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
    if slipping {
        lines.push(tr(
            "fras.inspect.slipping",
            vec![num(crate::sim::rot::part::BELT_LIMIT)],
        ));
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
    if block.is_plant() {
        steam_inspect::lines(host, level, dim, pos, block, &mut lines);
    }
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

pub(crate) fn hand_slot(host: &dyn mod_api::Host, player: PlayerRef, hand: Hand) -> u32 {
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
    if !block.is_pulley() {
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
    let first =
        read_part(&*host, level, a).filter(|(b, _, n)| b.is_pulley() && blocks::belt(n).is_none());
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
        (a, pos, block.is_large_pulley()),
        (pos, a, block_a.is_large_pulley()),
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
/// Tooltip lines wrap at this many GUI pixels.
const TOOLTIP_WIDTH: i32 = 200;

#[mod_api::horizon_part]
impl ItemPart for Tooltip {
    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        let key = format!("tooltip.fras.{}", self.0);
        // Long texts wrap, so a tooltip fits on the screen.
        let mut push = |text: Text| match host.text_split(&text, TOOLTIP_WIDTH) {
            Ok(lines) if !lines.is_empty() => {
                event.lines.extend(lines.into_iter().map(|l| (l, GRAY)))
            }
            _ => event.lines.push((text, GRAY)),
        };
        push(Text::translate(key.clone(), vec![]));
        let details = format!("{key}.details");
        if host.translate(&details).is_none() {
            return;
        }
        if host.modifiers().bits & SHIFT != 0 {
            push(Text::translate(details, vec![]));
        } else {
            event
                .lines
                .push((Text::translate("tooltip.fras.shift", vec![]), 0x6060_60FF));
        }
    }
}

// ---- Belts and elevators ------------------------------------------------------

/// The wrench on a belt, diverter or elevator: sneak for the overlay;
/// otherwise repair a jammed line, swap a diverter's flap, turn an
/// elevator's outlet or reverse a belt line.
fn belt_wrench(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
    sneaking: bool,
) -> InteractionResult {
    if sneaking {
        for line in belt_inspect(&*host, level, dim, pos, block) {
            tell(host, player, line);
        }
        return InteractionResult::Success;
    }
    let (blocks_of_line, drives, kind) = with_level(dim, |l| {
        l.belts
            .line_at(pos)
            .map(|(ls, _)| (ls.line.blocks.clone(), ls.drives.clone(), Some(ls.kind)))
            .unwrap_or((vec![pos], Vec::new(), None))
    });
    let jammed: Vec<(Pos, Jam)> = with_level(dim, |l| {
        drives
            .iter()
            .filter_map(|d| l.mech.part(*d).and_then(|p| p.jam).map(|j| (*d, j)))
            .collect()
    });
    if let Some((_, jam)) = jammed.first().copied() {
        // A belt takes 1 leather or 1 wool (logistics 3.5).
        let cost: Vec<(&str, u32)> = if block == Block::BucketElevator {
            repair_cost(block)
        } else if !carried_id(&*host, player, "minecraft:leather").is_empty() {
            vec![("minecraft:leather", 1)]
        } else {
            vec![("#minecraft:wool", 1)]
        };
        if !pay_cost(host, player, &cost) {
            return InteractionResult::Fail;
        }
        with_level(dim, |l| {
            for (d, _) in &jammed {
                l.mech.repair(*d);
            }
        });
        effects::sound(host, level, pos, "minecraft:block.anvil.use", 0.5, 1.5);
        let line = Text::translate(
            "fras.repair.done",
            vec![Text::translate(format!("fras.jam.{}", jam.name()), vec![])],
        );
        bar(host, player, line);
        return InteractionResult::Success;
    }
    let prop = |host: &dyn mod_api::Host, q: Pos, key: &str| server::property(host, level, q, key);
    let done = match block {
        Block::Diverter => {
            let flap = if prop(&*host, pos, "flap").as_deref() == Some("right") {
                "left"
            } else {
                "right"
            };
            set_property(host, level, pos, "flap", flap)
        }
        Block::BucketElevator => {
            // The head's outlet turns to the next horizontal side that isn't
            // its drive (logistics 8.1).
            let head = match kind {
                Some(_) => *blocks_of_line.last().unwrap_or(&pos),
                None => pos,
            };
            let facing = prop(&*host, head, "facing")
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(Dir::North);
            let drive = prop(&*host, head, "drive")
                .as_deref()
                .and_then(Dir::from_name);
            let mut next = facing.clockwise();
            if Some(next) == drive {
                next = next.clockwise();
            }
            set_property(host, level, head, "facing", next.name())
        }
        _ => {
            // Reverse the whole line's heading (logistics 3.2).
            let mut any = false;
            for b in &blocks_of_line {
                if let Some(f) = prop(&*host, *b, "facing")
                    .as_deref()
                    .and_then(Dir::from_name)
                {
                    any |= set_property(host, level, *b, "facing", f.opposite().name());
                }
            }
            any
        }
    };
    if done {
        effects::sound(host, level, pos, "minecraft:block.chain.place", 0.6, 1.4);
        InteractionResult::Success
    } else {
        InteractionResult::Pass
    }
}

/// The overlay of a belt line, elevator or diverter (logistics 11).
pub fn belt_inspect(
    host: &dyn mod_api::Host,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
) -> Vec<Text> {
    use super::belts::{EndState, LineKind};
    use crate::sim::belt;
    let mut lines = vec![
        Text::literal("— ")
            .append(Text::translate(
                format!("block.fras.{}", block.name()),
                vec![],
            ))
            .append(Text::literal(" —")),
    ];
    let info = with_level(dim, |l| {
        let unformed = l.belts.unformed.get(&pos).copied();
        let line = l.belts.line_at(pos).map(|(ls, _)| {
            let rpm = ls
                .drives
                .first()
                .and_then(|d| l.mech.speed(*d))
                .map_or(0.0, |s| s.0);
            (ls.clone(), rpm)
        });
        (unformed, line)
    });
    let (unformed, line) = info;
    if let Some(why) = unformed {
        lines.push(tr(&format!("fras.belt.unformed.{why}"), vec![]));
        return lines;
    }
    let Some((ls, rpm)) = line else {
        lines.push(tr("fras.inspect.not_connected", vec![]));
        return lines;
    };
    let bps = ls.v.abs() * 20.0;
    lines.push(tr("fras.belt.speed", vec![num(bps), num(rpm.abs())]));
    lines.push(tr(
        "fras.belt.items",
        vec![ls.line.count().to_string(), ls.line.capacity().to_string()],
    ));
    lines.push(tr("fras.belt.load", vec![num(ls.load)]));
    match ls.kind {
        LineKind::Belt => {
            lines.push(tr(
                "fras.belt.drives",
                vec![
                    ls.drives.len().to_string(),
                    belt::drives_needed(ls.line.len()).to_string(),
                ],
            ));
            lines.push(tr("fras.belt.length", vec![ls.line.len().to_string()]));
            let way = if ls.v < -1e-9 {
                "fras.belt.backwards"
            } else {
                "fras.belt.forward"
            };
            lines.push(tr(way, vec![]));
        }
        LineKind::Elevator { outlet } => {
            lines.push(tr("fras.belt.height", vec![ls.line.len().to_string()]));
            let head = *ls.line.blocks.last().unwrap_or(&pos);
            let q = head.offset(outlet);
            lines.push(tr(
                "fras.belt.outlet",
                vec![q.x.to_string(), q.y.to_string(), q.z.to_string()],
            ));
        }
    }
    let end = match &ls.end {
        EndState::Idle => tr("fras.belt.end.idle", vec![]),
        EndState::Delivering(q) => tr(
            "fras.belt.end.delivering",
            vec![q.x.to_string(), q.y.to_string(), q.z.to_string()],
        ),
        EndState::Open => tr("fras.belt.end.open", vec![]),
        EndState::BlockedSolid => tr("fras.belt.end.solid", vec![]),
        EndState::BlockedFull => tr("fras.belt.end.full", vec![]),
    };
    lines.push(end);
    if let Some(stop) = ls.stop {
        lines.push(tr(
            &format!("fras.belt.stop.{}", super::belts::stop_name(stop)),
            vec![],
        ));
    }
    if block == Block::Diverter {
        let powered = server::property(host, level, pos, "powered").as_deref() == Some("true");
        let key = if powered {
            "fras.belt.diverter.on"
        } else {
            "fras.belt.diverter.off"
        };
        let heading = server::property(host, level, pos, "facing")
            .as_deref()
            .and_then(Dir::from_name)
            .unwrap_or(Dir::North);
        let out = super::belts::flap_dir(host, level, pos, heading).opposite();
        let q = pos.offset(out);
        lines.push(tr(
            key,
            vec![q.x.to_string(), q.y.to_string(), q.z.to_string()],
        ));
    }
    lines
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
            let never_damaged = matches!(
                b,
                Block::Tachometer
                    | Block::SteamDome
                    | Block::Firebox
                    | Block::PressureGauge
                    | Block::SightGlass
                    | Block::CopperRadiator
                    | Block::IronRadiator
                    | Block::CreativeEngine
                    | Block::LoadBank
                    | Block::CreativeSteam
                    | Block::CreativeWater
                    | Block::CreativeDrain
                    | Block::CreativeFirebox
            );
            assert_eq!(repair_cost(b).is_empty(), never_damaged);
        }
    }
}
