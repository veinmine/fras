//! The 0.4 machines on Horizon (machines spec 1.1).
//!
//! **Line machines** (washer, trip hammer, lathe, sawmill, inline grinder)
//! are two blocks tall: the machine on top with a built-in belt from its back
//! (input) to its front (output, `facing`), the drive below with one socket.
//! They have no slots: a one-item inbox takes items from belts, hoppers and
//! hands, and the built-in belt (`sim::machine::station`) carries them
//! through and hands them on at the output with a belt end's rules.
//!
//! **The trommel** is three trommel blocks in a row, formed like a boiler;
//! its drum lives in the middle block, items come in at either end, and the
//! outlets under the three blocks are counted from the end an item entered.
//!
//! Stations and drums are kept in memory (`Queue`) and written to their
//! block entity when their items change.

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;

use mod_api::{
    BlockClick, BlockEntityTickEvent, BlockEntityTickReply, ClientHost, Compound, Host,
    HostBlockStates, HostTransaction, Ingredient, InteractionResult, ItemResource, ItemStack,
    LevelRef, ModContainer, STORAGE, ScreenDrawEvent, ScreenEvent, ServerHost, Tag,
};

use super::blocks::{self, Block};
use super::grinder::{HostItems, item_key, random};
use super::server::{self, Dim, block_at, property, with_level};
use crate::sim::fluid::Content;
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::grind::{self, MachineState, Recipe, Status};
use crate::sim::machine::station::{self, Station};
use crate::sim::machine::{self, Drum, Machine, Size, Tank, Tumbling};

/// The container of a 0.4 machine: a one-item inbox every face reaches, no
/// outputs, and a menu that shows information only (machines spec 2.5).
pub fn container(b: Block) -> ModContainer {
    ModContainer {
        slots: 1,
        faces: [vec![0], vec![0], vec![0], vec![0], vec![0], vec![0]],
        inputs: vec![0],
        outputs: vec![],
        menu: Some(format!("fras:{}", b.name())),
    }
}

// ---- Recipes -------------------------------------------------------------------

#[derive(Default)]
struct Books {
    by: HashMap<&'static str, Vec<Recipe>>,
    /// Tag ingredients and the trommel's size tags, for matching.
    tags: HashMap<String, Ingredient>,
}

static BOOKS: RwLock<Option<Books>> = RwLock::new(None);

pub fn clear() {
    *BOOKS.write().unwrap_or_else(|e| e.into_inner()) = None;
}

fn add_tag(host: &dyn Host, books: &mut Books, tag: &str) {
    if books.tags.contains_key(tag) {
        return;
    }
    if let Ok(Some(ing)) =
        host.recipe_ingredient(&mod_api::serde_json::json!(format!("#{tag}")).to_string())
    {
        books.tags.insert(tag.to_owned(), ing);
    }
}

/// Read every machine's recipes after the server's data loaded or reloaded.
/// A machine whose type has no recipe files falls back on the built-in
/// table, so a broken datapack still leaves the machines working.
pub fn load(host: &dyn Host) {
    let mut books = Books::default();
    for m in Machine::ALL {
        let Some(kind) = m.recipe_type() else {
            continue;
        };
        if m == Machine::Grinder {
            continue;
        }
        let mut list = Vec::new();
        for r in host.recipes_of_type(kind).iter() {
            match super::grinder::parse_for(&r.id, &r.json, m.default_load(), m.default_water()) {
                // A line machine works on one item at a time (machines spec 2.1).
                Ok(recipe) if recipe.count > 1 => {
                    eprintln!("[fras] {kind} recipe {} skipped: count above 1", r.id);
                }
                Ok(recipe) => list.push(recipe),
                Err(e) => eprintln!("[fras] {kind} recipe {} skipped: {e}", r.id),
            }
        }
        if list.is_empty() {
            list = machine::builtin(m);
        }
        for (a, b) in grind::duplicates(&list) {
            eprintln!("[fras] {kind} recipes {a} and {b} take the same input; the first wins");
        }
        for r in &list {
            if let grind::Ingredient::Tag(tag) = &r.ingredient {
                add_tag(host, &mut books, tag);
            }
        }
        books.by.insert(m.name(), list);
    }
    for tag in [machine::TAG_FINE, machine::TAG_MEDIUM] {
        add_tag(host, &mut books, tag);
    }
    *BOOKS.write().unwrap_or_else(|e| e.into_inner()) = Some(books);
}

