//! Belts, bucket elevators and diverters on Horizon (logistics spec 10):
//! lines traced from the blocks, their drives tied in the network, stepped
//! once per level tick after the network, items handed between lines,
//! inventories, elevator boots and the world, saved per block at level save
//! and when a line pauses, and a snapshot per line for clients to
//! extrapolate.
//!
//! Belt blocks don't tick. Hooks queue work in `Belts`; `tick` does it with a
//! host that can write. While a tick works, the level's `Belts` is taken out
//! of the shared state (so the hooks its writes trigger can't deadlock), and
//! what they queued meanwhile is merged back afterwards.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use mod_api::{
    Compound, Host, HostTransaction, ItemResource, ItemStack, LevelRef, STORAGE, ServerHost, Tag,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, property, with_level};
use crate::sim::belt::{self, Action, Line, Slope, Stations, Stop};
use crate::sim::geom::{Axis, Dir, Pos};

/// What a line is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    Belt,
    /// A bucket elevator with its head's outlet.
    Elevator {
        outlet: Dir,
    },
}

/// What a line's leading end does now (the overlay, logistics 11).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum EndState {
    #[default]
    Idle,
    /// Handing items to a block.
    Delivering(Pos),
    /// Items fall off into the world.
    Open,
    BlockedSolid,
    BlockedFull,
}

/// A line in a level.
#[derive(Clone, Debug)]
pub struct LineState {
    pub line: Line<ItemStack>,
    pub kind: LineKind,
    /// Blocks with a drive (belt) or the head (elevator, if driven).
    pub drives: Vec<Pos>,
    pub stop: Option<Stop>,
    /// Blocks per tick, signed, last tick.
    pub v: f64,
    /// What clients last got: speed and stop.
    pub sent_v: f64,
    pub sent_stop: Option<Stop>,
    pub need_sync: bool,
    pub end: EndState,
    /// It was paused and its items written to its blocks.
    pub written: bool,
    pub load: f64,
}

/// A throughput counter at a belt block (debug tools).
#[derive(Clone, Debug, Default)]
pub struct Counter {
    pub total: u64,
    /// Game ticks of the last 1200 crossings, for the per-minute rate.
    pub recent: std::collections::VecDeque<u64>,
    pub since: u64,
}

/// One level's belts.
#[derive(Default)]
pub struct Belts {
    pub lines: Vec<Option<LineState>>,
    /// Which line and block index each belt or elevator block is.
    pub by_block: HashMap<Pos, (usize, usize)>,
    /// Blocks to trace again (placed, broken, turned, loaded).
    pub dirty: BTreeSet<Pos>,
    /// Blocks whose drive may have changed (a neighbour changed).
    pub drive_check: BTreeSet<Pos>,
    /// Items saved by a block, waiting for its line to be built.
    pub restore: HashMap<Pos, Vec<(ItemStack, f64)>>,
    /// Blocks broken: their items drop.
    pub broken: Vec<Pos>,
    /// Blocks whose chunk unloaded: their line leaves memory.
    pub unloaded: BTreeSet<Pos>,
    /// Diverters whose redstone input may have changed.
    pub redstone: BTreeSet<Pos>,
    /// Diverters' comparator pulses: ticks left.
    pub pulses: HashMap<Pos, u32>,
    /// Throughput counters by block (debug tools).
    pub counters: BTreeMap<Pos, Counter>,
    /// Bucket elevator columns that don't form, and why (the wrench).
    pub unformed: HashMap<Pos, &'static str>,
    /// Ticks since the world started, for counters.
    pub ticks: u64,
    /// Items that fell off ends, were pushed off, or were delivered (debug report).
    pub stats: Stats,
}

/// Running totals for the debug report.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub delivered: u64,
    pub dropped: u64,
    pub diverted: u64,
}

impl Belts {
    /// Merge what hooks queued while this level's belts were taken out.
    fn absorb(&mut self, other: Belts) {
        self.dirty.extend(other.dirty);
        self.drive_check.extend(other.drive_check);
        self.redstone.extend(other.redstone);
        self.broken.extend(other.broken);
        self.unloaded.extend(other.unloaded);
        for (pos, saved) in other.restore {
            self.restore.entry(pos).or_default().extend(saved);
        }
        for (pos, c) in other.counters {
            self.counters.entry(pos).or_insert(c);
        }
    }

    pub fn line_at(&self, pos: Pos) -> Option<(&LineState, usize)> {
        let (li, i) = *self.by_block.get(&pos)?;
        Some((self.lines.get(li)?.as_ref()?, i))
    }

    /// Lines alive.
    pub fn iter(&self) -> impl Iterator<Item = &LineState> {
        self.lines.iter().flatten()
    }
}

/// A belt or elevator block's way in for hoppers: one slot, emptied onto
/// the line each tick (logistics 4.2).
pub fn inbox() -> mod_api::ModContainer {
    mod_api::ModContainer {
        slots: 1,
        faces: [vec![0], vec![0], vec![0], vec![0], vec![0], vec![0]],
        inputs: vec![0],
        outputs: vec![],
        menu: None,
    }
}

// ---- Hooks' side ---------------------------------------------------------------

/// A belt or elevator block was placed, turned or changed shape.
pub fn queue(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.belts.dirty.insert(pos);
        l.belts.drive_check.insert(pos);
    });
}

/// A belt or elevator block was broken: its items drop.
pub fn broke(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.belts.broken.push(pos);
        l.belts.dirty.insert(pos);
    });
}

/// A neighbour changed: a shaft may drive it now, or no more; a
/// diverter's redstone may have changed.
pub fn neighbour(dim: Dim, pos: Pos, diverter: bool) {
    with_level(dim, |l| {
        l.belts.drive_check.insert(pos);
        if diverter {
            l.belts.redstone.insert(pos);
        }
    });
}

/// Diverters follow their redstone (logistics 9).
fn diverters(host: &mut dyn ServerHost, level: LevelRef, belts: &mut Belts) {
    for pos in std::mem::take(&mut belts.redstone) {
        if block_at(&*host, level, pos) != Some(Block::Diverter) {
            continue;
        }
        let powered = level.has_neighbor_signal(&*host, pos.to_array());
        super::world::set_property(
            host,
            level,
            pos,
            "powered",
            if powered { "true" } else { "false" },
        );
    }
}

