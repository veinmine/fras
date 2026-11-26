//! The 0.4 machines on Horizon (machines spec 2 to 8 and 11): the washer,
//! trip hammer, lathe and sawmill share the grinder's slots and screen; the
//! washer and trip hammer also work over a belt; the trommel sorts by tags
//! into three outlets.
//!
//! Recipes come from one loader for the four recipe types, the grinder's
//! format with each machine's default load and water. A belt machine
//! registers its state in `Level::belt_jobs` every tick it is over a flat
//! belt, and the belt line asks it (`belt_arrive`, `belt_poll`) whether to
//! hold the item under it and what it became.

use std::collections::HashMap;
use std::sync::RwLock;

use mod_api::{
    BlockClick, BlockEntityTickEvent, BlockEntityTickReply, ClientHost, Compound, Host,
    HostBlockStates, HostTransaction, Identifier, Ingredient, InteractionResult, ItemResource,
    ItemStack, LevelRef, ModContainer, STORAGE, ScreenDrawEvent, ScreenEvent, ServerHost, Tag,
};

use super::blocks::{self, Block};
use super::grinder::{HostItems, item_key, random, stack_of};
use super::server::{self, Dim, block_at, property, with_level};
use crate::sim::fluid::Content;
use crate::sim::geom::{Dir, Pos};
use crate::sim::grind::{self, MachineState, Recipe, Stack, Status};
use crate::sim::machine::{self, BeltWork, Drum, Machine, Size, Tank, Tumbling};

/// Slots of a recipe machine: input, then four outputs.
pub const SLOTS: usize = 1 + grind::OUTPUTS;
/// Slots of the trommel: input, then the fine, medium and oversize outlets.
pub const TROMMEL_SLOTS: usize = 4;