/// Run `f` with a machine's recipes and the item facts.
fn with_book<R>(host: &dyn Host, m: Machine, f: impl FnOnce(&[Recipe], &HostItems<'_>) -> R) -> R {
    let books = BOOKS.read().unwrap_or_else(|e| e.into_inner());
    let empty = Books::default();
    let books = books.as_ref().unwrap_or(&empty);
    let builtin;
    let recipes: &[Recipe] = match books.by.get(m.name()) {
        Some(r) => r,
        None => {
            builtin = machine::builtin(m);
            &builtin
        }
    };
    let items = HostItems {
        host,
        tags: &books.tags,
    };
    f(recipes, &items)
}

/// Whether a machine has a recipe for `stack` (one item in).
pub fn has_recipe(host: &dyn Host, m: Machine, stack: &ItemStack) -> bool {
    if m == Machine::Trommel {
        return true;
    }
    let id = stack.item().id(host);
    with_book(host, m, |recipes, items| {
        machine::belt_accepts(&id, recipes, items)
    })
}

/// The recipes a machine runs, for the debug tools.
pub fn recipes(host: &dyn Host, m: Machine) -> Vec<Recipe> {
    with_book(host, m, |r, _| r.to_vec())
}

fn make(host: &dyn Host, id: &str, n: u32) -> Option<ItemStack> {
    ItemStack::new(item_key(host, id)?, n)
}

// ---- Shared state -------------------------------------------------------------------

/// The 0.4 machines' state in a level, and work hooks queue for the ticks.
#[derive(Default)]
pub struct Queue {
    /// Water poured in by hand, mB, by washer (top block).
    pub fill: HashMap<Pos, f64>,
    /// Trommel blocks placed or broken: their rows form again.
    pub trommels: BTreeSet<Pos>,
    /// Line machine halves placed (true) or broken (false).
    pub lines: Vec<(Pos, bool)>,
    /// Drives and trommels whose neighbours changed: their socket may move.
    pub sockets: BTreeSet<Pos>,
    /// Recipes finished (items sorted, for the trommel) per machine, for the debug report.
    pub made: HashMap<Pos, u64>,
    /// Items handed on from a drive's or a trommel end's inbox, by the block
    /// that takes them (a machine's top, a trommel's middle), with the end
    /// they came in at (true: the low end).
    pub feed: HashMap<Pos, Vec<(ItemStack, bool)>>,
    /// Room each of those has for fed items, as of its last tick.
    pub room: HashMap<Pos, usize>,
    /// Line machines' built-in belts, by top block.
    pub stations: HashMap<Pos, Station<ItemStack>>,
    /// Trommels' drums, by middle block.
    pub drums: HashMap<Pos, Drum<ItemStack>>,
    /// Comparator signals by block.
    pub signals: HashMap<Pos, i32>,
}

fn rot_state(dim: Dim, pos: Pos, nbt: Option<&Compound>) -> (f64, bool, bool) {
    let saved = nbt.and_then(blocks::saved);
    with_level(dim, |l| match l.mech.part(pos) {
        Some(p) => {
            let (w, paused) = l.mech.speed(pos).unwrap_or((p.omega, true));
            (w, paused, p.jam.is_some())
        }
        None => (
            saved.map_or(0.0, |s| s.omega),
            true,
            saved.is_some_and(|s| s.jam.is_some()),
        ),
    })
}

fn dir_prop(host: &dyn Host, level: LevelRef, pos: Pos, key: &str) -> Option<Dir> {
    property(host, level, pos, key)
        .as_deref()
        .and_then(Dir::from_name)
}

/// Whether the top block of a line machine is at `pos`.
fn is_top(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    property(host, level, pos, "half").as_deref() == Some("upper")
}

/// The block that is the machine's part on the network: a line machine's
/// drive, a formed trommel's socket block (the wrench and the debug tools
/// stand there).
pub fn part_pos(host: &dyn Host, level: LevelRef, pos: Pos, b: Block) -> Option<Pos> {
    if b.is_line_machine() {
        return Some(if is_top(host, level, pos) {
            pos.offset(Dir::Down)
        } else {
            pos
        });
    }
    if b == Block::Trommel {
        let row = trommel_row(host, level, pos)?;
        return row
            .into_iter()
            .find(|q| property(host, level, *q, "socket").is_some_and(|s| s != "none"));
    }
    None
}

/// The block that keeps the machine's state: a line machine's top, a formed
/// trommel's middle.
pub fn state_pos(host: &dyn Host, level: LevelRef, pos: Pos, b: Block) -> Option<Pos> {
    if b.is_line_machine() {
        return Some(if is_top(host, level, pos) {
            pos
        } else {
            pos.offset(Dir::Up)
        });
    }
    if b == Block::Trommel {
        return trommel_row(host, level, pos).map(|r| r[1]);
    }
    None
}

/// Whether the block beyond `pos`'s face `d` is a network part with a port
/// facing back: a shaft end against that face.
fn shaft_against(dim: Dim, pos: Pos, d: Dir) -> bool {
    with_level(dim, |l| {
        l.mech
            .part(pos.offset(d))
            .is_some_and(|p| p.spec.port(d.opposite()).is_some())
    })
}

/// Whether the machine block at `pos` takes items from a belt end or chute
/// arriving through its face `face` (machines spec 2.4, 8.2, 9).
pub fn takes_from(host: &dyn Host, level: LevelRef, pos: Pos, b: Block, face: Dir) -> bool {
    match b {
        Block::Grinder => {
            let facing = dir_prop(host, level, pos, "facing").unwrap_or(Dir::North);
            face != Dir::Down && face != facing
        }
        Block::Trommel => {
            let Some(row) = trommel_row(host, level, pos) else {
                return false;
            };
            let axis = trommel_axis(&row);
            (pos == row[0] && face == axis_dir(axis).opposite())
                || (pos == row[2] && face == axis_dir(axis))
                || ((pos == row[0] || pos == row[2]) && face == Dir::Up)
        }
        b if b.is_line_machine() => {
            if !is_top(host, level, pos) {
                return false;
            }
            let facing = dir_prop(host, level, pos, "facing").unwrap_or(Dir::North);
            face == facing.opposite() || face == Dir::Up
        }
        _ => true,
    }
}

// ---- Line machines -----------------------------------------------------------------------

struct StationWorld<'a> {
    host: &'a dyn Host,
}

impl station::World<ItemStack> for StationWorld<'_> {
    fn id(&self, item: &ItemStack) -> String {
        item.item().id(self.host).to_string()
    }
    fn make(&self, id: &str) -> Option<ItemStack> {
        make(self.host, id, 1)
    }
}

fn read_station(host: &dyn Host, nbt: &Compound) -> Station<ItemStack> {
    let mut st = Station::default();
    if let Some(Tag::List(list)) = nbt.get(&"st".into()) {
        for t in list {
            if let Tag::Compound(c) = t
                && let Some(Tag::Compound(i)) = c.get(&"i".into())
                && let Some(item) = super::belts::stack_from_nbt(host, i)
            {
                st.entries.push(station::Entry {
                    item,
                    x: blocks::get_f64(c, "x").unwrap_or(0.0).clamp(0.0, 1.0),
                    done: blocks::get_bool(c, "d"),
                });
            }
        }
    }
    st.entries.sort_by(|a, b| a.x.total_cmp(&b.x));
    st.entries.truncate(station::CAPACITY);
    if let Some(Tag::List(list)) = nbt.get(&"stp".into()) {
        for t in list {
            if let Tag::Compound(c) = t
                && let Some(item) = super::belts::stack_from_nbt(host, c)
            {
                st.pending.push(item);
            }
        }
    }
    st.work.progress = blocks::get_f64(nbt, "progress").unwrap_or(0.0);
    st.work.recipe = blocks::get_str(nbt, "recipe").filter(|s| !s.is_empty());
    st
}

/// Write a station into its block entity data: its items (for saving and
/// for the client, which draws them moving at `v` blocks per tick from
/// game time `now`).
fn write_station(host: &dyn Host, nbt: &mut Compound, st: &Station<ItemStack>, v: f64, now: i64) {
    let list: Vec<Tag> = st
        .entries
        .iter()
        .map(|e| {
            let mut c = Compound::new();
            c.insert(
                "i".into(),
                Tag::Compound(super::belts::stack_to_nbt(host, &e.item)),
            );
            blocks::put_f64(&mut c, "x", e.x);
            blocks::put_bool(&mut c, "d", e.done);
            Tag::Compound(c)
        })
        .collect();
    nbt.insert("st".into(), Tag::List(list));
    let pending: Vec<Tag> = st
        .pending
        .iter()
        .map(|s| Tag::Compound(super::belts::stack_to_nbt(host, s)))
        .collect();
    nbt.insert("stp".into(), Tag::List(pending));
    blocks::put_f64(nbt, "progress", st.work.progress);
    blocks::put_str(nbt, "recipe", st.work.recipe.as_deref().unwrap_or(""));
    blocks::put_f64(nbt, "st_v", v);
    nbt.insert("st_t".into(), Tag::Long(now));
}

/// The comparator signal of a built-in belt (machines spec 2.4).
fn station_signal(st: &Station<ItemStack>, blocked: bool) -> i32 {
    match (st.entries.len(), blocked) {
        (_, true) => 15,
        (0, _) => 0,
        (1, _) => 7,
        _ => 11,
    }
}

fn read_tank(nbt: &Compound) -> Tank {
    Tank {
        content: blocks::get_str(nbt, "tank_c")
            .as_deref()
            .and_then(Content::from_name),
        amount: blocks::get_f64(nbt, "tank").unwrap_or(0.0),
    }
}

/// One item from a block entity's inbox, if any.
fn take_inbox(items: &mut [Option<ItemStack>]) -> Option<ItemStack> {
    let slot = items.get_mut(0)?;
    let stack = slot.take()?;
    let parts = stack.split(1);
    *slot = parts.remainder;
    parts.taken
}