/// A belt block's entity loaded: bring back its items (unless its line is
/// still in memory, which is newer).
pub fn loaded(dim: Dim, pos: Pos, saved: Vec<(ItemStack, f64)>) {
    with_level(dim, |l| {
        if !l.belts.by_block.contains_key(&pos) && !saved.is_empty() {
            l.belts.restore.insert(pos, saved);
        }
        l.belts.dirty.insert(pos);
        l.belts.drive_check.insert(pos);
    });
}

/// A belt block's chunk unloaded.
pub fn unloaded(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.belts.unloaded.insert(pos);
    });
}

/// The saved items of a belt block's entity (logistics 10.2).
pub fn saved_items(host: &dyn Host, nbt: &Compound) -> Vec<(ItemStack, f64)> {
    let Some(Tag::List(list)) = nbt.get(&"bitems".into()) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|t| match t {
            Tag::Compound(c) => {
                let stack = stack_from_nbt(host, c)?;
                Some((stack, blocks::get_f64(c, "x").unwrap_or(0.5)))
            }
            _ => None,
        })
        .collect()
}

/// An item stack as a compound: id, count, component patch.
pub fn stack_to_nbt(host: &dyn Host, stack: &ItemStack) -> Compound {
    let mut c = Compound::new();
    blocks::put_str(&mut c, "id", &stack.item().id(host));
    if stack.count() != 1 {
        blocks::put_i32(&mut c, "n", stack.count() as i32);
    }
    if let Some(comp) = stack.components()
        && !comp.as_nbt().is_empty()
    {
        c.insert("c".into(), Tag::Compound(comp.as_nbt().clone()));
    }
    c
}

pub fn stack_from_nbt(host: &dyn Host, c: &Compound) -> Option<ItemStack> {
    let id = blocks::get_str(c, "id")?;
    let key = super::grinder::item_key(host, &id)?;
    let n = blocks::get_i32(c, "n").unwrap_or(1).max(1) as u32;
    let mut stack = ItemStack::new(key, n)?;
    if let Some(Tag::Compound(patch)) = c.get(&"c".into()) {
        let _ = stack.apply_components(patch);
    }
    Some(stack)
}

// ---- Reading the world ------------------------------------------------------------

