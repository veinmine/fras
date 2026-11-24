//! The steam plant of one level, free of the game: the parts of specs 2 to 4
//! (boilers, tanks, domes, pipes, valves, vents, cylinders, flywheels,
//! fireboxes and pumps), the multiblocks they form, their pipe networks,
//! steam networks and plants, and the steam step that runs them.
//!
//! The game reports parts as they load, change and go (as for the
//! rotational network), sets each firebox's heat, each pump's water sources
//! and the speeds the rotational network gives, then runs `step` every 4
//! ticks; it reads back the engines' drives, the pumps' loads, events and
//! what clients should show. One `Works` per level.
//!
//! - `part`: the parts and what they save;
//! - `build`: multiblocks, pipe networks, steam networks and plants from the parts;
//! - `step`: the steam step (spec 2, 14.1);
//! - `info`: what the wrench and the gauges read.

pub mod build;
pub mod info;
pub mod part;
pub mod step;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::sim::fluid::Content;
use crate::sim::fluid::route::{Graph, Tree};
use crate::sim::geom::{Chunk, Pos};
use crate::sim::multi::ShellKind;
use crate::sim::multi::shape::{Cuboid, Refusal};
use crate::sim::rot::Drive;
use crate::sim::steam::pool::Pool;

pub use info::{EngineInfo, Gauge, ShellInfo, SteamInfo};
pub use part::{Kind, PipeMat, Saved, Spec};

/// A part in the registry.
#[derive(Clone, Debug)]
pub struct Part {
    pub spec: Spec,
    pub saved: Saved,
    /// What clients were last told (shell warning level and glow).
    pub sent: (u8, bool),
}

/// A formed boiler or tank.
#[derive(Clone, Debug)]
pub struct Shell {
    pub kind: ShellKind,
    pub cuboid: Cuboid,
    /// Its blocks that are loaded.
    pub blocks: Vec<Pos>,
    /// A block of it is damaged: it is inert until every block is repaired.
    pub damaged: bool,
    /// Part of it is in an unloaded chunk: it keeps its saved formation and pauses.
    pub partial: bool,
    /// It ruptured: inert until the game has dealt with its blocks.
    pub ruptured: bool,
    pub water: f64,
    pub temp: f64,
    pub timer: f64,
    /// Tanks: what they hold.
    pub content: Option<Content>,
    /// Boilers: their domes.
    pub domes: Vec<Pos>,
    pub pool: Option<usize>,
    pub plant: usize,
    /// Last step: heat in (kW), from how many fireboxes, boiled and fed (mB/s).
    pub heat: f64,
    pub fireboxes: usize,
    pub boiled: f64,
    pub fed: f64,
}

impl Shell {
    pub fn n(&self) -> u32 {
        self.cuboid.volume() as u32
    }

    pub fn is_boiler(&self) -> bool {
        self.kind.is_boiler()
    }

    /// Working: formed whole, not damaged, not ruptured.
    pub fn works(&self) -> bool {
        !self.damaged && !self.ruptured
    }

    /// Water it holds at most.
    pub fn water_capacity(&self) -> f64 {
        use crate::sim::steam::{BOILER_WATER, TANK_WATER};
        self.n() as f64
            * if self.is_boiler() {
                BOILER_WATER
            } else {
                TANK_WATER
            }
    }

    /// Its steam capacity, mB per bar.
    pub fn steam_capacity(&self) -> f64 {
        use crate::sim::steam::{BOILER_STEAM, TANK_STEAM};
        if !self.works() {
            return 0.0;
        }
        if self.is_boiler() {
            self.n() as f64 * BOILER_STEAM
        } else if self.content == Some(Content::Steam) {
            self.n() as f64 * TANK_STEAM
        } else {
            0.0
        }
    }

    /// Steam capacity of one of its blocks.
    pub fn block_steam_capacity(&self) -> f64 {
        self.steam_capacity() / self.n().max(1) as f64
    }
}

