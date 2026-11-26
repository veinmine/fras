//! A line machine's built-in belt (machines spec 2.4): one block long, driven
//! by the machine, holding two items. Items come in at the input end (0),
//! stop at the work point (the middle) when the machine has a recipe for
//! them, turn into the result there, and leave at the output end (1) with a
//! belt end's rules. Byproducts follow the main result out, one at a time;
//! the machine takes no new item until they are all out.
//!
//! Generic over the item type, as `sim::belt`: the game keeps item stacks,
//! the tests strings.

use super::{BeltTick, BeltWork, belt_tick};
use crate::sim::grind::{Items, MachineState, Recipe, Status};

/// Items keep this far apart, blocks (a belt's spacing).
pub const SPACING: f64 = crate::sim::belt::SPACING;
/// Where the machine works on an item, blocks from the input end.
pub const WORK_POINT: f64 = 0.5;
/// Items the built-in belt holds.
pub const CAPACITY: usize = 2;
const EPS: f64 = 1e-9;

/// One item on the built-in belt.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry<T> {
    pub item: T,
    /// Blocks from the input end, 0 to 1.
    pub x: f64,
    /// The machine is done with it (worked, or it has no recipe): it moves on.
    pub done: bool,
}

/// The built-in belt and the work on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Station<T> {
    /// Sorted by position, ascending.
    pub entries: Vec<Entry<T>>,
    pub work: BeltWork,
    /// Results waiting to follow the main one out.
    pub pending: Vec<T>,
}

impl<T> Default for Station<T> {
    fn default() -> Self {
        Station {
            entries: Vec::new(),
            work: BeltWork::default(),
            pending: Vec::new(),
        }
    }
}

/// What the station needs from the world.
pub trait World<T> {
    /// The item's id, for recipes.
    fn id(&self, item: &T) -> String;
    /// An item of `id`, for results.
    fn make(&self, id: &str) -> Option<T>;
}

/// One tick of the station.
#[derive(Clone, Debug, PartialEq)]
pub struct StationTick {
    pub load: f64,
    pub status: Status,
    /// Revolutions the current recipe needs.
    pub needed: f64,
    pub water_used: f64,
    /// A recipe finished.
    pub completed: bool,
    /// Entries were added, removed, held or released (clients need a snapshot).
    pub changed: bool,
}

impl<T: Clone> Station<T> {
    /// Whether an item can come in at the input end now.
    pub fn has_room(&self) -> bool {
        self.entries.len() < CAPACITY && self.entries.first().is_none_or(|e| e.x >= SPACING - EPS)
    }

    /// Put an item on at the input end (a belt, a hopper, a hand).
    pub fn insert(&mut self, item: T) -> Result<(), T> {
        if !self.has_room() {
            return Err(item);
        }
        self.entries.insert(
            0,
            Entry {
                item,
                x: 0.0,
                done: false,
            },
        );
        Ok(())
    }

    /// The item at the work point the machine hasn't finished with.
    pub fn held(&self) -> Option<&Entry<T>> {
        self.entries
            .iter()
            .find(|e| !e.done && (e.x - WORK_POINT).abs() < EPS)
    }

    /// The item at the output end, ready to be offered to the block beyond
    /// (the game offers it before the tick, `pop_ready` when it was taken).
    pub fn ready(&self) -> Option<&T> {
        self.entries
            .last()
            .filter(|e| e.x >= 1.0 - EPS)
            .map(|e| &e.item)
    }

    pub fn pop_ready(&mut self) -> Option<T> {
        self.ready()?;
        self.entries.pop().map(|e| e.item)
    }

    /// Every item on it, for breaking the machine.
    pub fn take_all(&mut self) -> Vec<T> {
        let mut v: Vec<T> = self.entries.drain(..).map(|e| e.item).collect();
        v.append(&mut self.pending);
        self.work = BeltWork::default();
        v
    }