pub fn tick(b: Block, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    if b == Block::Trommel {
        trommel_tick(host, event);
        return;
    }
    let Some(m) = b.machine() else {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    };
    let dim = host.dimension();
    let Some(level) = host.level(dim) else {
        return;
    };
    let pos = Pos::from_array(event.pos);
    if !is_top(&*host, level, pos) {
        // The drive: hand what a hopper put in its inbox to the top.
        forward_inbox(dim, pos, pos.offset(Dir::Up), true, event);
        return;
    }
    line_tick(m, host, level, dim, pos, event);
}

/// Pass the inbox's item to the block that works on it (a machine's top, a
/// trommel's middle) if that has room.
fn forward_inbox(
    dim: Dim,
    pos: Pos,
    to: Pos,
    from_low: bool,
    event: &mut BlockEntityTickEvent<'_>,
) {
    let room = with_level(dim, |l| l.machines.room.get(&to).copied().unwrap_or(0));
    let queued = with_level(dim, |l| l.machines.feed.get(&to).map_or(0, Vec::len));
    if room > queued
        && let Some(one) = take_inbox(&mut event.entity.items)
    {
        with_level(dim, |l| {
            l.machines.feed.entry(to).or_default().push((one, from_low));
        });
        let _ = pos;
        return;
    }
    *event.reply = BlockEntityTickReply::Unchanged;
}