/// An endpoint of a pipe network (spec 3, 3.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum End {
    /// A boiler's domes (steam source), by shell.
    Domes(usize),
    /// A boiler's feed (water consumer), by shell.
    Feed(usize),
    /// A tank (source and consumer of its content), by shell.
    Tank(usize),
    Vent(Pos),
    Cylinder(Pos),
    Pump(Pos),
}

impl End {
    /// The content it gives a network (a tank's depends on what it holds).
    pub fn content(self) -> Option<Content> {
        match self {
            End::Domes(_) | End::Vent(_) | End::Cylinder(_) => Some(Content::Steam),
            End::Feed(_) | End::Pump(_) => Some(Content::Water),
            End::Tank(_) => None,
        }
    }
}

/// A pipe network: pipe parts joined by connected faces, and its endpoints.
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub content: Option<Content>,
    /// It touches steam and water endpoints: inert (the game never connects such faces).
    pub mixed: bool,
    pub pipes: Vec<Pos>,
    pub graph: Graph,
    /// Node of each pipe part.
    pub node_of: BTreeMap<Pos, usize>,
    /// Source nodes (one per source endpoint) and the endpoint.
    pub sources: Vec<(End, usize)>,
    /// Consumer endpoints and the nodes they attach at (all of them).
    pub consumers: Vec<(End, Vec<usize>)>,
    /// Steam: the tree from every source. Water: from the pumps, and from the tanks.
    pub tree: Tree,
    pub tank_tree: Tree,
    /// Positions next to it in unloaded chunks: it pauses.
    pub frontier: Vec<Pos>,
    pub pool: Option<usize>,
    pub plant: usize,
    /// Last step: the flow through each node, mB/s.
    pub flow: Vec<f64>,
}

/// One steam network: its steam, what it is made of and its red line.
#[derive(Clone, Debug, Default)]
pub struct SteamNet {
    pub pool: Pool,
    pub shells: Vec<usize>,
    pub nets: Vec<usize>,
    /// The lowest rating on it and the kind of part that sets it.
    pub red: f64,
    pub red_by: Option<Kind>,
    /// Warning timers of its pipe parts, by kind (one bursts when it fills).
    pub timers: BTreeMap<u8, f64>,
    pub plant: usize,
    /// Last step: steam boiled in and drawn or vented out, mB/s.
    pub steam_in: f64,
    pub steam_out: f64,
}

/// A steam engine: a flywheel and the cylinders driving it.
#[derive(Clone, Debug)]
pub struct Engine {
    pub flywheel: Pos,
    pub cylinders: Vec<Pos>,
    /// Last step, per cylinder: demand (mB/s) and fill.
    pub demand: Vec<f64>,
    pub fill: Vec<f64>,
    pub drive: Drive,
    pub plant: usize,
}

/// Everything linked by shared parts (spec 2, 14.2): it pauses as a whole.
#[derive(Clone, Debug, Default)]
pub struct Plant {
    pub chunks: BTreeSet<Chunk>,
    pub frontier: Vec<Pos>,
    pub partial: bool,
    /// Flywheels and pumps: their rotational networks are linked to it.
    pub rot_parts: Vec<Pos>,
    pub paused: bool,
}

/// What the rotational network tells the plant about a flywheel or pump.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rot {
    pub omega: f64,
    /// Its rotational network (an index that holds until the next rebuild).
    pub net: Option<usize>,
    /// Its network paused by its own rules.
    pub paused: bool,
    pub jammed: bool,
}

/// Something the game should do or show.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A boiler or steam tank ruptured (spec 2, 7.3).
    Rupture {
        kind: ShellKind,
        blocks: Vec<Pos>,
        center: [f64; 3],
        power: f64,
        cloud: f64,
        dry: bool,
    },
    /// A pipe part burst (spec 3, 4.4): the game marks it damaged.
    Burst(Pos),
    /// A cylinder jammed over its rating.
    CylinderJam(Pos),
    /// A block let its steam go (broken, or its boiler unformed).
    Puff(Pos),
    /// An emergency valve popped open, or reseated.
    ValveOpen(Pos),
    ValveShut(Pos),
}