/// The container of a 0.4 machine (machines spec 2.3, 8.2).
pub fn container(b: Block) -> ModContainer {
    let menu = Some(format!("fras:{}", b.name()));
    if b == Block::Trommel {
        let all: Vec<usize> = (0..TROMMEL_SLOTS).collect();
        return ModContainer {
            slots: TROMMEL_SLOTS,
            faces: [
                vec![1, 2, 3],
                all.clone(),
                all.clone(),
                all.clone(),
                all.clone(),
                all,
            ],
            inputs: vec![0],
            outputs: vec![1, 2, 3],
            menu,
        };
    }
    let all: Vec<usize> = (0..SLOTS).collect();
    ModContainer {
        slots: SLOTS,
        faces: [
            vec![1, 2, 3, 4],
            all.clone(),
            all.clone(),
            all.clone(),
            all.clone(),
            all,
        ],
        inputs: vec![0],
        outputs: vec![1, 2, 3, 4],
        menu,
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

/// Whether a machine has a recipe for `stack` (any count).
pub fn has_recipe(host: &dyn Host, m: Machine, stack: &ItemStack) -> bool {
    if m == Machine::Trommel {
        return true;
    }
    let id = stack.item().id(host);
    with_book(host, m, |recipes, items| {
        grind::find(recipes, &id, items).is_some()
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

/// A belt machine's state for the line under it (logistics 7).
#[derive(Clone, Debug)]
pub struct BeltJob {
    pub machine: Machine,
    pub work: BeltWork,
    pub state: MachineState,
    /// Results waiting to go onto the belt behind the main one.
    pub pending: Vec<ItemStack>,
    pub load: f64,
    pub status: Status,
    pub needed: f64,
    /// Water used since the machine's tick last drained its tank.
    pub water_used: f64,
    /// The item it holds and works on.
    pub held: Option<String>,
    /// Recipes finished, for the debug report.
    pub completed: u64,
    /// Server tick the machine last refreshed it.
    pub seen: i64,
}

/// Work hooks queue for the machines' ticks.
#[derive(Default)]
pub struct Queue {
    /// Water poured in by hand, mB, by washer.
    pub fill: HashMap<Pos, f64>,
    /// Trommels placed (true) or broken (false), by start block.
    pub trommels: Vec<(Pos, bool)>,
    /// Recipes finished per machine, for the debug report.
    pub made: HashMap<Pos, u64>,
}

fn rot_state(dim: Dim, pos: Pos, nbt: &Compound) -> (f64, bool, bool) {
    let saved = blocks::saved(nbt);
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

/// Whether a washer or trip hammer at `pos` is in belt mode: directly above
/// a flat belt block (machines spec 2.3, 3).
pub fn belt_mode(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    let Some(m) = block_at(host, level, pos).and_then(Block::machine) else {
        return false;
    };
    if !m.works_on_belt() {
        return false;
    }
    let below = pos.offset(Dir::Down);
    block_at(host, level, below) == Some(Block::ConveyorBelt)
        && property(host, level, below, "slope").is_none_or(|s| s == "flat")
}

/// Whether the machine block at `pos` takes items from a belt end or chute
/// arriving through its face `face` (machines spec 2.4, 8.2, 9).
pub fn takes_from(host: &dyn Host, level: LevelRef, pos: Pos, b: Block, face: Dir) -> bool {
    let facing = property(host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::North);
    match b {
        Block::Trommel => {
            property(host, level, pos, "part").as_deref() != Some("end")
                && (face == Dir::Up || face == facing.opposite())
        }
        Block::Washer | Block::TripHammer if belt_mode(host, level, pos) => false,
        _ => face != Dir::Down && face != facing,
    }
}

// ---- Belt mode: the line's side ------------------------------------------------------

/// An item reached the centre of the belt block under the machine at
/// `pos`: whether the machine holds it to work on.
pub fn belt_arrive(host: &dyn Host, level: LevelRef, dim: Dim, pos: Pos, item: &ItemStack) -> bool {
    let Some(job) = with_level(dim, |l| l.belt_jobs.get(&pos).cloned()) else {
        return false;
    };
    let now = level.game_time(host).unwrap_or(0);
    if job.seen + 2 < now || job.held.is_some() {
        return false;
    }
    let id = item.item().id(host);
    let takes = with_book(host, job.machine, |recipes, items| {
        machine::belt_accepts(&id, recipes, items)
    });
    if takes {
        with_level(dim, |l| {
            if let Some(j) = l.belt_jobs.get_mut(&pos) {
                j.held = Some(id);
            }
        });
    }
    takes
}

/// An item held under the machine at `pos`: work on it; whether to keep
/// holding it. A finished recipe turns it into the main result in place.
pub fn belt_poll(host: &dyn Host, dim: Dim, pos: Pos, item: &mut ItemStack) -> bool {
    let Some(mut job) = with_level(dim, |l| l.belt_jobs.remove(&pos)) else {
        return false;
    };
    let now = host
        .level(dim)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    if job.seen + 2 < now {
        // The machine left belt mode or was broken.
        job.held = None;
        with_level(dim, |l| l.belt_jobs.insert(pos, job));
        return false;
    }
    let id = item.item().id(host);
    if !job.pending.is_empty() {
        // The last item's byproducts aren't on the belt yet: wait (7.3).
        job.held = Some(id);
        with_level(dim, |l| l.belt_jobs.insert(pos, job));
        return true;
    }
    let m = job.machine;
    let t = with_book(host, m, |recipes, items| {
        machine::belt_tick(&mut job.work, &id, &job.state, recipes, items, &mut random)
    });
    job.load = t.load;
    job.status = t.status;
    job.needed = t.needed;
    job.water_used += t.water_used;
    if let Some(w) = job.state.water.as_mut() {
        *w = (*w - t.water_used).max(0.0);
    }
    let keep = match t.done {
        None => {
            job.held = Some(id);
            // No recipe for it after all (a datapack reload): let it go.
            job.work.recipe.is_some() || job.state.jammed
        }
        Some(main) => {
            for p in std::mem::take(&mut job.work.pending) {
                if let Some(s) = make(host, &p, 1) {
                    job.pending.push(s);
                }
            }
            match main.and_then(|m| make(host, &m, 1)) {
                Some(s) => *item = s,
                None if !job.pending.is_empty() => *item = job.pending.remove(0),
                None => {}
            }
            job.held = None;
            job.completed += 1;
            false
        }
    };
    if !keep {
        job.held = None;
    }
    with_level(dim, |l| l.belt_jobs.insert(pos, job));
    keep
}

// ---- The tick --------------------------------------------------------------------------

fn read_tank(nbt: &Compound) -> Tank {
    Tank {
        content: blocks::get_str(nbt, "tank_c")
            .as_deref()
            .and_then(Content::from_name),
        amount: blocks::get_f64(nbt, "tank").unwrap_or(0.0),
    }
}

/// Write the slot model back into the block entity's items.
fn write_outputs(
    host: &dyn Host,
    items: &mut [Option<ItemStack>],
    first: usize,
    outputs: &[Option<Stack>],
) {
    for (i, out) in outputs.iter().enumerate() {
        let slot = &mut items[first + i];
        if stack_of(host, slot) == *out {
            continue;
        }
        *slot = match out {
            None => None,
            Some(s) => match slot.as_mut() {
                Some(existing) if existing.item().id(host) == s.item => {
                    let _ = existing.set_count(s.count);
                    slot.take()
                }
                _ => make(host, &s.item, s.count),
            },
        };
    }
}

/// Move one item from `slots` to the block at `target`, entering through
/// its face `face`: an inventory or belt (through its storage), else dropped
/// into the air if it is open (machines spec 2.4, 8.2).
fn push_one(
    host: &mut dyn ServerHost,
    level: LevelRef,
    items: &mut [Option<ItemStack>],
    slots: &[usize],
    target: Pos,
    face: Dir,
    drop_into_air: bool,
) -> bool {
    let Some(&i) = slots.iter().find(|i| items[**i].is_some()) else {
        return false;
    };
    let Some(stack) = items[i].clone() else {
        return false;
    };
    if let Some(storage) = level.capability(
        host,
        &STORAGE,
        target.to_array(),
        Some(super::direction(face)),
    ) {
        let resource = ItemResource::of(&stack);
        let moved: Result<u64, ()> =
            host.transaction(|host, tx| Ok(storage.insert(host, tx, &resource, 1)));
        if moved == Ok(1) {
            items[i] = stack.shrink(1);
            return true;
        }
        return false;
    }
    if !drop_into_air {
        return false;
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
    let Some(one) = stack.clone().split(1).taken else {
        return false;
    };
    let _ = level.spawn_item(
        host,
        target.x as f64 + 0.5,
        target.y as f64 + 0.5,
        target.z as f64 + 0.5,
        one,
        10,
    );
    items[i] = stack.shrink(1);
    true
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
    let (rpm, paused, jammed) = rot_state(dim, pos, &event.entity.nbt);
    let now = server::with(|s| s.ticks);
    let game_now = level.game_time(&*host).unwrap_or(0);
    let before = event.entity.items.clone();
    if event.entity.items.len() < SLOTS {
        event.entity.items.resize(SLOTS, None);
    }
    let old_nbt = event.entity.nbt.clone();

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
    let water = (m == Machine::Washer).then_some(tank.amount);
    let state = MachineState {
        rpm,
        jammed,
        paused,
        range: m.range(),
        water,
    };

    let on_belt = belt_mode(&*host, level, pos);
    let (load, status, progress, needed, working);
    if on_belt {
        let job = with_level(dim, |l| {
            let job = l.belt_jobs.entry(pos).or_insert_with(|| BeltJob {
                machine: m,
                work: BeltWork::default(),
                state: state.clone(),
                pending: Vec::new(),
                load: 0.0,
                status: Status::Idle,
                needed: 0.0,
                water_used: 0.0,
                held: None,
                completed: 0,
                seen: game_now,
            });
            job.machine = m;
            // The line polls after this tick: a held item not polled since
            // the last tick (a stopped belt) draws no load.
            if job.held.is_none() {
                job.load = 0.0;
                job.status = idle_status(&state);
            }
            let mut st = state.clone();
            st.water = water.map(|w| (w - job.water_used).max(0.0));
            job.state = st;
            job.seen = game_now;
            let used = std::mem::take(&mut job.water_used);
            (job.clone(), used)
        });
        let (job, used) = job;
        tank.drain(used);
        load = job.load;
        status = job.status;
        progress = job.work.progress;
        needed = job.needed;
        working = job.held.is_some() && status == Status::Grinding;
        if used > 0.0 || job.completed > 0 {
            with_level(dim, |l| {
                *l.machines.made.entry(pos).or_default() = job.completed;
            });
        }
    } else {
        // Slot mode: as the grinder (machines spec 2.3). Leaving belt mode
        // puts byproducts still waiting into the outputs.
        let pending = with_level(dim, |l| l.belt_jobs.remove(&pos)).map(|j| j.pending);
        let input = stack_of(&*host, &event.entity.items[0]);
        let mut outputs: Vec<Option<Stack>> = (1..SLOTS)
            .map(|i| stack_of(&*host, &event.entity.items[i]))
            .collect();
        let mut work = grind::Work {
            progress: blocks::get_f64(&event.entity.nbt, "progress").unwrap_or(0.0),
            recipe: blocks::get_str(&event.entity.nbt, "recipe").filter(|s| !s.is_empty()),
        };
        let result = with_book(&*host, m, |recipes, items| {
            if let Some(pending) = &pending {
                for s in pending {
                    let one = [Stack::new(&s.item().id(items.host), s.count())];
                    let _ = grind::insert(&one, &mut outputs, items);
                }
            }
            grind::tick_machine(
                &mut work,
                input.as_ref(),
                &mut outputs,
                &state,
                recipes,
                items,
                &mut random,
            )
        });
        tank.drain(result.water_used);
        if result.consumed > 0
            && let Some(stack) = event.entity.items[0].take()
        {
            event.entity.items[0] = stack.shrink(result.consumed);
        }
        write_outputs(&*host, &mut event.entity.items, 1, &outputs);
        if result.completed {
            with_level(dim, |l| *l.machines.made.entry(pos).or_default() += 1);
        }
        load = result.load;
        status = result.status;
        progress = work.progress;
        needed = result.needed;
        working = status == Status::Grinding;
        let nbt = &mut event.entity.nbt;
        blocks::put_f64(nbt, "progress", work.progress);
        blocks::put_str(nbt, "recipe", work.recipe.as_deref().unwrap_or(""));
        // Every 8 ticks: push one down, pull in what was tossed on top.
        if (now + pos.x as u64 + pos.z as u64).is_multiple_of(grind::TRANSFER_EVERY) {
            push_one(
                host,
                level,
                &mut event.entity.items,
                &[1, 2, 3, 4],
                pos.offset(Dir::Down),
                Dir::Up,
                false,
            );
            pull_tossed(host, level, pos, &mut event.entity.items[0], &|h, s| {
                has_recipe(h, m, s)
            });
        }
    }
    with_level(dim, |l| {
        l.loads.insert(pos, load);
        if m == Machine::Washer {
            // Ask the pipes for the room in the tank, at most a bucket a
            // second (machines spec 4.3).
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
            if on_belt {
                let below = pos.offset(Dir::Down);
                let min = [below.x as f64 + 0.2, below.y as f64, below.z as f64 + 0.2];
                let max = [
                    below.x as f64 + 0.8,
                    below.y as f64 + 1.0,
                    below.z as f64 + 0.8,
                ];
                hurt_in(host, level, min, max, machine::HAMMER_DAMAGE);
            }
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

    // Menu data: progress ‰, RPM × 10, status, max RPM, tank mB, belt
    // mode, min RPM, strikes needed (the hammer counts strikes).
    let progress_milli = if needed > 0.0 {
        (progress / needed * 1000.0) as i32
    } else {
        0
    };
    let (min, max) = m.range();
    let data = vec![
        progress_milli,
        (rpm * 10.0).round() as i32,
        status.code(),
        max as i32,
        tank.amount.round() as i32,
        on_belt as i32,
        min as i32,
        (needed * m.strikes_per_rev()).round() as i32,
    ];
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let shown_item = if on_belt {
        with_level(dim, |l| l.belt_jobs.get(&pos).and_then(|j| j.held.clone())).unwrap_or_default()
    } else {
        stack_of(&*host, &event.entity.items[0])
            .map(|s| s.item)
            .unwrap_or_default()
    };
    let nbt = &mut event.entity.nbt;
    let client_changed = blocks::get_bool(nbt, "working") != working
        || blocks::get_str(nbt, "m_item").unwrap_or_default() != shown_item
        || blocks::get_bool(nbt, "m_hold") != on_belt
        || blocks::get_i32(nbt, "m_hit").unwrap_or(0) != hit
        || (m == Machine::Washer
            && (blocks::get_f64(nbt, "tank").unwrap_or(0.0) - tank.amount).abs() >= 50.0);
    blocks::put_bool(nbt, "working", working);
    blocks::put_str(nbt, "m_item", &shown_item);
    blocks::put_bool(nbt, "m_hold", on_belt);
    blocks::put_i32(nbt, "m_hit", hit);
    blocks::put_f64(nbt, "m_ph", phase);
    if m == Machine::Washer {
        blocks::put_f64(nbt, "tank", tank.amount);
        blocks::put_str(nbt, "tank_c", tank.content.map_or("", Content::name));
    }
    if client_changed {
        host.block_entity_sync(event.pos);
    }
    let items_changed = event.entity.items != before;
    if items_changed {
        refresh_signal_from(&*host, dim, pos, &event.entity.items);
    }
    if !(items_changed || data_changed || event.entity.nbt != old_nbt) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

fn idle_status(state: &MachineState) -> Status {
    if state.jammed {
        Status::Jammed
    } else if state.paused {
        Status::Paused
    } else if state.rpm.abs() > state.range.1 {
        Status::Overspeed
    } else {
        Status::Idle
    }
}

fn refresh_signal_from(host: &dyn Host, dim: Dim, pos: Pos, items: &[Option<ItemStack>]) {
    let slots: Vec<Option<(u32, u32)>> = items
        .iter()
        .map(|s| {
            s.as_ref()
                .map(|s| (s.count(), host.max_stack_size(s.item()).unwrap_or(64)))
        })
        .collect();
    let signal = grind::comparator(&slots) as i32;
    with_level(dim, |l| {
        l.grinder_signal.insert(pos, signal);
    });
}

/// Read a machine's comparator signal from its slots as they are now.
pub fn refresh_signal(host: &dyn Host, level: LevelRef, pos: Pos, slots: usize) {
    let Some(dim) = server::dim_of(host, level) else {
        return;
    };
    let Some(entity) = host.block_entity(level, pos.to_array()) else {
        return;
    };
    let items: Vec<Option<ItemStack>> = (0..slots as u32)
        .map(|i| entity.item_slot(host, i).ok().flatten())
        .collect();
    refresh_signal_from(host, dim, pos, &items);
}

// ---- The trommel -------------------------------------------------------------------------

fn read_drum(host: &dyn Host, nbt: &Compound, items: &[Option<ItemStack>]) -> Drum {
    let mut drum = Drum::default();
    if let Some(Tag::List(list)) = nbt.get(&"drum".into()) {
        for t in list {
            if let Tag::Compound(c) = t {
                let item = blocks::get_str(c, "id").unwrap_or_default();
                if item.is_empty() {
                    continue;
                }
                let size = match blocks::get_i32(c, "s").unwrap_or(2) {
                    0 => Size::Fine,
                    1 => Size::Medium,
                    _ => Size::Oversize,
                };
                drum.items.push(Tumbling {
                    item,
                    size,
                    progress: blocks::get_f64(c, "p").unwrap_or(0.0),
                });
            }
        }
    }
    for k in 0..3 {
        drum.outlets[k] = stack_of(host, &items[1 + k]);
    }
    drum
}

fn write_drum(nbt: &mut Compound, drum: &Drum) {
    let list: Vec<Tag> = drum
        .items
        .iter()
        .map(|t| {
            let mut c = Compound::new();
            blocks::put_str(&mut c, "id", &t.item);
            blocks::put_i32(&mut c, "s", t.size.index() as i32);
            blocks::put_f64(&mut c, "p", t.progress);
            Tag::Compound(c)
        })
        .collect();
    nbt.insert("drum".into(), Tag::List(list));
}

fn trommel_tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let Some(level) = host.level(dim) else {
        return;
    };
    let pos = Pos::from_array(event.pos);
    if property(&*host, level, pos, "part").as_deref() == Some("end") {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    let m = Machine::Trommel;
    let (rpm, paused, jammed) = rot_state(dim, pos, &event.entity.nbt);
    let now = server::with(|s| s.ticks);
    let before = event.entity.items.clone();
    let old_nbt = event.entity.nbt.clone();
    if event.entity.items.len() < TROMMEL_SLOTS {
        event.entity.items.resize(TROMMEL_SLOTS, None);
    }
    let mut drum = read_drum(&*host, &event.entity.nbt, &event.entity.items);
    let state = MachineState {
        rpm,
        jammed,
        paused,
        range: m.range(),
        water: None,
    };
    let result = with_book(&*host, m, |_, items| {
        // One item a tick from the input into the drum (8.2: a full drum
        // refuses).
        if drum.has_room()
            && let Some(stack) = event.entity.items[0].take()
        {
            let id = stack.item().id(items.host);
            if drum.insert(&id, items) {
                event.entity.items[0] = stack.shrink(1);
            } else {
                event.entity.items[0] = Some(stack);
            }
        }
        drum.tick(&state, items)
    });
    if result.sorted > 0 {
        with_level(dim, |l| {
            *l.machines.made.entry(pos).or_default() += result.sorted as u64
        });
    }
    let outlets: Vec<Option<Stack>> = drum.outlets.to_vec();
    write_outputs(&*host, &mut event.entity.items, 1, &outlets);
    with_level(dim, |l| {
        l.loads.insert(pos, result.load);
    });
    // Every 8 ticks each outlet pushes one (8.2): fines under the first
    // block, mediums under the second, oversize out of the far end.
    if (now + pos.x as u64 + pos.z as u64).is_multiple_of(grind::TRANSFER_EVERY) {
        let facing = property(&*host, level, pos, "facing")
            .as_deref()
            .and_then(Dir::from_name)
            .unwrap_or(Dir::North);
        let end = pos.offset(facing);
        let items = &mut event.entity.items;
        push_one(
            host,
            level,
            items,
            &[1],
            pos.offset(Dir::Down),
            Dir::Up,
            true,
        );
        push_one(
            host,
            level,
            items,
            &[2],
            end.offset(Dir::Down),
            Dir::Up,
            true,
        );
        push_one(
            host,
            level,
            items,
            &[3],
            end.offset(facing),
            facing.opposite(),
            true,
        );
        pull_tossed(host, level, pos, &mut items[0], &|_, _| true);
    }
    write_drum(&mut event.entity.nbt, &drum);
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
    let client_changed = blocks::get_bool(nbt, "working") != working;
    blocks::put_bool(nbt, "working", working);
    if client_changed {
        host.block_entity_sync(event.pos);
    }
    let items_changed = event.entity.items != before;
    if items_changed {
        refresh_signal_from(&*host, dim, pos, &event.entity.items);
    }
    if !(items_changed || data_changed || event.entity.nbt != old_nbt) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// Placed and broken trommels: the second block follows the first
/// (machines spec 8.2). Done by the level tick.
pub fn queued(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let work = with_level(dim, |l| std::mem::take(&mut l.machines.trommels));
    for (pos, placed) in work {
        let half = |q: Pos| -> Option<(bool, Dir)> {
            if block_at(&*host, level, q) != Some(Block::Trommel) {
                return None;
            }
            let f = property(&*host, level, q, "facing")
                .as_deref()
                .and_then(Dir::from_name)?;
            Some((
                property(&*host, level, q, "part").as_deref() == Some("end"),
                f,
            ))
        };
        if !placed {
            if let Some((q, drop)) = trommel_orphan(pos, &half) {
                let _ = level.destroy_block(host, q.to_array(), drop, None, false);
            }
            continue;
        }
        let Some(facing) = property(&*host, level, pos, "facing")
            .as_deref()
            .and_then(Dir::from_name)
        else {
            continue;
        };
        let part = property(&*host, level, pos, "part");
        if part.as_deref() != Some("start") {
            continue;
        }
        let ahead = pos.offset(facing);
        if block_at(&*host, level, ahead) == Some(Block::Trommel) {
            continue;
        }
        let Some(state) = level.block_state(&*host, pos.to_array()) else {
            continue;
        };
        let end = {
            let states = host.block_states();
            state.with(&states, "part", "end").ok()
        };
        if let Some(end) = end {
            let _ = level.set_block(
                host,
                ahead.to_array(),
                end,
                mod_api::BlockUpdateFlags::NEIGHBORS | mod_api::BlockUpdateFlags::CLIENTS,
            );
        }
    }
}

/// The other half to break, and whether it drops the item, when the
/// trommel half at `pos` was broken; `half` reads a block as a trommel half
/// (is it the end, its facing). Nothing when `pos` holds a trommel again:
/// the break is stale (it was replaced before the queue ran, as a scenario
/// rebuild does), and the new trommel must stay.
fn trommel_orphan(pos: Pos, half: &dyn Fn(Pos) -> Option<(bool, Dir)>) -> Option<(Pos, bool)> {
    if half(pos).is_some() {
        return None;
    }
    // `pos`'s facing is gone with it, so look both ways for the other half.
    Dir::HORIZONTAL.into_iter().find_map(|d| {
        let q = pos.offset(d);
        let (end, f) = half(q)?;
        let partner = if end {
            q.offset(f.opposite())
        } else {
            q.offset(f)
        };
        // The end gives nothing: the start drops the item.
        (partner == pos).then_some((q, !end))
    })
}

/// A trommel half was placed or broken.
pub fn trommel_changed(dim: Dim, pos: Pos, placed: bool) {
    with_level(dim, |l| l.machines.trommels.push((pos, placed)));
}

// ---- Hand use --------------------------------------------------------------------------

/// A water bucket emptied into a washer (machines spec 4.3).
pub fn bucket(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let keys = super::keys()?;
    let stack = click.stack?;
    if stack.item() != keys.water_bucket {
        return None;
    }
    let level = host.level(click.dimension.key().id())?;
    let dim = click.dimension.key().id();
    let pos = Pos::from_array(click.pos);
    let nbt = host.block_entity(level, click.pos)?.data(&*host)?;
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

// ---- Menus and screens -------------------------------------------------------------------

/// The input slot takes only what the machine can work (nothing in belt
/// mode, which the menu data says).
pub struct InputSlot(pub Machine);

#[mod_api::horizon_part]
impl mod_api::MenuPart for InputSlot {
    fn menu_slot_rules(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::MenuSlotRulesEvent<'_>,
    ) {
        if event.container == mod_api::MenuContainer::Main && event.slot == 0 {
            let loaded = BOOKS.read().map(|b| b.is_some()).unwrap_or(false);
            if loaded {
                event.may_place &= has_recipe(host, self.0, event.stack);
            }
        }
    }
}

pub fn screen(b: Block, _: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    let slots = if b == Block::Trommel {
        vec![
            Some([26.0, 26.0]),
            Some([98.0, 26.0]),
            Some([120.0, 26.0]),
            Some([142.0, 26.0]),
        ]
    } else {
        vec![
            Some([44.0, 26.0]),
            Some([104.0, 17.0]),
            Some([122.0, 17.0]),
            Some([104.0, 35.0]),
            Some([122.0, 35.0]),
        ]
    };
    Some(mod_api::Screen {
        width: 176.0,
        height: 166.0,
        slots,
        inventory_y: 84.0,
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
    let (progress, rpm10, status, max, tank, on_belt, min) = (
        d(0),
        d(1),
        Status::from_code(d(2)),
        d(3).max(1),
        d(4),
        d(5) == 1,
        d(6),
    );
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    if b == Block::Trommel {
        let text = format!("{} / {}", progress, d(7).max(1));
        draw.text(&text, [x + 52.0, y + 30.0], 0xFF40_4040, false);
        for (i, key) in ["fine", "medium", "oversize"].iter().enumerate() {
            let k = format!("fras.trommel.{key}");
            let label = host.translate(&k).unwrap_or(k);
            let label: String = label.chars().take(4).collect();
            draw.text(
                &label,
                [x + 96.0 + 22.0 * i as f32, y + 16.0],
                0xFF40_4040,
                false,
            );
        }
    } else if on_belt {
        let key = "fras.machine.belt_mode";
        let text = host.translate(key).unwrap_or_else(|| key.into());
        super::text_in(draw, &text, [x + 8.0, y + 46.0], 160.0, 0xFF20_5080);
    }
    if b != Block::Trommel {
        draw.fill([x + 66.0, y + 30.0, 30.0, 6.0], 0xFF55_5555);
        draw.fill(
            [
                x + 66.0,
                y + 30.0,
                30.0 * (progress.clamp(0, 1000) as f32 / 1000.0),
                6.0,
            ],
            0xFFFF_AA00,
        );
    }
    if b == Block::Washer {
        // The tank: a small fluid bar left of the input.
        let h = 34.0;
        let fill = h * (tank as f32 / machine::WASHER_TANK as f32).clamp(0.0, 1.0);
        draw.fill([x + 20.0, y + 18.0, 8.0, h], 0xFF30_3030);
        draw.fill([x + 20.0, y + 18.0 + h - fill, 8.0, fill], 0xFF30_60D0);
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
    super::text_in(draw, &text, [x + 8.0, y + 64.0], 160.0, color);
}

/// The item a machine shows (drawn on the client).
pub fn shown_item(host: &dyn Host, nbt: &Compound) -> Option<ItemStack> {
    let id = blocks::get_str(nbt, "m_item").filter(|s| !s.is_empty())?;
    let key = host.item_key(&Identifier::parse(&id).ok()?)?;
    ItemStack::new(key, 1)
}

/// Open the menu.
pub fn open(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> InteractionResult {
    // The trommel's second block opens the first one's menu.
    let level = host.level(click.dimension.key().id());
    if let Some(level) = level
        && block_at(&*host, level, Pos::from_array(click.pos)) == Some(Block::Trommel)
        && property(&*host, level, Pos::from_array(click.pos), "part").as_deref() == Some("end")
        && let Some(f) = property(&*host, level, Pos::from_array(click.pos), "facing")
            .as_deref()
            .and_then(Dir::from_name)
    {
        let start = Pos::from_array(click.pos).offset(f.opposite());
        let (Some(entity), Some(player)) = (
            host.block_entity(level, start.to_array()),
            host.player(click.player),
        ) else {
            return InteractionResult::Pass;
        };
        return match player.open_menu(host, entity) {
            Ok(true) => InteractionResult::Success,
            _ => InteractionResult::Pass,
        };
    }
    super::grinder::open(host, click)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn world(halves: &[(Pos, bool, Dir)]) -> impl Fn(Pos) -> Option<(bool, Dir)> + use<> {
        let m: HashMap<Pos, (bool, Dir)> = halves.iter().map(|(p, e, f)| (*p, (*e, *f))).collect();
        move |p| m.get(&p).copied()
    }

    #[test]
    fn breaking_a_trommel_half_breaks_the_other() {
        let start = Pos::new(0, 0, 0);
        let end = Pos::new(1, 0, 0);
        // The start broke: the end goes, dropping nothing.
        let w = world(&[(end, true, Dir::East)]);
        assert_eq!(trommel_orphan(start, &w), Some((end, false)));
        // The end broke: the start goes and drops the trommel.
        let w = world(&[(start, false, Dir::East)]);
        assert_eq!(trommel_orphan(end, &w), Some((start, true)));
    }

    #[test]
    fn a_stale_break_leaves_a_rebuilt_trommel_alone() {
        let start = Pos::new(0, 0, 0);
        let end = Pos::new(1, 0, 0);
        let w = world(&[(start, false, Dir::East), (end, true, Dir::East)]);
        assert_eq!(trommel_orphan(start, &w), None);
        assert_eq!(trommel_orphan(end, &w), None);
    }

    #[test]
    fn a_neighbouring_trommel_is_not_a_partner() {
        // Another trommel's end beside the broken block, facing away.
        let w = world(&[(Pos::new(0, 0, 1), true, Dir::East)]);
        assert_eq!(trommel_orphan(Pos::new(0, 0, 0), &w), None);
    }
}