fn line_tick(
    m: Machine,
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    event: &mut BlockEntityTickEvent<'_>,
) {
    let drive = pos.offset(Dir::Down);
    let facing = dir_prop(&*host, level, pos, "facing").unwrap_or(Dir::North);
    let drive_nbt = host
        .block_entity(level, drive.to_array())
        .and_then(|e| e.data(&*host));
    let (rpm, paused, jammed) = rot_state(dim, drive, drive_nbt.as_ref());
    let now = server::with(|s| s.ticks);
    let game_now = level.game_time(&*host).unwrap_or(0);
    let old_nbt = event.entity.nbt.clone();
    let before_items = event.entity.items.clone();
    let mut st = with_level(dim, |l| l.machines.stations.remove(&pos))
        .unwrap_or_else(|| read_station(&*host, &event.entity.nbt));
    let mut changed = false;

    // In: the inbox (belt ends, hoppers, hands), then what the drive passed up.
    if st.has_room()
        && let Some(one) = take_inbox(&mut event.entity.items)
    {
        let _ = st.insert(one);
        changed = true;
    }
    let fed = with_level(dim, |l| l.machines.feed.remove(&pos)).unwrap_or_default();
    for (stack, _) in fed {
        if let Err(back) = st.insert(stack) {
            // No room after all: back where it came from.
            let _ = level.spawn_item(
                host,
                pos.x as f64 + 0.5,
                pos.y as f64 + 1.1,
                pos.z as f64 + 0.5,
                back,
                10,
            );
        } else {
            changed = true;
        }
    }

    // The washer's tank: water from pipes and buckets (machines spec 4.3).
    let mut tank = read_tank(&event.entity.nbt);
    if m == Machine::Washer {
        let (piped, poured) = with_level(dim, |l| {
            (
                l.steam.works.take_washed(pos),
                l.machines.fill.remove(&pos).unwrap_or(0.0),
            )
        });
        if let Some((content, amount)) = piped {
            tank.fill(content, amount);
        }
        if poured > 0.0 {
            tank.fill(Content::Water, poured);
        }
    }
    let state = MachineState {
        rpm,
        jammed,
        paused,
        range: m.range(),
        water: (m == Machine::Washer).then_some(tank.amount),
    };
    let speed = crate::sim::belt::blocks_per_tick(rpm).abs();
    let moving = speed > 1e-9 && !jammed && !paused;

    // Out: the item at the output end, offered every few ticks (5.1).
    if moving
        && let Some(item) = st.ready().cloned()
        && (now + pos.x as u64).is_multiple_of(crate::sim::belt::RETRY_TICKS as u64)
        && super::belts::offer(
            host,
            level,
            dim,
            pos.offset(facing),
            facing,
            &item,
            speed * 20.0,
        )
    {
        st.pop_ready();
        changed = true;
    }

    let t = with_book(&*host, m, |recipes, items| {
        let mut world = StationWorld { host: items.host };
        st.tick(&mut world, &state, recipes, items, &mut random, speed)
    });
    changed |= t.changed;
    tank.drain(t.water_used);
    if t.completed {
        with_level(dim, |l| *l.machines.made.entry(pos).or_default() += 1);
        super::debug::event(dim, game_now, "made", pos, m.name().to_string());
    }
    let held = st.held().map(|e| e.item.item().id(&*host).to_string());
    let working = held.is_some() && t.status == Status::Grinding;
    let blocked = t.status == Status::OutputFull;
    with_level(dim, |l| {
        l.loads.insert(drive, t.load);
        l.machines.room.insert(pos, usize::from(st.has_room()));
        let signal = station_signal(&st, blocked);
        l.machines.signals.insert(pos, signal);
        l.machines.signals.insert(drive, signal);
        if m == Machine::Washer {
            let demand = tank.space().min(machine::WASHER_TANK);
            l.steam.works.set_wash_demand(pos, demand, tank.content);
        }
    });

    // Strikes and the blade (machines spec 5.3, 5.4, 7.3).
    let mut phase = blocks::get_f64(&event.entity.nbt, "m_ph").unwrap_or(0.0);
    let mut hit = blocks::get_i32(&event.entity.nbt, "m_hit").unwrap_or(0);
    if working && !paused {
        let (min, max) = m.range();
        let revs = grind::progress_per_tick(rpm, min, max);
        let before_strikes = (phase * m.strikes_per_rev()).floor();
        phase = (phase + revs) % 1024.0;
        let after = (phase * m.strikes_per_rev()).floor();
        if m == Machine::TripHammer && after != before_strikes {
            hit = hit.wrapping_add(1);
            super::effects::sound(host, level, pos, "minecraft:block.anvil.land", 0.35, 0.7);
            let min = [pos.x as f64 + 0.2, pos.y as f64, pos.z as f64 + 0.2];
            let max = [pos.x as f64 + 0.8, pos.y as f64 + 0.8, pos.z as f64 + 0.8];
            hurt_in(host, level, min, max, machine::HAMMER_DAMAGE);
        }
    }
    if m == Machine::Sawmill
        && !paused
        && rpm.abs() >= m.range().0
        && (now + pos.x as u64).is_multiple_of(machine::SAW_DAMAGE_EVERY)
    {
        let min = [pos.x as f64 - 0.05, pos.y as f64, pos.z as f64 - 0.05];
        let max = [pos.x as f64 + 1.05, pos.y as f64 + 1.3, pos.z as f64 + 1.05];
        hurt_in(host, level, min, max, machine::SAW_DAMAGE);
    }

    // Menu data: progress ‰, RPM × 10, status, max RPM, tank mB, 0, min
    // RPM, strikes needed.
    let progress_milli = if t.needed > 0.0 {
        (st.work.progress / t.needed * 1000.0) as i32
    } else {
        0
    };
    let (min, max) = m.range();
    let data = vec![
        progress_milli,
        (rpm * 10.0).round() as i32,
        t.status.code(),
        max as i32,
        tank.amount.round() as i32,
        0,
        min as i32,
        (t.needed * m.strikes_per_rev()).round() as i32,
    ];
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let shown = held.unwrap_or_default();
    let v = if moving { speed } else { 0.0 };
    let nbt = &mut event.entity.nbt;
    let sent_v = blocks::get_f64(nbt, "st_v").unwrap_or(0.0);
    let speed_changed = (sent_v - v).abs() > 0.05 * v.abs().max(0.002);
    let sent_rpm = blocks::get_f64(nbt, "m_rpm").unwrap_or(0.0);
    let rpm_changed = (sent_rpm - rpm).abs() > 0.05 * rpm.abs().max(1.0);
    let client_changed = changed
        || speed_changed
        || rpm_changed
        || blocks::get_bool(nbt, "working") != working
        || blocks::get_str(nbt, "m_item").unwrap_or_default() != shown
        || blocks::get_i32(nbt, "m_hit").unwrap_or(0) != hit
        || (m == Machine::Washer
            && (blocks::get_f64(nbt, "tank").unwrap_or(0.0) - tank.amount).abs() >= 50.0);
    if client_changed {
        write_station(&*host, nbt, &st, v, game_now);
        blocks::put_f64(nbt, "m_rpm", rpm);
        blocks::put_bool(nbt, "working", working);
        blocks::put_str(nbt, "m_item", &shown);
        blocks::put_i32(nbt, "m_hit", hit);
        if m == Machine::Washer {
            blocks::put_f64(nbt, "tank", tank.amount);
            blocks::put_str(nbt, "tank_c", tank.content.map_or("", Content::name));
        }
    }
    // Kept in memory between writes: the phase and the exact tank.
    blocks::put_f64(nbt, "m_ph", phase);
    if m == Machine::Washer {
        blocks::put_f64(nbt, "tank", tank.amount);
        blocks::put_str(nbt, "tank_c", tank.content.map_or("", Content::name));
    }
    with_level(dim, |l| l.machines.stations.insert(pos, st));
    if client_changed {
        host.block_entity_sync(event.pos);
    }
    if !(data_changed || event.entity.nbt != old_nbt || event.entity.items != before_items) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

// ---- The trommel -------------------------------------------------------------------------

/// The unit step along an axis, toward the high end.
fn axis_dir(axis: Axis) -> Dir {
    axis.positive()
}

fn trommel_axis(row: &[Pos; 3]) -> Axis {
    if row[0].x != row[1].x {
        Axis::X
    } else {
        Axis::Z
    }
}

/// The formed trommel `pos` belongs to, low end first, as its block states
/// say (`part`, `axis`).
pub fn trommel_row(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<[Pos; 3]> {
    if block_at(host, level, pos) != Some(Block::Trommel) {
        return None;
    }
    let part = property(host, level, pos, "part")?;
    let axis = property(host, level, pos, "axis")
        .as_deref()
        .and_then(Axis::from_name)?;
    let d = axis_dir(axis);
    let mid = match part.as_str() {
        "middle" => pos,
        "end" => {
            let up = pos.offset(d);
            let down = pos.offset(d.opposite());
            if property(host, level, up, "part").as_deref() == Some("middle")
                && block_at(host, level, up) == Some(Block::Trommel)
            {
                up
            } else if property(host, level, down, "part").as_deref() == Some("middle")
                && block_at(host, level, down) == Some(Block::Trommel)
            {
                down
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some([mid.offset(d.opposite()), mid, mid.offset(d)])
}

/// Which trommel blocks form: every straight run of exactly three along x
/// or z whose blocks touch no other trommel block across it (machines spec
/// 8.2). `is` says whether a block is a trommel block. Returns each formed
/// row, low end first, and the axis.
pub fn trommel_rows(is: &dyn Fn(Pos) -> bool, near: &[Pos]) -> Vec<([Pos; 3], Axis)> {
    let mut out: Vec<([Pos; 3], Axis)> = Vec::new();
    for &p in near {
        if !is(p) {
            continue;
        }
        for axis in [Axis::X, Axis::Z] {
            let d = axis_dir(axis);
            // The low end of the run through p.
            let mut lo = p;
            while is(lo.offset(d.opposite())) {
                lo = lo.offset(d.opposite());
            }
            let mut run = vec![lo];
            while is(run[run.len() - 1].offset(d)) && run.len() <= 3 {
                run.push(run[run.len() - 1].offset(d));
            }
            if run.len() != 3 {
                continue;
            }
            let across = if axis == Axis::X {
                Dir::South
            } else {
                Dir::East
            };
            let alone = run
                .iter()
                .all(|q| !is(q.offset(across)) && !is(q.offset(across.opposite())));
            let row = [run[0], run[1], run[2]];
            if alone && !out.iter().any(|(r, _)| *r == row) {
                out.push((row, axis));
            }
        }
    }
    out
}

/// Why a trommel block doesn't form, for the wrench (8.2).
pub fn trommel_unformed(host: &dyn Host, level: LevelRef, pos: Pos) -> &'static str {
    let is = |q: Pos| block_at(host, level, q) == Some(Block::Trommel);
    let longest = [Axis::X, Axis::Z]
        .into_iter()
        .map(|a| {
            let d = axis_dir(a);
            let mut n = 1;
            let mut q = pos.offset(d);
            while is(q) && n < 9 {
                n += 1;
                q = q.offset(d);
            }
            let mut q = pos.offset(d.opposite());
            while is(q) && n < 9 {
                n += 1;
                q = q.offset(d.opposite());
            }
            n
        })
        .max()
        .unwrap_or(1);
    if longest > 3 {
        "fras.trommel.too_long"
    } else if longest == 3 {
        "fras.trommel.crowded"
    } else {
        "fras.trommel.needs_three"
    }
}

fn read_drum(host: &dyn Host, nbt: &Compound) -> Drum<ItemStack> {
    let mut drum = Drum::default();
    if let Some(Tag::List(list)) = nbt.get(&"drum".into()) {
        for t in list {
            if let Tag::Compound(c) = t
                && let Some(Tag::Compound(i)) = c.get(&"i".into())
                && let Some(item) = super::belts::stack_from_nbt(host, i)
            {
                let size = match blocks::get_i32(c, "s").unwrap_or(2) {
                    0 => Size::Fine,
                    1 => Size::Medium,
                    _ => Size::Oversize,
                };
                drum.items.push(Tumbling {
                    item,
                    size,
                    progress: blocks::get_f64(c, "p").unwrap_or(0.0),
                    from_low: blocks::get_bool(c, "lo"),
                });
            }
        }
    }
    for (k, out) in drum.outlets.iter_mut().enumerate() {
        if let Some(Tag::List(list)) = nbt.get(&format!("out{k}").as_str().into()) {
            for t in list {
                if let Tag::Compound(c) = t
                    && let Some(item) = super::belts::stack_from_nbt(host, c)
                {
                    out.push(item);
                }
            }
        }
    }
    drum
}

fn write_drum(host: &dyn Host, nbt: &mut Compound, drum: &Drum<ItemStack>) {
    let list: Vec<Tag> = drum
        .items
        .iter()
        .map(|t| {
            let mut c = Compound::new();
            c.insert(
                "i".into(),
                Tag::Compound(super::belts::stack_to_nbt(host, &t.item)),
            );
            blocks::put_i32(&mut c, "s", t.size.index() as i32);
            blocks::put_f64(&mut c, "p", t.progress);
            blocks::put_bool(&mut c, "lo", t.from_low);
            Tag::Compound(c)
        })
        .collect();
    nbt.insert("drum".into(), Tag::List(list));
    for (k, out) in drum.outlets.iter().enumerate() {
        let list: Vec<Tag> = out
            .iter()
            .map(|s| Tag::Compound(super::belts::stack_to_nbt(host, s)))
            .collect();
        nbt.insert(format!("out{k}").as_str().into(), Tag::List(list));
    }
}

/// Push one item into the block at `target` from above, or drop it into
/// the air there (8.2). Whether it left.
fn push_down(host: &mut dyn ServerHost, level: LevelRef, target: Pos, item: &ItemStack) -> bool {
    if let Some(storage) = level.capability(
        host,
        &STORAGE,
        target.to_array(),
        Some(super::direction(Dir::Up)),
    ) {
        let resource = ItemResource::of(item);
        let moved: Result<u64, ()> =
            host.transaction(|host, tx| Ok(storage.insert(host, tx, &resource, 1)));
        return moved == Ok(1);
    }
    let aabb = [
        target.x as f64 + 0.05,
        target.y as f64 + 0.05,
        target.z as f64 + 0.05,
        target.x as f64 + 0.95,
        target.y as f64 + 0.95,
        target.z as f64 + 0.95,
    ];
    if level.has_collisions(&*host, aabb) != Some(false) {
        return false;
    }
    let _ = level.spawn_item(
        host,
        target.x as f64 + 0.5,
        target.y as f64 + 0.7,
        target.z as f64 + 0.5,
        item.clone(),
        10,
    );
    true
}

fn trommel_tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let Some(level) = host.level(dim) else {
        return;
    };
    let pos = Pos::from_array(event.pos);
    let Some(row) = trommel_row(&*host, level, pos) else {
        // Unformed: nothing works; the inbox keeps what it has.
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    };
    if pos != row[1] {
        // An end: hand what came in to the middle.
        forward_inbox(dim, pos, row[1], pos == row[0], event);
        return;
    }
    let m = Machine::Trommel;
    let socket_block = row
        .iter()
        .copied()
        .find(|q| property(&*host, level, *q, "socket").is_some_and(|s| s != "none"));
    let (rpm, paused, jammed) = match socket_block {
        Some(q) => {
            let nbt = host
                .block_entity(level, q.to_array())
                .and_then(|e| e.data(&*host));
            rot_state(dim, q, nbt.as_ref())
        }
        None => (0.0, false, false),
    };
    let now = server::with(|s| s.ticks);
    let game_now = level.game_time(&*host).unwrap_or(0);
    let old_nbt = event.entity.nbt.clone();
    let mut drum = with_level(dim, |l| l.machines.drums.remove(&pos))
        .unwrap_or_else(|| read_drum(&*host, &event.entity.nbt));
    let mut changed = false;
    let state = MachineState {
        rpm,
        jammed,
        paused,
        range: m.range(),
        water: None,
    };
    let fed = with_level(dim, |l| l.machines.feed.remove(&pos)).unwrap_or_default();
    // The middle's own inbox (a hopper above it) counts as the low end.
    let own = take_inbox(&mut event.entity.items).map(|s| (s, true));
    let result = with_book(&*host, m, |_, items| {
        for (stack, from_low) in fed.into_iter().chain(own) {
            let id = stack.item().id(items.host).to_string();
            match drum.insert(stack, &id, from_low, items) {
                Ok(()) => changed = true,
                Err(back) => {
                    // Full after all: it waits in the middle's inbox.
                    event.entity.items[0] = Some(back);
                }
            }
        }
        drum.tick(&state)
    });
    if result.sorted > 0 {
        changed = true;
        with_level(dim, |l| {
            *l.machines.made.entry(pos).or_default() += result.sorted as u64
        });
    }
    // Every 8 ticks each outlet pushes one into what's below it (8.2).
    if (now + pos.x as u64 + pos.z as u64).is_multiple_of(grind::TRANSFER_EVERY) {
        for (k, block) in row.iter().enumerate() {
            let Some(item) = drum.outlets[k].first().cloned() else {
                continue;
            };
            if push_down(host, level, block.offset(Dir::Down), &item) {
                drum.outlets[k].remove(0);
                changed = true;
            }
        }
    }
    let room = machine::TROMMEL_CAPACITY.saturating_sub(drum.items.len());
    with_level(dim, |l| {
        if let Some(q) = socket_block {
            l.loads.insert(q, result.load);
        }
        l.machines.room.insert(pos, room);
        let signal = (drum.items.len() * 15 / machine::TROMMEL_CAPACITY) as i32;
        for q in row {
            l.machines.signals.insert(q, signal);
        }
    });
    let (min, max) = m.range();
    let data = vec![
        drum.items.len() as i32,
        (rpm * 10.0).round() as i32,
        result.status.code(),
        max as i32,
        0,
        0,
        min as i32,
        machine::TROMMEL_CAPACITY as i32,
    ];
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let working = result.status == Status::Grinding;
    let nbt = &mut event.entity.nbt;
    let sent_rpm = blocks::get_f64(nbt, "m_rpm").unwrap_or(0.0);
    let client_changed = changed
        || blocks::get_bool(nbt, "working") != working
        || (sent_rpm - rpm).abs() > 0.05 * rpm.abs().max(1.0);
    if client_changed {
        write_drum(&*host, nbt, &drum);
        blocks::put_f64(nbt, "m_rpm", rpm);
        blocks::put_bool(nbt, "working", working);
        nbt.insert("st_t".into(), Tag::Long(game_now));
        blocks::put_f64(
            nbt,
            "st_v",
            if working {
                grind::progress_per_tick(rpm, min, max)
            } else {
                0.0
            },
        );
    }
    with_level(dim, |l| l.machines.drums.insert(pos, drum));
    if client_changed {
        host.block_entity_sync(event.pos);
    }
    if !(data_changed || event.entity.nbt != old_nbt) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

// ---- Placing, breaking, forming, sockets --------------------------------------------------

/// A line machine half was placed or broken.
pub fn line_changed(dim: Dim, pos: Pos, placed: bool) {
    with_level(dim, |l| l.machines.lines.push((pos, placed)));
}

/// A trommel block was placed or broken.
pub fn trommel_changed(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.machines.trommels.insert(pos);
    });
}

/// A drive's or a trommel's neighbour changed: its socket may move.
pub fn neighbour(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.machines.sockets.insert(pos);
    });
}