/// A multiblock's formation as the game shows it in each block's state.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Formation {
    pub cuboid: Cuboid,
}

/// The steam plant of one level.
#[derive(Default)]
pub struct Works {
    pub(crate) parts: HashMap<Pos, Part>,
    pub(crate) dirty: bool,
    // ---- Built from the parts ----
    pub(crate) shells: Vec<Shell>,
    pub(crate) shell_of: HashMap<Pos, usize>,
    /// Groups that don't form, by block.
    pub(crate) refused: HashMap<Pos, Refusal>,
    pub(crate) nets: Vec<Net>,
    /// The pipe network of each pipe part and fitting.
    pub(crate) net_of: HashMap<Pos, usize>,
    pub(crate) steam: Vec<SteamNet>,
    pub(crate) engines: Vec<Engine>,
    pub(crate) engine_of: HashMap<Pos, usize>,
    pub(crate) plants: Vec<Plant>,
    /// The plant of parts in no shell, network or engine (fireboxes, domes,
    /// lone flywheels and pumps).
    pub(crate) part_plant: HashMap<Pos, usize>,
    /// The boilers each firebox position heats.
    pub(crate) heats: BTreeMap<Pos, Vec<usize>>,
    // ---- Set by the game ----
    pub(crate) heat: HashMap<Pos, f64>,
    pub(crate) pump_sources: HashMap<Pos, u32>,
    pub(crate) rot: HashMap<Pos, Rot>,
    // ---- Results ----
    pub(crate) events: Vec<Event>,
    pub(crate) drives: BTreeMap<Pos, Drive>,
    pub(crate) pump_loads: BTreeMap<Pos, f64>,
    pub(crate) pump_flow: BTreeMap<Pos, f64>,
    pub(crate) vents: BTreeMap<Pos, VentState>,
    /// Flywheels and pumps whose rotational networks the plant holds paused.
    pub(crate) held: BTreeSet<Pos>,
    /// Shell blocks whose look changed.
    pub(crate) sync: Vec<Pos>,
    /// Formations to tell the blocks (position, its multiblock or none).
    pub(crate) formations: Vec<(Pos, Option<Formation>)>,
    pub(crate) steps: u64,
    /// Rebuilds done (for tests and the profiler).
    pub rebuilds: u64,
}

/// A vent's state for effects.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VentState {
    pub open: bool,
    pub simmer: bool,
    /// mB/s venting now.
    pub rate: f64,
}

impl Works {
    pub fn new() -> Works {
        Works::default()
    }

    // ---- What the game reports -------------------------------------------

    /// A part placed or loaded, with what its block saved.
    pub fn insert(&mut self, pos: Pos, spec: Spec, saved: Option<Saved>) {
        if let Some(p) = self.parts.get(&pos)
            && p.spec == spec
        {
            return;
        }
        self.dissolve();
        let saved = saved.unwrap_or_default();
        self.parts.insert(
            pos,
            Part {
                spec,
                saved,
                sent: (0, false),
            },
        );
        self.dirty = true;
    }

    /// A part's block changed how it is set.
    pub fn update(&mut self, pos: Pos, spec: Spec) {
        let Some(p) = self.parts.get_mut(&pos) else {
            return;
        };
        if p.spec == spec {
            return;
        }
        let rebuild = p.spec.needs_rebuild(&spec);
        if rebuild {
            self.dissolve();
        }
        if let Some(p) = self.parts.get_mut(&pos) {
            p.spec = spec;
        }
        if rebuild {
            self.dirty = true;
        }
    }