    /// One tick: place a waiting byproduct, work on the held item, move the
    /// rest. `step` is the belt's advance this tick,
    /// blocks (0 when the machine stands still).
    pub fn tick(
        &mut self,
        world: &mut dyn World<T>,
        state: &MachineState,
        recipes: &[Recipe],
        items: &dyn Items,
        random: &mut dyn FnMut() -> f64,
        step: f64,
    ) -> StationTick {
        let mut out = StationTick {
            load: 0.0,
            status: idle_status(state),
            needed: 0.0,
            water_used: 0.0,
            completed: false,
            changed: false,
        };
        let moving = step > EPS && !state.jammed && !state.paused;

        // 1. The front item still at the output end: the block beyond refused it.
        if moving && self.ready().is_some() {
            out.status = Status::OutputFull;
        }

        // 2. A byproduct onto the work point once there's room.
        if !self.pending.is_empty()
            && self
                .entries
                .iter()
                .all(|e| (e.x - WORK_POINT).abs() >= SPACING - EPS)
            && self.entries.len() < CAPACITY
        {
            let item = self.pending.remove(0);
            let at = self.entries.partition_point(|e| e.x < WORK_POINT);
            self.entries.insert(
                at,
                Entry {
                    item,
                    x: WORK_POINT,
                    done: true,
                },
            );
            out.changed = true;
        }

        // 3. Work on the item at the work point.
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| !e.done && (e.x - WORK_POINT).abs() < EPS)
        {
            let id = world.id(&self.entries[i].item);
            if !super::belt_accepts(&id, recipes, items) {
                self.entries[i].done = true;
                out.changed = true;
            } else if self.pending.is_empty() {
                let t: BeltTick = belt_tick(&mut self.work, &id, state, recipes, items, random);
                out.load = t.load;
                out.status = t.status;
                out.needed = t.needed;
                out.water_used = t.water_used;
                if let Some(main) = t.done {
                    out.completed = true;
                    out.changed = true;
                    let mut rest: Vec<T> = std::mem::take(&mut self.work.pending)
                        .iter()
                        .filter_map(|r| world.make(r))
                        .collect();
                    match main.and_then(|m| world.make(&m)) {
                        Some(result) => {
                            self.entries[i].item = result;
                            self.entries[i].done = true;
                        }
                        None if !rest.is_empty() => {
                            self.entries[i].item = rest.remove(0);
                            self.entries[i].done = true;
                        }
                        None => {
                            self.entries.remove(i);
                        }
                    }
                    self.pending.extend(rest);
                }
            }
        }

        // 4. Move, front to back: keep the spacing, stop at the work point
        // until done, wait at the input while byproducts are still to come.
        if moving {
            let mut ahead: Option<f64> = None;
            for e in self.entries.iter_mut().rev() {
                let mut limit = ahead.map_or(1.0, |a| a - SPACING).min(1.0);
                if !e.done {
                    let stop = if self.pending.is_empty() {
                        WORK_POINT
                    } else {
                        0.0
                    };
                    if e.x <= stop + EPS {
                        limit = limit.min(stop);
                    }
                }
                let x = (e.x + step).min(limit).max(e.x);
                if !e.done && (e.x - WORK_POINT).abs() >= EPS && (x - WORK_POINT).abs() < EPS {
                    out.changed = true;
                }
                e.x = x;
                ahead = Some(e.x);
            }
        }
        if out.status == Status::Idle && !self.entries.is_empty() && !moving {
            out.status = idle_status(state);
        }
        out
    }
}