/// The other half of a line machine whose half at `pos` was broken, and
/// whether breaking it drops the machine (the drive carries the drop).
/// `half` reads a block as a line machine half (is it the top). Nothing
/// when `pos` holds a half again (a stale break).
fn line_partner(pos: Pos, half: &dyn Fn(Pos) -> Option<bool>) -> Option<(Pos, bool)> {
    if half(pos).is_some() {
        return None;
    }
    if half(pos.offset(Dir::Up)) == Some(true) {
        return Some((pos.offset(Dir::Up), false));
    }
    if half(pos.offset(Dir::Down)) == Some(false) {
        return Some((pos.offset(Dir::Down), true));
    }
    None
}

fn drop_items(host: &mut dyn ServerHost, level: LevelRef, at: Pos, items: Vec<ItemStack>) {
    for stack in items {
        let _ = level.spawn_item(
            host,
            at.x as f64 + 0.5,
            at.y as f64 + 0.5,
            at.z as f64 + 0.5,
            stack,
            10,
        );
    }
}

/// Placed and broken machines: a drive gets its top, a broken half takes
/// the other with it, trommel rows form again, sockets move. Done by the
/// level tick.
pub fn queued(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let lines = with_level(dim, |l| std::mem::take(&mut l.machines.lines));
    for (pos, placed) in lines {
        if placed {
            let Some(b) = block_at(&*host, level, pos) else {
                continue;
            };
            if !b.is_line_machine() || is_top(&*host, level, pos) {
                continue;
            }
            let above = pos.offset(Dir::Up);
            if block_at(&*host, level, above) == Some(b) {
                continue;
            }
            let Some(state) = level.block_state(&*host, pos.to_array()) else {
                continue;
            };
            let top = {
                let states = host.block_states();
                state
                    .with(&states, "half", "upper")
                    .ok()
                    .and_then(|s| s.with(&states, "socket", "down").ok())
            };
            let free = level.block(&*host, above.to_array()).is_some_and(|k| {
                matches!(
                    k.id(&*host).to_string().as_str(),
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:short_grass"
                )
            });
            match top {
                Some(top) if free => {
                    let _ = level.set_block(
                        host,
                        above.to_array(),
                        top,
                        mod_api::BlockUpdateFlags::NEIGHBORS | mod_api::BlockUpdateFlags::CLIENTS,
                    );
                }
                _ => {
                    let _ = level.destroy_block(host, pos.to_array(), true, None, false);
                }
            }
            continue;
        }
        // Broken: the station's items drop, the other half goes.
        if let Some(st) = with_level(dim, |l| l.machines.stations.remove(&pos)) {
            let mut st = st;
            drop_items(host, level, pos, st.take_all());
        }
        let half = |q: Pos| -> Option<bool> {
            let b = block_at(&*host, level, q)?;
            b.is_line_machine().then(|| is_top(&*host, level, q))
        };
        if let Some((q, drop)) = line_partner(pos, &half) {
            let _ = level.destroy_block(host, q.to_array(), drop, None, false);
            if let Some(mut st) = with_level(dim, |l| l.machines.stations.remove(&q)) {
                drop_items(host, level, q, st.take_all());
            }
        }
    }

    let trommels = with_level(dim, |l| std::mem::take(&mut l.machines.trommels));
    if !trommels.is_empty() {
        form_trommels(host, level, dim, &trommels);
    }
    let sockets = with_level(dim, |l| std::mem::take(&mut l.machines.sockets));
    for pos in sockets {
        match block_at(&*host, level, pos) {
            Some(b) if b.is_line_machine() && !is_top(&*host, level, pos) => {
                drive_socket(host, level, dim, pos);
            }
            Some(Block::Trommel) => {
                if let Some(row) = trommel_row(&*host, level, pos) {
                    trommel_socket(host, level, dim, &row, false);
                }
            }
            _ => {}
        }
    }
}