    /// A part broken: a shell block lets its steam go.
    pub fn remove(&mut self, pos: Pos) {
        if !self.parts.contains_key(&pos) {
            return;
        }
        self.dissolve();
        if let Some(p) = self.parts.remove(&pos)
            && matches!(p.spec.kind, Kind::Shell(_))
            && p.saved.steam > 1.0
        {
            self.events.push(Event::Puff(pos));
        }
        self.heat.remove(&pos);
        self.pump_sources.remove(&pos);
        self.dirty = true;
    }

    /// Its chunk unloaded: it leaves quietly (its block keeps what it saved).
    pub fn unload(&mut self, pos: Pos) {
        if !self.parts.contains_key(&pos) {
            return;
        }
        self.dissolve();
        self.parts.remove(&pos);
        self.heat.remove(&pos);
        self.pump_sources.remove(&pos);
        self.dirty = true;
    }

    /// A firebox's heat now, kW (0 when out).
    pub fn set_heat(&mut self, pos: Pos, kw: f64) {
        if kw > 0.0 {
            self.heat.insert(pos, kw);
        } else {
            self.heat.remove(&pos);
        }
    }

    /// The water source blocks a pump touches.
    pub fn set_pump_sources(&mut self, pos: Pos, n: u32) {
        self.pump_sources.insert(pos, n);
    }

    /// What the rotational network says about a flywheel or pump.
    pub fn set_rot(&mut self, pos: Pos, rot: Rot) {
        self.rot.insert(pos, rot);
    }

    // ---- Reads -----------------------------------------------------------

    pub fn part(&self, pos: Pos) -> Option<&Part> {
        self.parts.get(&pos)
    }

    pub fn contains(&self, pos: Pos) -> bool {
        self.parts.contains_key(&pos)
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Positions of every flywheel and pump (the game feeds their speeds).
    pub fn rot_parts(&self) -> Vec<Pos> {
        let mut out: Vec<Pos> = self
            .parts
            .iter()
            .filter(|(_, p)| matches!(p.spec.kind, Kind::Flywheel | Kind::Pump))
            .map(|(q, _)| *q)
            .collect();
        out.sort();
        out
    }

    /// The formed multiblock a block belongs to.
    pub fn shell_at(&self, pos: Pos) -> Option<&Shell> {
        self.shell_of.get(&pos).map(|&i| &self.shells[i])
    }

    /// The engine drives to hand the rotational network (flywheel, drive).
    pub fn drives(&self) -> Vec<(Pos, Drive)> {
        self.drives.iter().map(|(p, d)| (*p, *d)).collect()
    }

    /// Pumps' loads, Nm (pumping or idle).
    pub fn pump_loads(&self) -> Vec<(Pos, f64)> {
        self.pump_loads.iter().map(|(p, l)| (*p, *l)).collect()
    }

    /// Vents and burst parts venting now, for effects and scalding.
    pub fn vents(&self) -> Vec<(Pos, VentState)> {
        self.vents.iter().map(|(p, v)| (*p, *v)).collect()
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Blocks whose formation changed: the game sets their states.
    pub fn take_formations(&mut self) -> Vec<(Pos, Option<Formation>)> {
        std::mem::take(&mut self.formations)
    }

    /// Whether the part's plant is paused (or it isn't in one yet).
    pub fn paused(&self, pos: Pos) -> bool {
        self.plant_of(pos).is_none_or(|p| self.plants[p].paused)
    }

    /// The plant a part belongs to.
    pub fn plant_of(&self, pos: Pos) -> Option<usize> {
        if let Some(&s) = self.shell_of.get(&pos) {
            return Some(self.shells[s].plant);
        }
        if let Some(&n) = self.net_of.get(&pos) {
            return Some(self.nets[n].plant);
        }
        if let Some(&e) = self.engine_of.get(&pos) {
            return Some(self.engines[e].plant);
        }
        self.part_plant.get(&pos).copied()
    }
}