fn heading_at(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<Dir> {
    property(host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .filter(|d| d.axis() != Axis::Y)
}

fn belt_block(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<belt::BeltBlock> {
    let b = block_at(host, level, pos)?;
    if !b.is_belt() {
        return None;
    }
    Some(belt::BeltBlock {
        heading: heading_at(host, level, pos)?,
    })
}

fn is_elevator(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    block_at(host, level, pos) == Some(Block::BucketElevator)
}

/// A bucket elevator column through `pos`, boot first.
fn column(host: &dyn Host, level: LevelRef, pos: Pos) -> Vec<Pos> {
    let mut bottom = pos;
    let mut guard = 0;
    while is_elevator(host, level, bottom.offset(Dir::Down)) && guard < 512 {
        bottom = bottom.offset(Dir::Down);
        guard += 1;
    }
    let mut out = vec![bottom];
    let mut at = bottom;
    while is_elevator(host, level, at.offset(Dir::Up)) && out.len() < 512 {
        at = at.offset(Dir::Up);
        out.push(at);
    }
    out
}

/// The side a shaft-like part drives `pos` from: a horizontal side among
/// `sides` whose neighbour has a port facing it, turning about `axis`
/// (logistics 3.3, 8.1).
fn drive_side(dim: Dim, pos: Pos, sides: &[Dir], axis: Option<Axis>) -> Option<Dir> {
    with_level(dim, |l| {
        sides.iter().copied().find(|d| {
            let q = pos.offset(*d);
            l.mech.part(q).is_some_and(|p| {
                !matches!(p.spec.kind, crate::sim::rot::Kind::BeltDrive { .. })
                    && p.spec.port(d.opposite()).is_some()
                    && axis.is_none_or(|a| p.spec.axis() == Some(a))
            })
        })
    })
}

// ---- The tick ---------------------------------------------------------------------

/// After the network's tick: rebuild what changed, step every line, sync.
pub fn tick(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let mut belts = with_level(dim, |l| std::mem::take(&mut l.belts));
    belts.ticks += 1;
    unload(host, level, &mut belts);
    diverters(host, level, &mut belts);
    drop_broken(host, level, dim, &mut belts);
    rebuild(host, level, dim, &mut belts);
    check_drives(host, level, dim, &mut belts);
    step(host, level, dim, &mut belts);
    sync(host, level, &mut belts);
    for v in belts.pulses.values_mut() {
        *v = v.saturating_sub(1);
    }
    belts.pulses.retain(|_, v| *v > 0);
    with_level(dim, |l| {
        let queued = std::mem::replace(&mut l.belts, belts);
        l.belts.absorb(queued);
    });
}

/// Lines with a block whose chunk unloaded leave memory (their items were
/// written when they paused).
fn unload(host: &mut dyn ServerHost, level: LevelRef, belts: &mut Belts) {
    let gone = std::mem::take(&mut belts.unloaded);
    for pos in gone {
        if level.is_loaded(&*host, pos.to_array()) {
            continue;
        }
        if let Some(&(li, _)) = belts.by_block.get(&pos)
            && let Some(ls) = belts.lines[li].take()
        {
            if !ls.written {
                write_line(host, level, &ls);
            }
            for b in &ls.line.blocks {
                belts.by_block.remove(b);
            }
        }
    }
}

/// Broken blocks drop the items on them (logistics 4.3).
fn drop_broken(host: &mut dyn ServerHost, level: LevelRef, _dim: Dim, belts: &mut Belts) {
    let broken = std::mem::take(&mut belts.broken);
    for pos in broken {
        belts.restore.remove(&pos);
        let Some(&(li, i)) = belts.by_block.get(&pos) else {
            continue;
        };
        let Some(ls) = belts.lines[li].as_mut() else {
            continue;
        };
        let items = ls.line.take_block(i);
        ls.need_sync = true;
        for stack in items {
            let _ = level.spawn_item(
                host,
                pos.x as f64 + 0.5,
                pos.y as f64 + 0.5,
                pos.z as f64 + 0.5,
                stack,
                10,
            );
        }
    }
}

/// Dissolve the lines around dirty blocks and trace them again.
fn rebuild(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, belts: &mut Belts) {
    if belts.dirty.is_empty() {
        return;
    }
    let dirty = std::mem::take(&mut belts.dirty);
    let mut seeds: BTreeSet<Pos> = BTreeSet::new();
    for &pos in &dirty {
        seeds.insert(pos);
        for d in Dir::ALL {
            seeds.insert(pos.offset(d));
            if d.axis() != Axis::Y {
                seeds.insert(pos.offset(d).offset(Dir::Up));
                seeds.insert(pos.offset(d).offset(Dir::Down));
            }
        }
    }
    // Dissolve: items go back to their blocks' restore lists.
    let mut old_drives: BTreeSet<Pos> = BTreeSet::new();
    for pos in &seeds {
        let Some(&(li, _)) = belts.by_block.get(pos) else {
            continue;
        };
        let Some(ls) = belts.lines[li].take() else {
            continue;
        };
        for (i, b) in ls.line.blocks.iter().enumerate() {
            belts.by_block.remove(b);
            let items = ls.line.entries_in(i);
            if !items.is_empty() {
                belts.restore.entry(*b).or_default().extend(items);
            }
            seeds.iter().for_each(|_| {});
        }
        old_drives.extend(ls.drives.iter().copied());
        // Its blocks are seeds too: they rejoin whatever they make now.
        for b in &ls.line.blocks {
            belts.dirty.insert(*b);
        }
    }
    let more = std::mem::take(&mut belts.dirty);
    seeds.extend(more);
    // Trace.
    let mut new_drives: Vec<Vec<Pos>> = Vec::new();
    for pos in seeds {
        if belts.by_block.contains_key(&pos) {
            continue;
        }
        match block_at(&*host, level, pos) {
            Some(b) if b.is_belt() => {
                let get = |q: Pos| belt_block(&*host, level, q);
                let chain = belt::chain(pos, &get);
                if chain.is_empty() {
                    continue;
                }
                let heading = get(chain[0]).map(|b| b.heading).unwrap_or(Dir::North);
                for (blocks, slopes) in belt::split(&chain) {
                    let li = install(host, level, belts, blocks, slopes, heading, LineKind::Belt);
                    if let Some(ls) = belts.lines[li].as_ref() {
                        new_drives.push(ls.drives.clone());
                    }
                }
            }
            Some(Block::BucketElevator) => {
                let col = column(&*host, level, pos);
                for b in &col {
                    belts.unformed.remove(b);
                }
                let ok = belt::elevator_ok(col.len());
                let parts: Vec<&str> = match ok {
                    Ok(()) => (0..col.len())
                        .map(|i| {
                            if i == 0 {
                                "boot"
                            } else if i + 1 == col.len() {
                                "head"
                            } else {
                                "casing"
                            }
                        })
                        .collect(),
                    Err(_) => vec!["single"; col.len()],
                };
                for (b, part) in col.iter().zip(&parts) {
                    super::world::set_property(host, level, *b, "part", part);
                }
                match ok {
                    Ok(()) => {
                        let head = *col.last().unwrap();
                        let outlet = heading_at(&*host, level, head).unwrap_or(Dir::North);
                        let n = col.len();
                        let li = install(
                            host,
                            level,
                            belts,
                            col,
                            vec![Slope::Flat; n],
                            Dir::Up,
                            LineKind::Elevator { outlet },
                        );
                        if let Some(ls) = belts.lines[li].as_ref() {
                            new_drives.push(ls.drives.clone());
                        }
                    }
                    Err(why) => {
                        for b in col {
                            belts.unformed.insert(b, why);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Ties: untie every old drive, then tie each new line's drives.
    let old: Vec<Pos> = old_drives.into_iter().collect();
    with_level(dim, |l| {
        for d in &old {
            l.mech.set_ties(&[*d], None);
        }
        for group in &new_drives {
            if group.len() > 1 {
                let cause = group.iter().copied().find(|q| dirty.contains(q));
                l.mech.set_ties(group, cause);
            }
        }
    });
    // Saved items for blocks that are no longer belts drop where they were.
    let stale: Vec<Pos> = belts
        .restore
        .keys()
        .copied()
        .filter(|q| !belts.by_block.contains_key(q) && level.is_loaded(&*host, q.to_array()))
        .filter(|q| {
            block_at(&*host, level, *q).is_none_or(|b| !b.is_belt() && b != Block::BucketElevator)
        })
        .collect();
    for q in stale {
        for (stack, _) in belts.restore.remove(&q).unwrap_or_default() {
            let _ = level.spawn_item(
                host,
                q.x as f64 + 0.5,
                q.y as f64 + 0.5,
                q.z as f64 + 0.5,
                stack,
                10,
            );
        }
    }
}

/// Make a line from traced blocks: slopes onto the blocks, items back,
/// drives found. Returns its index.
fn install(
    host: &mut dyn ServerHost,
    level: LevelRef,
    belts: &mut Belts,
    blocks: Vec<Pos>,
    slopes: Vec<Slope>,
    heading: Dir,
    kind: LineKind,
) -> usize {
    if kind == LineKind::Belt {
        for (b, s) in blocks.iter().zip(&slopes) {
            if block_at(&*host, level, *b) == Some(Block::ConveyorBelt) {
                super::world::set_property(host, level, *b, "slope", s.name());
            }
        }
    }
    let mut line = match kind {
        LineKind::Belt => Line::new(blocks.clone(), heading, slopes),
        LineKind::Elevator { .. } => Line::elevator(blocks.clone()),
    };
    for (i, b) in blocks.iter().enumerate() {
        if let Some(saved) = belts.restore.remove(b) {
            for stack in line.restore(i, saved) {
                let _ = level.spawn_item(
                    host,
                    b.x as f64 + 0.5,
                    b.y as f64 + 0.5,
                    b.z as f64 + 0.5,
                    stack,
                    10,
                );
            }
        }
        // The inbox (a hopper's way in) holds one item at a time.
        limit_inbox(host, level, *b);
    }
    let drives: Vec<Pos> = blocks
        .iter()
        .copied()
        .filter(|b| property(&*host, level, *b, "drive").is_some_and(|d| d != "none"))
        .collect();
    let li = match belts.lines.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            belts.lines.push(None);
            belts.lines.len() - 1
        }
    };
    for (i, b) in blocks.iter().enumerate() {
        belts.by_block.insert(*b, (li, i));
    }
    belts.lines[li] = Some(LineState {
        line,
        kind,
        drives,
        stop: None,
        v: 0.0,
        sent_v: f64::NAN,
        sent_stop: None,
        need_sync: true,
        end: EndState::Idle,
        written: false,
        load: 0.0,
    });
    // Every block's entity knows whether it is the controller.
    for (i, b) in blocks.iter().enumerate() {
        let mut c = Compound::new();
        blocks::put_bool(&mut c, "bl_ctl", i == 0);
        if i != 0 {
            c.insert("bl_items".into(), Tag::List(Vec::new()));
        }
        server::merge(host, level, *b, c);
    }
    li
}

/// Set the inbox slot's stack limit to 1 (logistics 4.2: a hopper only gets
/// an item in when the belt has room for it).
fn limit_inbox(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    let Some(cap) = level.capability(host, &mod_api::ITEMS, pos.to_array(), None) else {
        return;
    };
    let _: Result<bool, ()> = host.transaction(|host, tx| Ok(cap.set_max_stack_size(host, tx, 1)));
}

/// Belts whose neighbours changed: find their drive side again.
fn check_drives(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, belts: &mut Belts) {
    if belts.drive_check.is_empty() {
        return;
    }
    let check = std::mem::take(&mut belts.drive_check);
    let mut changed = false;
    for pos in check {
        let Some(b) = block_at(&*host, level, pos) else {
            continue;
        };
        let want = match b {
            Block::ConveyorBelt => {
                let Some(h) = heading_at(&*host, level, pos) else {
                    continue;
                };
                let flat = property(&*host, level, pos, "slope").is_none_or(|s| s == "flat");
                let sides = [h.clockwise(), h.clockwise().opposite()];
                if flat {
                    drive_side(dim, pos, &sides, Some(belt::drive_axis(h)))
                } else {
                    None
                }
            }
            Block::BucketElevator => {
                if property(&*host, level, pos, "part").as_deref() != Some("head") {
                    None
                } else {
                    let outlet = heading_at(&*host, level, pos).unwrap_or(Dir::North);
                    let sides: Vec<Dir> = Dir::HORIZONTAL
                        .into_iter()
                        .filter(|d| *d != outlet)
                        .collect();
                    drive_side(dim, pos, &sides, None)
                }
            }
            _ => continue,
        };
        let value = want.map_or("none", |d| d.name());
        if property(&*host, level, pos, "drive").as_deref() != Some(value) {
            super::world::set_property(host, level, pos, "drive", value);
            changed = true;
            belts.dirty.insert(pos);
        }
    }
    if changed {
        rebuild(host, level, dim, belts);
    }
}

/// The step of every line.
fn step(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, belts: &mut Belts) {
    let view = server::GameView {
        host: &*host,
        level,
    };
    let check_pause = belts.ticks.is_multiple_of(10);
    // Speeds, stops and loads from the network.
    for ls in belts.lines.iter_mut().flatten() {
        let (stop, v, jam_all) = with_level(dim, |l| line_speed(l, ls));
        if !jam_all.is_empty() {
            with_level(dim, |l| {
                for d in &jam_all {
                    l.mech.jam_part(*d, crate::sim::rot::Jam::Shear);
                }
            });
        }
        let mut stop = stop;
        if check_pause || ls.stop == Some(Stop::Paused) {
            let outside = ls
                .line
                .blocks
                .iter()
                .any(|b| !crate::sim::rot::View::simulated(&view, b.chunk()));
            if outside {
                stop = Some(Stop::Paused);
            }
        } else if ls.stop == Some(Stop::Paused) {
            stop = Some(Stop::Paused);
        }
        if stop != ls.stop && matches!(stop, Some(Stop::Jammed | Stop::UnderDriven)) {
            let t = level.game_time(&*host).unwrap_or(0);
            let kind = if stop == Some(Stop::Jammed) {
                "line_jam"
            } else {
                "under_driven"
            };
            super::debug::event(dim, t, kind, ls.line.blocks[0], String::new());
        }
        ls.stop = stop;
        ls.v = if stop.is_some() { 0.0 } else { v };
        let forward = ls.v >= 0.0;
        ls.load = ls.line.load(forward);
        let n = ls.drives.len().max(1) as f64;
        let share = ls.load / n;
        let moving = stop.is_none() || stop == Some(Stop::Still);
        with_level(dim, |l| {
            for d in &ls.drives {
                l.loads.insert(*d, if moving { share } else { 0.0 });
            }
        });
    }
    // Write a line's items when it pauses (its chunks may unload next).
    let pausing: Vec<usize> = belts
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            l.as_ref()
                .filter(|l| l.stop == Some(Stop::Paused) && !l.written)
                .map(|_| i)
        })
        .collect();
    for li in pausing {
        if let Some(ls) = belts.lines[li].as_ref() {
            write_line(host, level, ls);
        }
        if let Some(ls) = belts.lines[li].as_mut() {
            ls.written = true;
        }
    }
    for ls in belts.lines.iter_mut().flatten() {
        if ls.stop != Some(Stop::Paused) {
            ls.written = false;
        }
    }
    // Inboxes, then the step, line by line.
    let n = belts.lines.len();
    for li in 0..n {
        let Some(mut ls) = belts.lines[li].take() else {
            continue;
        };
        if ls.stop != Some(Stop::Paused) {
            drain_inboxes(host, level, &mut ls);
            let stations = stations(host, level, &ls, belts);
            let forward = ls.v >= 0.0;
            let mut world = World {
                host,
                level,
                dim,
                belts,
                kind: ls.kind,
                blocks: ls.line.blocks.clone(),
                heading: ls.line.heading,
                forward,
                speed: ls.v.abs(),
                end: EndState::Idle,
                crossed_items: 0,
                dropped: Vec::new(),
            };
            let out = ls.line.step(ls.v, &stations, &mut world);
            let end = std::mem::take(&mut world.end);
            let dropped = std::mem::take(&mut world.dropped);
            if end != EndState::Idle || ls.line.entries.is_empty() {
                let blocked =
                    |e: &EndState| matches!(e, EndState::BlockedFull | EndState::BlockedSolid);
                if blocked(&end) && !blocked(&ls.end) {
                    let t = level.game_time(&*host).unwrap_or(0);
                    let at = *ls.line.blocks.last().unwrap_or(&ls.line.blocks[0]);
                    super::debug::event(dim, t, "backup", at, format!("{end:?}"));
                }
                ls.end = end;
            }
            for (at, id) in dropped {
                let t = level.game_time(&*host).unwrap_or(0);
                super::debug::event(dim, t, "drop", at, id);
            }
            for i in &out.crossed {
                let pos = ls.line.blocks[*i];
                if block_at(&*host, level, pos) == Some(Block::Diverter) {
                    belts.pulses.insert(pos, belt::DIVERTER_PULSE);
                }
                if let Some(c) = belts.counters.get_mut(&pos) {
                    c.total += 1;
                    c.recent.push_back(belts.ticks);
                    while c.recent.len() > 1200 {
                        c.recent.pop_front();
                    }
                }
            }
            if out.eventful() {
                ls.need_sync = true;
            }
            place_byproducts(host, level, dim, &mut ls);
            carry_entities(host, level, &mut ls);
        }
        belts.lines[li] = Some(ls);
    }
}

/// A line's stop, speed (blocks per tick) and the drives to jam because
/// one of them jammed (a line is one belt, logistics 3.5).
fn line_speed(l: &server::Level, ls: &LineState) -> (Option<Stop>, f64, Vec<Pos>) {
    let jammed: Vec<bool> = ls
        .drives
        .iter()
        .map(|d| l.mech.part(*d).is_some_and(|p| p.jam.is_some()))
        .collect();
    if jammed.iter().any(|j| *j) {
        let rest = ls
            .drives
            .iter()
            .zip(&jammed)
            .filter(|(_, j)| !**j)
            .map(|(d, _)| *d)
            .collect();
        return (Some(Stop::Jammed), 0.0, rest);
    }
    match ls.kind {
        LineKind::Belt => {
            if ls.drives.len() < belt::drives_needed(ls.line.len()) {
                return (Some(Stop::UnderDriven), 0.0, Vec::new());
            }
            let Some((w, paused)) = l.mech.speed(ls.drives[0]) else {
                return (Some(Stop::Still), 0.0, Vec::new());
            };
            if paused {
                return (Some(Stop::Paused), 0.0, Vec::new());
            }
            let v = belt::drive_sign(ls.line.heading) * belt::blocks_per_tick(w);
            (
                if w.abs() < 1e-6 {
                    Some(Stop::Still)
                } else {
                    None
                },
                v,
                Vec::new(),
            )
        }
        LineKind::Elevator { .. } => {
            let Some(d) = ls.drives.first() else {
                return (Some(Stop::Still), 0.0, Vec::new());
            };
            let Some((w, paused)) = l.mech.speed(*d) else {
                return (Some(Stop::Still), 0.0, Vec::new());
            };
            if paused {
                return (Some(Stop::Paused), 0.0, Vec::new());
            }
            // It lifts whichever way it turns (logistics 8.2).
            let v = belt::blocks_per_tick(w.abs());
            (
                if w.abs() < 1e-6 {
                    Some(Stop::Still)
                } else {
                    None
                },
                v,
                Vec::new(),
            )
        }
    }
}

/// Move items from each block's inbox slot onto the line where there is room.
fn drain_inboxes(host: &mut dyn ServerHost, level: LevelRef, ls: &mut LineState) {
    let blocks: Vec<(usize, Pos)> = match ls.kind {
        // Only the boot takes items in.
        LineKind::Elevator { .. } => vec![(0, ls.line.blocks[0])],
        LineKind::Belt => ls.line.blocks.iter().copied().enumerate().collect(),
    };
    for (i, pos) in blocks {
        let Some(entity) = host.block_entity(level, pos.to_array()) else {
            continue;
        };
        let Ok(Some(stack)) = entity.item_slot(&*host, 0) else {
            continue;
        };
        let spot = match ls.kind {
            LineKind::Elevator { .. } => ls.line.free_spot(0.0, 0.0),
            LineKind::Belt => ls.line.spot_near_centre(i),
        };
        let Some(x) = spot else {
            continue;
        };
        let one = stack.clone().split(1);
        let (one, rest) = (one.taken, one.remainder);
        let ok: Result<bool, ()> = host.transaction(|host, tx| {
            Ok(entity
                .set_item_slot(host, tx, 0, rest.clone())
                .unwrap_or(false))
        });
        if ok == Ok(true)
            && let Some(one) = one
        {
            let _ = ls.line.insert(x, one);
            ls.need_sync = true;
        }
    }
}

/// Block indices with a station: diverters, machines working on the belt
/// above, counters.
fn stations(host: &dyn Host, level: LevelRef, ls: &LineState, belts: &Belts) -> Vec<usize> {
    if ls.kind != LineKind::Belt {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, pos) in ls.line.blocks.iter().enumerate() {
        let here = block_at(host, level, *pos);
        let station = here == Some(Block::Diverter)
            || belts.counters.contains_key(pos)
            || (here == Some(Block::ConveyorBelt)
                && ls.line.slopes[i] == Slope::Flat
                && block_at(host, level, pos.offset(Dir::Up))
                    .and_then(Block::machine)
                    .is_some_and(|m| m.works_on_belt()));
        if station {
            out.push(i);
        }
    }
    out
}

/// The world a line's step sees.
struct World<'a> {
    host: &'a mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    belts: &'a mut Belts,
    kind: LineKind,
    blocks: Vec<Pos>,
    heading: Dir,
    forward: bool,
    speed: f64,
    end: EndState,
    crossed_items: u64,
    /// Items that fell off the end this step, for the event log.
    dropped: Vec<(Pos, String)>,
}

impl Stations<ItemStack> for World<'_> {
    fn arrive(&mut self, index: usize, item: &ItemStack) -> Action {
        self.crossed_items += 1;
        let pos = self.blocks[index];
        match block_at(&*self.host, self.level, pos) {
            Some(Block::Diverter) => {
                let powered =
                    property(&*self.host, self.level, pos, "powered").as_deref() == Some("true");
                if !powered {
                    return Action::Pass;
                }
                // Items go off the side away from the flap (logistics 9).
                let flap = flap_dir(&*self.host, self.level, pos, self.heading);
                let out = flap.opposite();
                match deliver(
                    self.host,
                    self.level,
                    self.belts,
                    pos.offset(out),
                    out,
                    item,
                    self.speed,
                ) {
                    Delivery::Taken(_) | Delivery::Fell => {
                        self.belts.stats.diverted += 1;
                        Action::Remove
                    }
                    // A diverter never stops a line (L4).
                    _ => Action::Pass,
                }
            }
            Some(Block::ConveyorBelt) => {
                let above = pos.offset(Dir::Up);
                if super::machines::belt_arrive(&*self.host, self.level, self.dim, above, item) {
                    Action::Hold
                } else {
                    Action::Pass
                }
            }
            _ => Action::Pass,
        }
    }

    fn poll(&mut self, index: usize, item: &mut ItemStack) -> bool {
        let above = self.blocks[index].offset(Dir::Up);
        super::machines::belt_poll(&*self.host, self.dim, above, item)
    }

    fn deliver(&mut self, item: &ItemStack) -> bool {
        let (target, dir) = match self.kind {
            LineKind::Elevator { outlet } => {
                let head = *self.blocks.last().unwrap();
                (head.offset(outlet), outlet)
            }
            LineKind::Belt => {
                if self.forward {
                    (
                        self.blocks[self.blocks.len() - 1].offset(self.heading),
                        self.heading,
                    )
                } else {
                    (
                        self.blocks[0].offset(self.heading.opposite()),
                        self.heading.opposite(),
                    )
                }
            }
        };
        match deliver(
            self.host, self.level, self.belts, target, dir, item, self.speed,
        ) {
            Delivery::Taken(at) => {
                self.end = EndState::Delivering(at);
                self.belts.stats.delivered += 1;
                true
            }
            Delivery::Fell => {
                self.end = EndState::Open;
                self.belts.stats.dropped += 1;
                self.dropped
                    .push((target, item.item().id(&*self.host).to_string()));
                true
            }
            Delivery::Solid => {
                self.end = EndState::BlockedSolid;
                false
            }
            Delivery::Full => {
                self.end = EndState::BlockedFull;
                false
            }
        }
    }
}

/// The side a diverter's flap is on.
pub fn flap_dir(host: &dyn Host, level: LevelRef, pos: Pos, heading: Dir) -> Dir {
    // Left of the heading: counter-clockwise seen from above.
    let left = heading.clockwise().opposite();
    if property(host, level, pos, "flap").as_deref() == Some("right") {
        left.opposite()
    } else {
        left
    }
}

/// What happened to an item offered to a block.
enum Delivery {
    Taken(Pos),
    Fell,
    Solid,
    Full,
}

/// Offer one item to the block at `target`, moving along `dir` (logistics
/// 5.1): another line, an elevator's boot, an inventory, or air.
fn deliver(
    host: &mut dyn ServerHost,
    level: LevelRef,
    belts: &mut Belts,
    target: Pos,
    dir: Dir,
    item: &ItemStack,
    speed: f64,
) -> Delivery {
    if let Some(&(li, i)) = belts.by_block.get(&target)
        && let Some(other) = belts.lines.get_mut(li).and_then(|l| l.as_mut())
    {
        let x = match other.kind {
            LineKind::Elevator { .. } => {
                if i != 0 {
                    return Delivery::Solid;
                }
                Some(0.0)
            }
            LineKind::Belt => {
                let h = other.line.heading;
                if h == dir {
                    // In line, from behind: its block's back edge.
                    Some(i as f64)
                } else if h == dir.opposite() {
                    // Head to head: nothing passes (5.2).
                    return Delivery::Solid;
                } else {
                    Some(i as f64 + 0.5)
                }
            }
        };
        let Some(x) = x else {
            return Delivery::Full;
        };
        let spot = other.line.free_spot(x, belt::ROOM);
        return match spot {
            Some(at) if other.line.insert(at, item.clone()).is_ok() => {
                other.need_sync = true;
                Delivery::Taken(target)
            }
            _ => Delivery::Full,
        };
    }
    // FRAS machines check their own faces (logistics 6).
    if let Some(b) = block_at(&*host, level, target)
        && (b.machine().is_some())
        && !super::machines::takes_from(&*host, level, target, b, dir.opposite())
    {
        return Delivery::Solid;
    }
    if let Some(storage) = level.capability(
        host,
        &STORAGE,
        target.to_array(),
        Some(super::direction(dir.opposite())),
    ) {
        let resource = ItemResource::of(item);
        let moved: Result<u64, ()> =
            host.transaction(|host, tx| Ok(storage.insert(host, tx, &resource, 1)));
        return if moved == Ok(1) {
            Delivery::Taken(target)
        } else {
            Delivery::Full
        };
    }
    let aabb = [
        target.x as f64 + 0.05,
        target.y as f64 + 0.05,
        target.z as f64 + 0.05,
        target.x as f64 + 0.95,
        target.y as f64 + 0.95,
        target.z as f64 + 0.95,
    ];
    if level.has_collisions(&*host, aabb) == Some(true) {
        return Delivery::Solid;
    }
    // Air: it falls off, thrown forward at the belt's speed (L3).
    let [dx, _, dz] = dir.step();
    let from = target.offset(dir.opposite());
    let at = [
        from.x as f64 + 0.5 + dx as f64 * 0.55,
        from.y as f64 + belt::BELT_TOP + 0.15,
        from.z as f64 + 0.5 + dz as f64 * 0.55,
    ];
    let motion = [dx as f64 * speed, 0.05, dz as f64 * speed];
    let _ = level.spawn_item_with_motion(host, at[0], at[1], at[2], item.clone(), motion, 10);
    Delivery::Fell
}

/// Byproducts waiting in belt machines go onto the belt in front of the
/// hold point, behind the main result (logistics 7.3).
fn place_byproducts(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, ls: &mut LineState) {
    if ls.kind != LineKind::Belt {
        return;
    }
    let forward = ls.v >= 0.0;
    for (i, pos) in ls.line.blocks.clone().iter().enumerate() {
        let above = pos.offset(Dir::Up);
        let pending = with_level(dim, |l| {
            l.belt_jobs.get(&above).map_or(0, |j| j.pending.len())
        });
        if pending == 0 {
            continue;
        }
        let c = i as f64 + 0.5;
        let x = if forward {
            c + belt::SPACING
        } else {
            c - belt::SPACING
        };
        if x < 0.0 || x > ls.line.length() || !ls.line.room_at(x) {
            continue;
        }
        let next = with_level(dim, |l| {
            l.belt_jobs.get_mut(&above).and_then(|j| {
                if j.pending.is_empty() {
                    None
                } else {
                    Some(j.pending.remove(0))
                }
            })
        });
        if let Some(stack) = next {
            let _ = ls.line.insert(x, stack);
            ls.need_sync = true;
        }
    }
    let _ = (host, level);
}

/// Players, mobs and dropped items on a moving belt ride it; dropped items
/// get on it (logistics 4.2, 4.4).
fn carry_entities(host: &mut dyn ServerHost, level: LevelRef, ls: &mut LineState) {
    if ls.kind != LineKind::Belt || ls.v.abs() < 1e-9 {
        return;
    }
    let (mut min, mut max) = ([f64::MAX; 3], [f64::MIN; 3]);
    for b in &ls.line.blocks {
        min[0] = min[0].min(b.x as f64);
        min[1] = min[1].min(b.y as f64);
        min[2] = min[2].min(b.z as f64);
        max[0] = max[0].max(b.x as f64 + 1.0);
        max[1] = max[1].max(b.y as f64 + 1.5);
        max[2] = max[2].max(b.z as f64 + 1.0);
    }
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return;
    };
    let [hx, _, hz] = ls.line.heading.step();
    let blocks = ls.line.blocks.clone();
    let along = |p: [f64; 3]| -> Option<(usize, f64)> {
        let feet = Pos::new(
            p[0].floor() as i32,
            (p[1] - 0.05).floor() as i32,
            p[2].floor() as i32,
        );
        let index_of = |q: Pos| blocks.iter().position(|b| *b == q);
        let i = index_of(feet).or_else(|| index_of(feet.offset(Dir::Down)))?;
        let b = blocks[i];
        let f = (p[0] - b.x as f64) * hx as f64 + (p[2] - b.z as f64) * hz as f64;
        let f = if hx + hz > 0 { f } else { 1.0 + f };
        Some((i, i as f64 + f.clamp(0.0, 1.0)))
    };
    let tick = server::with(|s| s.ticks);
    for e in entities {
        let Some(p) = e.position(&*host) else {
            continue;
        };
        let Some((_, x)) = along(p) else {
            continue;
        };
        if let Some(item) = host.item_entity(e) {
            // One item every 4 ticks per block, from the entity's stack.
            if (tick + (x as u64)).is_multiple_of(4)
                && let Some(stack) = item.stack(&*host)
                && let Some(at) = ls.line.free_spot(x, belt::ROOM)
            {
                let parts = stack.split(1);
                if let Some(one) = parts.taken
                    && ls.line.insert(at, one).is_ok()
                {
                    match parts.remainder {
                        Some(rest) => {
                            let _ = item.set_stack(host, rest);
                        }
                        None => {
                            let _ = e.discard(host);
                        }
                    }
                    ls.need_sync = true;
                    continue;
                }
            }
        }
        if e.is_crouching(&*host) == Some(true) {
            continue;
        }
        let Some(v) = e.velocity(&*host) else {
            continue;
        };
        let speed = ls.v;
        let want = [hx as f64 * speed, v[1], hz as f64 * speed];
        // Keep sideways motion, set the belt's along the heading.
        let new = if hx != 0 {
            [want[0], v[1], v[2]]
        } else {
            [v[0], v[1], want[2]]
        };
        let _ = e.set_velocity(host, new);
    }
}

// ---- Saving and syncing ----------------------------------------------------------------

/// Write each block's items into its entity (logistics 10.2).
pub fn write_line(host: &mut dyn ServerHost, level: LevelRef, ls: &LineState) {
    for (i, b) in ls.line.blocks.iter().enumerate() {
        let items: Vec<Tag> = ls
            .line
            .entries_in(i)
            .into_iter()
            .map(|(stack, x)| {
                let mut c = stack_to_nbt(&*host, &stack);
                blocks::put_f64(&mut c, "x", x);
                Tag::Compound(c)
            })
            .collect();
        let mut c = Compound::new();
        c.insert("bitems".into(), Tag::List(items));
        server::merge(host, level, *b, c);
    }
}

/// The level is being saved: write every line's items.
pub fn save(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let belts = with_level(dim, |l| std::mem::take(&mut l.belts));
    for ls in belts.iter() {
        write_line(host, level, ls);
    }
    with_level(dim, |l| {
        let queued = std::mem::replace(&mut l.belts, belts);
        l.belts.absorb(queued);
    });
}

/// Send the controller a new snapshot where clients can't predict it
/// (logistics 10.3).
fn sync(host: &mut dyn ServerHost, level: LevelRef, belts: &mut Belts) {
    let now = level.game_time(&*host).unwrap_or(0);
    for ls in belts.lines.iter_mut().flatten() {
        let speed_moved = {
            let a = ls.sent_v * 640.0;
            let b = ls.v * 640.0;
            a.is_nan() || (a - b).abs() > 0.5f64.max(0.03 * a.abs().max(b.abs()))
        };
        if !(ls.need_sync || speed_moved || ls.sent_stop != ls.stop) {
            continue;
        }
        ls.need_sync = false;
        ls.sent_v = ls.v;
        ls.sent_stop = ls.stop;
        let c = snapshot(&*host, ls, now);
        server::merge(host, level, ls.line.blocks[0], c);
    }
}

fn snapshot(host: &dyn Host, ls: &LineState, now: i64) -> Compound {
    let mut c = Compound::new();
    blocks::put_bool(&mut c, "bl_ctl", true);
    blocks::put_f64(&mut c, "bl_v", ls.v);
    blocks::put_f64(&mut c, "bl_t", now as f64);
    blocks::put_str(&mut c, "bl_stop", ls.stop.map_or("", stop_name));
    let items: Vec<Tag> = ls
        .line
        .entries
        .iter()
        .map(|e| {
            let mut t = stack_to_nbt(host, &e.item);
            blocks::put_f64(&mut t, "x", e.x);
            if e.hold.is_some() {
                blocks::put_bool(&mut t, "h", true);
            }
            Tag::Compound(t)
        })
        .collect();
    c.insert("bl_items".into(), Tag::List(items));
    let origin = ls.line.blocks[0];
    let mut path = Vec::with_capacity(ls.line.len() * 4);
    for (b, s) in ls.line.blocks.iter().zip(&ls.line.slopes) {
        path.push(b.x - origin.x);
        path.push(b.y - origin.y);
        path.push(b.z - origin.z);
        path.push(match s {
            Slope::Flat => 0,
            Slope::Rising => 1,
            Slope::Falling => 2,
        });
    }
    c.insert("bl_path".into(), Tag::IntArray(path));
    blocks::put_bool(
        &mut c,
        "bl_elev",
        matches!(ls.kind, LineKind::Elevator { .. }),
    );
    c
}

pub fn stop_name(s: Stop) -> &'static str {
    match s {
        Stop::UnderDriven => "under_driven",
        Stop::Jammed => "jammed",
        Stop::Paused => "paused",
        Stop::Still => "still",
    }
}

// ---- Players ------------------------------------------------------------------------

/// Right-click a belt: with an item, put one on (sneaking: as many as fit
/// on that block); empty-handed, take the item nearest the click (4.2, 4.3).
pub fn use_on(
    host: &mut dyn ServerHost,
    click: &mod_api::BlockClick<'_>,
) -> Option<mod_api::InteractionResult> {
    let dim = click.dimension.key().id();
    let level = host.level(dim)?;
    let player = host.player(click.player)?;
    let pos = Pos::from_array(click.pos);
    let sneaking = player.is_secondary_use_active(&*host) == Some(true);
    let mut belts = with_level(dim, |l| std::mem::take(&mut l.belts));
    let result = (|| {
        let &(li, i) = belts.by_block.get(&pos)?;
        let ls = belts.lines[li].as_mut()?;
        match click.stack {
            Some(stack) if stack.count() > 0 => {
                let n = if sneaking { 2 } else { 1 };
                let mut put = 0;
                for _ in 0..n {
                    let x = match ls.kind {
                        LineKind::Elevator { .. } => {
                            if i != 0 {
                                break;
                            }
                            ls.line.free_spot(0.0, 0.0)
                        }
                        LineKind::Belt => ls.line.spot_near_centre(i),
                    };
                    let Some(x) = x else { break };
                    let one = stack.clone().split(1).taken?;
                    if ls.line.insert(x, one).is_ok() {
                        put += 1;
                    }
                }
                if put == 0 {
                    return Some(mod_api::InteractionResult::Fail);
                }
                ls.need_sync = true;
                Some(mod_api::InteractionResult::Success)
                    .map(|r| (r, put))
                    .map(|(r, put)| {
                        let _ = take_from_hand(host, player, click.hand, click.stack.cloned(), put);
                        r
                    })
            }
            _ => {
                let hit = click.hit.point;
                let b = ls.line.blocks[i];
                let [hx, _, hz] = ls.line.heading.step();
                let f = (hit[0] - b.x as f64) * hx as f64 + (hit[2] - b.z as f64) * hz as f64;
                let f = if hx + hz > 0 { f } else { 1.0 + f };
                let stack = ls.line.take_nearest(i as f64 + f.clamp(0.0, 1.0))?;
                ls.need_sync = true;
                let _ = player.give_stack(host, stack);
                Some(mod_api::InteractionResult::Success)
            }
        }
    })();
    with_level(dim, |l| {
        let queued = std::mem::replace(&mut l.belts, belts);
        l.belts.absorb(queued);
    });
    let _ = level;
    result
}

fn take_from_hand(
    host: &mut dyn ServerHost,
    player: mod_api::PlayerRef,
    hand: mod_api::Hand,
    stack: Option<ItemStack>,
    n: u32,
) -> bool {
    if super::tools::creative(&*host, player) {
        return true;
    }
    let Some(stack) = stack else {
        return false;
    };
    let slot = super::tools::hand_slot(&*host, player, hand);
    let _ = player.set_inventory_slot(host, slot, stack.shrink(n));
    true
}

/// A diverter's comparator signal: 15 for a moment after each item.
pub fn diverter_signal(dim: Dim, pos: Pos) -> i32 {
    with_level(dim, |l| {
        if l.belts.pulses.contains_key(&pos) {
            15
        } else {
            0
        }
    })
}

/// Lines through a set of blocks, for the wrench and debug tools.
pub fn line_index(dim: Dim, pos: Pos) -> Option<usize> {
    with_level(dim, |l| l.belts.by_block.get(&pos).map(|(li, _)| *li))
}