/// The blocks a trommel change at `changed` can form or unform: every row
/// through a changed block, beside it (a block touching a row's side), or
/// that it lengthens lies within three blocks along x and z, diagonals
/// included (a block beside a middle leaves the ends diagonal to it).
fn trommel_near(changed: &BTreeSet<Pos>) -> Vec<Pos> {
    let mut near: BTreeSet<Pos> = BTreeSet::new();
    for p in changed {
        for dx in -3..=3 {
            for dz in -3..=3 {
                near.insert(Pos::new(p.x + dx, p.y, p.z + dz));
            }
        }
    }
    near.into_iter().collect()
}

/// Form the trommel rows near changed blocks; unformed blocks drop what
/// their drum held.
fn form_trommels(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, changed: &BTreeSet<Pos>) {
    let near = trommel_near(changed);
    let (rows, present) = {
        let is = |q: Pos| block_at(&*host, level, q) == Some(Block::Trommel);
        let rows = trommel_rows(&is, &near);
        let mut present: BTreeSet<Pos> = near.iter().copied().filter(|q| is(*q)).collect();
        present.extend(rows.iter().flat_map(|(row, _)| row.iter().copied()));
        (rows, present)
    };
    let mut formed: HashMap<Pos, (&'static str, Axis)> = HashMap::new();
    for (row, axis) in &rows {
        formed.insert(row[0], ("end", *axis));
        formed.insert(row[1], ("middle", *axis));
        formed.insert(row[2], ("end", *axis));
    }
    // The drums of middles that aren't any more spill.
    let lost: Vec<Pos> = with_level(dim, |l| {
        l.machines
            .drums
            .keys()
            .copied()
            .filter(|p| near.contains(p) && formed.get(p).is_none_or(|(part, _)| *part != "middle"))
            .collect()
    });
    for p in lost {
        if let Some(drum) = with_level(dim, |l| l.machines.drums.remove(&p)) {
            let mut all: Vec<ItemStack> = drum.items.into_iter().map(|t| t.item).collect();
            for out in drum.outlets {
                all.extend(out);
            }
            drop_items(host, level, p, all);
        }
    }
    for q in &present {
        let (part, axis) = formed
            .get(q)
            .map_or(("none", None), |(p, a)| (*p, Some(*a)));
        if property(&*host, level, *q, "part").as_deref() != Some(part) {
            super::world::set_property(host, level, *q, "part", part);
        }
        if let Some(axis) = axis
            && property(&*host, level, *q, "axis").as_deref() != Some(axis.name())
        {
            super::world::set_property(host, level, *q, "axis", axis.name());
        }
        if part == "none" && property(&*host, level, *q, "socket").is_some_and(|s| s != "none") {
            super::world::set_property(host, level, *q, "socket", "none");
        }
    }
    for (row, _) in rows {
        trommel_socket(host, level, dim, &row, false);
    }
}

/// The faces a drive can take its socket on: four sides and the bottom (2.3).
const DRIVE_FACES: [Dir; 5] = [Dir::North, Dir::East, Dir::South, Dir::West, Dir::Down];

/// Keep a drive's socket on a face with a shaft, or move it to one (2.3).
fn drive_socket(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, pos: Pos) {
    let current = dir_prop(&*host, level, pos, "socket").unwrap_or(Dir::Down);
    if shaft_against(dim, pos, current) {
        return;
    }
    if let Some(d) = DRIVE_FACES
        .into_iter()
        .find(|d| *d != current && shaft_against(dim, pos, *d))
    {
        super::world::set_property(host, level, pos, "socket", d.name());
    }
}

/// The long side faces of a formed trommel: (block, face), low end first.
fn trommel_faces(row: &[Pos; 3]) -> Vec<(Pos, Dir)> {
    let across = if trommel_axis(row) == Axis::X {
        [Dir::North, Dir::South]
    } else {
        [Dir::West, Dir::East]
    };
    row.iter()
        .flat_map(|p| across.into_iter().map(move |d| (*p, d)))
        .collect()
}

/// Keep a trommel's socket on a long side with a shaft, or move it to the
/// first one that has one (8.2); `next` moves it on to the next face with a
/// shaft (the wrench). Whether it moved.
fn trommel_socket(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    row: &[Pos; 3],
    next: bool,
) -> bool {
    let faces = trommel_faces(row);
    let current = faces
        .iter()
        .position(|(p, d)| dir_prop(&*host, level, *p, "socket") == Some(*d));
    let with_shaft: Vec<usize> = (0..faces.len())
        .filter(|i| shaft_against(dim, faces[*i].0, faces[*i].1))
        .collect();
    let want = match current {
        Some(c) if !next && with_shaft.contains(&c) => return false,
        Some(c) if next => with_shaft
            .iter()
            .copied()
            .find(|i| *i > c)
            .or_else(|| with_shaft.first().copied()),
        _ => with_shaft.first().copied(),
    };
    if want == current {
        return false;
    }
    for p in row {
        let socket = match want {
            Some(i) if faces[i].0 == *p => faces[i].1.name(),
            _ => "none",
        };
        if property(&*host, level, *p, "socket").as_deref() != Some(socket) {
            super::world::set_property(host, level, *p, "socket", socket);
        }
    }
    true
}

/// The wrench on a machine: the socket moves to the next face with a shaft
/// against it (all of a drive's five faces if none has one). Whether it moved.
pub fn cycle_socket(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    b: Block,
) -> bool {
    if b == Block::Trommel {
        return match trommel_row(&*host, level, pos) {
            Some(row) => trommel_socket(host, level, dim, &row, true),
            None => false,
        };
    }
    let Some(drive) = part_pos(&*host, level, pos, b) else {
        return false;
    };
    let current = dir_prop(&*host, level, drive, "socket").unwrap_or(Dir::Down);
    let with_shaft: Vec<Dir> = DRIVE_FACES
        .into_iter()
        .filter(|d| shaft_against(dim, drive, *d))
        .collect();
    let choices: Vec<Dir> = if with_shaft.is_empty() {
        DRIVE_FACES.to_vec()
    } else {
        with_shaft
    };
    let i = choices.iter().position(|d| *d == current);
    let next = match i {
        Some(i) => choices[(i + 1) % choices.len()],
        None => choices[0],
    };
    if next == current {
        return false;
    }
    super::world::set_property(host, level, drive, "socket", next.name())
}

// ---- Hand use --------------------------------------------------------------------------

/// Right-click on a machine: an item it has a recipe for goes onto its
/// built-in belt (machines spec 2.6); anything else opens the info screen.
pub fn use_on(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> InteractionResult {
    let dim = click.dimension.key().id();
    let Some(level) = host.level(dim) else {
        return InteractionResult::Pass;
    };
    let pos = Pos::from_array(click.pos);
    let Some(b) = block_at(&*host, level, pos) else {
        return InteractionResult::Pass;
    };
    let Some(at) = state_pos(&*host, level, pos, b) else {
        if b == Block::Trommel {
            if let Some(player) = host.player(click.player) {
                let key = trommel_unformed(&*host, level, pos);
                super::tools::bar(host, player, super::tools::tr(key, vec![]));
            }
            return InteractionResult::Success;
        }
        return InteractionResult::Pass;
    };
    if b.is_line_machine()
        && let (Some(stack), Some(m)) = (click.stack, b.machine())
        && stack.count() > 0
        && has_recipe(&*host, m, stack)
    {
        let room = with_level(dim, |l| l.machines.room.get(&at).copied().unwrap_or(0));
        let queued = with_level(dim, |l| l.machines.feed.get(&at).map_or(0, Vec::len));
        if room > queued
            && let Some(one) = stack.clone().split(1).taken
        {
            with_level(dim, |l| {
                l.machines.feed.entry(at).or_default().push((one, true))
            });
            if let Some(player) = host.player(click.player)
                && !super::tools::creative(&*host, player)
            {
                let slot = super::tools::hand_slot(&*host, player, click.hand);
                let _ = player.set_inventory_slot(host, slot, stack.clone().shrink(1));
            }
            return InteractionResult::Success;
        }
        return InteractionResult::Fail;
    }
    let (Some(entity), Some(player)) = (
        host.block_entity(level, at.to_array()),
        host.player(click.player),
    ) else {
        return InteractionResult::Pass;
    };
    match player.open_menu(host, entity) {
        Ok(true) => InteractionResult::Success,
        _ => InteractionResult::Pass,
    }
}

/// Pull in items tossed on top of a machine (machines spec 2.4, 9): one
/// stack at a time into the input, if it takes the item.
pub fn pull_tossed(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    input: &mut Option<ItemStack>,
    accepts: &dyn Fn(&dyn Host, &ItemStack) -> bool,
) {
    let min = [pos.x as f64, pos.y as f64 + 0.9, pos.z as f64];
    let max = [pos.x as f64 + 1.0, pos.y as f64 + 1.5, pos.z as f64 + 1.0];
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return;
    };
    for e in entities {
        let Some(item) = host.item_entity(e) else {
            continue;
        };
        let Some(stack) = item.stack(&*host) else {
            continue;
        };
        if !accepts(&*host, &stack) {
            continue;
        }
        let max_stack = host.max_stack_size(stack.item()).unwrap_or(64);
        let room = match input {
            None => max_stack,
            Some(s) if s.item() == stack.item() && s.components() == stack.components() => {
                max_stack.saturating_sub(s.count())
            }
            Some(_) => 0,
        };
        if room == 0 {
            return;
        }
        let parts = stack.split(room);
        let Some(taken) = parts.taken else {
            continue;
        };
        let n = taken.count();
        match input {
            Some(s) => {
                let c = s.count();
                let _ = s.set_count(c + n);
            }
            None => *input = Some(taken),
        }
        match parts.remainder {
            Some(rest) => {
                let _ = item.set_stack(host, rest);
            }
            None => {
                let _ = e.discard(host);
            }
        }
        return;
    }
}

/// Hurt everything in a box (the hammer's strike, the saw's blade).
fn hurt_in(host: &mut dyn ServerHost, level: LevelRef, min: [f64; 3], max: [f64; 3], amount: f32) {
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return;
    };
    for e in entities {
        if host.item_entity(e).is_some() {
            continue;
        }
        let _ = e.hurt(host, "fras:machine", amount, None, None);
    }
}