/// The status of a machine with nothing to work on.
pub fn idle_status(state: &MachineState) -> Status {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::machine::{Machine, hammering, washing};

    struct NoTags;
    impl Items for NoTags {
        fn has_tag(&self, _: &str, _: &str) -> bool {
            false
        }
        fn max_stack(&self, _: &str) -> u32 {
            64
        }
    }

    struct W {
        out: Vec<String>,
        open: bool,
    }
    impl World<String> for W {
        fn id(&self, item: &String) -> String {
            item.clone()
        }
        fn make(&self, id: &str) -> Option<String> {
            Some(id.to_string())
        }
    }

    impl W {
        fn deliver(&mut self, st: &mut Station<String>) {
            if self.open
                && let Some(item) = st.pop_ready()
            {
                self.out.push(item);
            }
        }
    }

    fn state(m: Machine, rpm: f64) -> MachineState {
        MachineState {
            rpm,
            jammed: false,
            paused: false,
            range: m.range(),
            water: (m == Machine::Washer).then_some(1000.0),
        }
    }

    fn run(
        st: &mut Station<String>,
        w: &mut W,
        s: &MachineState,
        recipes: &[Recipe],
        ticks: usize,
        feed: &mut Vec<String>,
    ) {
        let step = crate::sim::belt::blocks_per_tick(s.rpm).abs();
        for _ in 0..ticks {
            if let Some(next) = feed.first().cloned()
                && st.insert(next).is_ok()
            {
                feed.remove(0);
            }
            w.deliver(st);
            st.tick(w, s, recipes, &NoTags, &mut || 0.0, step);
        }
    }

    #[test]
    fn ingots_become_plates_and_other_items_pass_through() {
        let recipes = hammering();
        let mut st = Station::default();
        let mut w = W {
            out: vec![],
            open: true,
        };
        let mut feed = vec![
            "minecraft:iron_ingot".to_string(),
            "minecraft:dirt".to_string(),
            "minecraft:iron_ingot".to_string(),
        ];
        run(
            &mut st,
            &mut w,
            &state(Machine::TripHammer, 64.0),
            &recipes,
            400,
            &mut feed,
        );
        assert_eq!(
            w.out,
            ["fras:iron_plate", "minecraft:dirt", "fras:iron_plate"],
            "in order, dirt untouched"
        );
        assert!(st.entries.is_empty());
    }

    #[test]
    fn byproducts_follow_the_main_result_and_the_next_item_waits() {
        // A rod gives 8 rivets: one replaces the rod, 7 follow it out.
        let recipes = hammering();
        let mut st = Station::default();
        let mut w = W {
            out: vec![],
            open: true,
        };
        let mut feed = vec![
            "fras:iron_rod".to_string(),
            "minecraft:iron_ingot".to_string(),
        ];
        run(
            &mut st,
            &mut w,
            &state(Machine::TripHammer, 64.0),
            &recipes,
            600,
            &mut feed,
        );
        let rivets = w.out.iter().filter(|s| *s == "fras:rivet").count();
        assert_eq!(rivets, 8);
        assert_eq!(
            w.out.last().map(String::as_str),
            Some("fras:iron_plate"),
            "the ingot after every rivet"
        );
    }

    #[test]
    fn a_blocked_output_backs_the_machine_up() {
        let recipes = hammering();
        let mut st = Station::default();
        let mut w = W {
            out: vec![],
            open: false,
        };
        let mut feed: Vec<String> = (0..5).map(|_| "minecraft:iron_ingot".to_string()).collect();
        let s = state(Machine::TripHammer, 64.0);
        run(&mut st, &mut w, &s, &recipes, 600, &mut feed);
        assert_eq!(st.entries.len(), CAPACITY, "full");
        assert!(!st.has_room());
        assert_eq!(feed.len(), 3, "the rest wait upstream");
        let t = st.tick(&mut w, &s, &recipes, &NoTags, &mut || 0.0, 0.1);
        assert_eq!(t.status, Status::OutputFull);
        w.open = true;
        run(&mut st, &mut w, &s, &recipes, 1500, &mut feed);
        assert_eq!(w.out.len(), 5, "nothing lost");
    }

    #[test]
    fn a_dry_washer_holds_its_item_at_the_work_point() {
        let recipes = washing();
        let mut st = Station::default();
        let mut w = W {
            out: vec![],
            open: true,
        };
        let mut dry = state(Machine::Washer, 32.0);
        dry.water = Some(0.0);
        let mut feed = vec!["fras:crushed_iron".to_string()];
        run(&mut st, &mut w, &dry, &recipes, 300, &mut feed);
        assert!(w.out.is_empty());
        assert!(st.held().is_some(), "held, not passed");
        let wet = state(Machine::Washer, 32.0);
        run(&mut st, &mut w, &wet, &recipes, 300, &mut feed);
        assert_eq!(w.out.first().map(String::as_str), Some("fras:iron_dust"));
    }

    #[test]
    fn a_still_machine_moves_nothing() {
        let recipes = hammering();
        let mut st = Station::default();
        let mut w = W {
            out: vec![],
            open: true,
        };
        st.insert("minecraft:dirt".to_string()).unwrap();
        let s = state(Machine::TripHammer, 0.0);
        for _ in 0..100 {
            st.tick(&mut w, &s, &recipes, &NoTags, &mut || 0.0, 0.0);
        }
        assert_eq!(st.entries[0].x, 0.0);
        assert!(w.out.is_empty());
    }
}