/// A water bucket emptied into a washer (machines spec 4.3).
pub fn bucket(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let keys = super::keys()?;
    let stack = click.stack?;
    if stack.item() != keys.water_bucket {
        return None;
    }
    let level = host.level(click.dimension.key().id())?;
    let dim = click.dimension.key().id();
    let clicked = Pos::from_array(click.pos);
    let pos = state_pos(&*host, level, clicked, Block::Washer).unwrap_or(clicked);
    let nbt = host.block_entity(level, pos.to_array())?.data(&*host)?;
    let tank = read_tank(&nbt);
    let queued = with_level(dim, |l| l.machines.fill.get(&pos).copied().unwrap_or(0.0));
    if tank.space() - queued < 1000.0 - 1e-6 || tank.content.is_some_and(|c| c != Content::Water) {
        return Some(InteractionResult::Fail);
    }
    with_level(dim, |l| *l.machines.fill.entry(pos).or_default() += 1000.0);
    let player = host.player(click.player)?;
    if !super::tools::creative(&*host, player)
        && let Some(empty) = ItemStack::new(keys.bucket, 1)
    {
        let slot = super::tools::hand_slot(&*host, player, click.hand);
        let _ = player.set_inventory_slot(host, slot, Some(empty));
    }
    super::effects::sound(host, level, pos, "minecraft:item.bucket.empty", 0.8, 1.0);
    Some(InteractionResult::Success)
}

pub fn screen(_: Block, _: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    // Information only: no slots, and the player's inventory off screen
    // (machines spec 2.5).
    Some(mod_api::Screen {
        width: 176.0,
        height: 90.0,
        slots: vec![None],
        inventory_y: 1000.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// Progress, speed bar with the machine's band, status, and the washer's
/// water or the trommel's drum.
pub fn screen_draw(b: Block, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (progress, rpm10, status, max, tank, min) =
        (d(0), d(1), Status::from_code(d(2)), d(3).max(1), d(4), d(6));
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let grey = 0xFF40_4040;
    if b == Block::Trommel {
        // The drum: how many items tumble (8.4).
        let k = "fras.trommel.drum";
        let text = host
            .translate(k)
            .unwrap_or_else(|| k.into())
            .replacen("%s", &progress.to_string(), 1)
            .replacen("%s", &d(7).max(1).to_string(), 1);
        draw.text(&text, [x + 8.0, y + 22.0], grey, false);
    } else {
        // Progress on the item being worked (2.5).
        draw.fill([x + 34.0, y + 26.0, 60.0, 6.0], 0xFF55_5555);
        draw.fill(
            [
                x + 34.0,
                y + 26.0,
                60.0 * (progress.clamp(0, 1000) as f32 / 1000.0),
                6.0,
            ],
            0xFFFF_AA00,
        );
    }
    if b == Block::Washer {
        // The tank gauge.
        let h = 34.0;
        let fill = h * (tank as f32 / machine::WASHER_TANK as f32).clamp(0.0, 1.0);
        draw.fill([x + 150.0, y + 14.0, 8.0, h], 0xFF30_3030);
        draw.fill([x + 150.0, y + 14.0 + h - fill, 8.0, fill], 0xFF30_60D0);
        let mb = format!("{tank} mB");
        draw.text(&mb, [x + 104.0, y + 40.0], grey, false);
    }
    // Speed bar: 0 to twice... scaled to the machine's top plus a margin.
    let top = (max as f32 * 1.25).max(32.0);
    let (bx, by, bw) = (x + 8.0, y + 58.0, 112.0);
    let scale = |rpm: f32| bw * (rpm / top).clamp(0.0, 1.0);
    let (minf, maxf) = (min as f32, max as f32);
    draw.fill([bx, by, bw, 4.0], 0xFF40_4040);
    draw.fill(
        [bx + scale(minf), by, scale(maxf) - scale(minf), 4.0],
        0xFF3C_A03C,
    );
    draw.fill([bx + scale(maxf), by, bw - scale(maxf), 4.0], 0xFFB0_3030);
    let rpm = (rpm10 as f32 / 10.0).abs();
    draw.fill([bx + scale(rpm) - 1.0, by - 2.0, 2.0, 8.0], 0xFFFF_FFFF);
    let speed = format!("{} RPM", super::num(rpm as f64));
    draw.text(&speed, [bx + bw + 4.0, by - 2.0], 0xFF40_4040, false);
    let key = match status {
        Status::TooSlow => "fras.machine.too_slow".to_string(),
        Status::Grinding => "fras.machine.working".to_string(),
        _ => format!("fras.grinder.status.{}", status.key()),
    };
    let text = host
        .translate(&key)
        .unwrap_or(key)
        .replace("%s", &min.to_string());
    let color = match status {
        Status::Jammed | Status::Overspeed | Status::NoWater => 0xFFB0_2020,
        Status::Grinding => 0xFF20_7020,
        _ => 0xFF40_4040,
    };
    super::text_in(draw, &text, [x + 8.0, y + 72.0], 160.0, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn halves(h: &[(Pos, bool)]) -> impl Fn(Pos) -> Option<bool> + use<> {
        let m: HashMap<Pos, bool> = h.iter().copied().collect();
        move |p| m.get(&p).copied()
    }

    #[test]
    fn breaking_a_line_machine_half_breaks_the_other() {
        let drive = Pos::new(0, 0, 0);
        let top = Pos::new(0, 1, 0);
        // The drive broke: the top goes, dropping nothing.
        assert_eq!(
            line_partner(drive, &halves(&[(top, true)])),
            Some((top, false))
        );
        // The top broke: the drive goes and drops the machine.
        assert_eq!(
            line_partner(top, &halves(&[(drive, false)])),
            Some((drive, true))
        );
        // A stale break leaves a rebuilt machine alone.
        let both = halves(&[(drive, false), (top, true)]);
        assert_eq!(line_partner(drive, &both), None);
        // Another machine's drive above a broken drive isn't a partner.
        assert_eq!(line_partner(drive, &halves(&[(top, false)])), None);
    }

    fn rows(blocks: &[Pos]) -> Vec<([Pos; 3], Axis)> {
        let set: HashSet<Pos> = blocks.iter().copied().collect();
        trommel_rows(&|p| set.contains(&p), blocks)
    }

    #[test]
    fn three_in_a_row_form_a_trommel_low_end_first() {
        let r = rows(&[Pos::new(2, 0, 0), Pos::new(0, 0, 0), Pos::new(1, 0, 0)]);
        assert_eq!(
            r,
            vec![(
                [Pos::new(0, 0, 0), Pos::new(1, 0, 0), Pos::new(2, 0, 0)],
                Axis::X
            )]
        );
        let r = rows(&[Pos::new(0, 0, 0), Pos::new(0, 0, 1), Pos::new(0, 0, 2)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, Axis::Z);
    }

    #[test]
    fn two_four_or_crowded_rows_dont_form() {
        assert!(rows(&[Pos::new(0, 0, 0), Pos::new(1, 0, 0)]).is_empty());
        let four: Vec<Pos> = (0..4).map(|x| Pos::new(x, 0, 0)).collect();
        assert!(rows(&four).is_empty());
        let mut crowded: Vec<Pos> = (0..3).map(|x| Pos::new(x, 0, 0)).collect();
        crowded.push(Pos::new(1, 0, 1));
        assert!(rows(&crowded).is_empty());
        // Two rows side by side touch each other across: neither forms.
        let mut pair: Vec<Pos> = (0..3).map(|x| Pos::new(x, 0, 0)).collect();
        pair.extend((0..3).map(|x| Pos::new(x, 0, 1)));
        assert!(rows(&pair).is_empty());
        // Stacked rows don't touch across: both form.
        let mut stacked: Vec<Pos> = (0..3).map(|x| Pos::new(x, 0, 0)).collect();
        stacked.extend((0..3).map(|x| Pos::new(x, 1, 0)));
        assert_eq!(rows(&stacked).len(), 2);
    }

    #[test]
    fn a_block_beside_a_middle_reaches_the_rows_ends() {
        // Found in game: a trommel placed beside a formed row's middle left
        // its ends formed, as they sit diagonal to the new block.
        let changed: BTreeSet<Pos> = [Pos::new(1, 0, 1)].into();
        let near = trommel_near(&changed);
        for x in 0..3 {
            assert!(near.contains(&Pos::new(x, 0, 0)));
        }
        // A fourth block on the end still reaches the far end.
        let changed: BTreeSet<Pos> = [Pos::new(3, 0, 0)].into();
        assert!(trommel_near(&changed).contains(&Pos::new(0, 0, 0)));
    }
}
