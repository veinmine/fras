//! The server's side of the networks: one `Mechanics` per level, fed by the
//! block entities' load and unload and by block changes, ticked after each
//! level tick, and written back to the parts' block entities when what
//! clients show changed (rotational network spec 10).
//!
//! Block hooks often get a read-only level, so they only queue work here;
//! `level_tick` does it with a host that can write.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use mod_api::{Host, LevelRef, ServerHost};

use super::blocks::{self, Block};
use crate::sim::geom::{Chunk, Pos};
use crate::sim::rot::part::{CRANK_STALL, CRANK_TOP, motor_drive, motor_ramp};
use crate::sim::rot::{Drive, Mechanics, Spec, View};

/// A level, by its dimension id (`minecraft:overworld`).
pub type Dim = &'static str;

pub const VANILLA_LEVELS: [Dim; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];

/// A crank being turned: ticks of push left and the direction.
#[derive(Copy, Clone, Debug)]
pub struct Cranking {
    pub ticks: u32,
    pub dir: f64,
}

/// One level's state.
#[derive(Default)]
pub struct Level {
    pub mech: Mechanics,
    pub cranks: HashMap<Pos, Cranking>,
    /// Parts to read from the world again, and whether a player caused the
    /// change (a placement: joining may shock it).
    pub pending: BTreeMap<Pos, bool>,
    /// Pulleys whose belt partner broke: clear their belt.
    pub clear_belts: BTreeSet<Pos>,
    /// Bearings whose mount may have changed.
    pub bearings: BTreeSet<Pos>,
    /// Clutches whose redstone input may have changed.
    pub clutches: BTreeSet<Pos>,
    /// Gearboxes whose redstone may have changed; true when just placed
    /// (it takes the signal as it is without flipping).
    pub gearboxes: BTreeMap<Pos, bool>,
    /// Large pulleys just placed: check their plane is clear.
    pub large_pulleys: BTreeSet<Pos>,
    /// Water wheels whose water may have changed, and their torque.
    pub wheels: BTreeSet<Pos>,
    pub wheel_torque: HashMap<Pos, f64>,
    /// Grinder loads reported by their block entity this tick.
    pub loads: HashMap<Pos, f64>,
    /// Comparator values of tachometers, by position.
    pub tachometers: HashMap<Pos, i32>,
    /// Comparator values of grinders.
    pub grinder_signal: HashMap<Pos, i32>,
    /// The steam plant (0.2).
    pub steam: super::steam::Steam,
    /// Creative engines' settings this tick (signed RPM), from their block
    /// entities, and the speed each is ramping through.
    pub motors: HashMap<Pos, f64>,
    pub motor_ramps: HashMap<Pos, f64>,
    /// Gauges' histories for their screens.
    pub histories: HashMap<Pos, super::readout::History>,
}

#[derive(Default)]
pub struct Server {
    pub levels: HashMap<Dim, Level>,
    /// Leather belts in hand: the first pulley a player clicked.
    pub belt_starts: HashMap<u128, (Dim, Pos)>,
    /// Pulleys whose belt partner broke, from a hook that doesn't know the
    /// level: (pulley, the broken partner).
    pub orphan_belts: Vec<(Pos, Pos)>,
    pub ticks: u64,
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

pub fn with<R>(f: impl FnOnce(&mut Server) -> R) -> R {
    let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Server::default))
}

pub fn with_level<R>(dim: Dim, f: impl FnOnce(&mut Level) -> R) -> R {
    with(|s| f(s.levels.entry(dim).or_default()))
}

/// Forget everything (server start and stop).
pub fn clear() {
    *SERVER.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The dimension id of a level handle.
pub fn dim_of(host: &dyn Host, level: LevelRef) -> Option<Dim> {
    let known: Vec<Dim> = with(|s| s.levels.keys().copied().collect());
    VANILLA_LEVELS
        .into_iter()
        .chain(known)
        .find(|d| host.level(d) == Some(level))
        .or_else(|| {
            let d = host.dimension();
            (host.level(d) == Some(level)).then_some(d)
        })
}

/// The game's world, as the network reads it.
pub struct GameView<'a> {
    pub host: &'a dyn Host,
    pub level: LevelRef,
}

impl View for GameView<'_> {
    fn loaded(&self, pos: Pos) -> bool {
        self.level.is_loaded(self.host, pos.to_array())
    }

    fn simulated(&self, chunk: Chunk) -> bool {
        let pos = [chunk.x * 16 + 8, 64, chunk.z * 16 + 8];
        self.level.is_loaded(self.host, pos)
            && self.level.should_tick_blocks_at(self.host, pos) == Some(true)
    }
}

/// A position's block property, read from the level.
pub fn property(host: &dyn Host, level: LevelRef, pos: Pos, key: &str) -> Option<String> {
    level.block_property(host, pos.to_array(), key)
}

/// Which of FRAS's blocks stands at `pos`.
pub fn block_at(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<Block> {
    let block = level.block(host, pos.to_array())?;
    Block::from_id(&block.id(host))
}

/// Read a part's block from the level: its block, spec and saved data.
pub fn read_part(
    host: &dyn Host,
    level: LevelRef,
    pos: Pos,
) -> Option<(Block, Spec, mod_api::Compound)> {
    let b = block_at(host, level, pos)?;
    let get = |key: &str| property(host, level, pos, key);
    let nbt = host
        .block_entity(level, pos.to_array())
        .and_then(|e| e.data(host))
        .unwrap_or_default();
    let spec = b.spec(&get, &nbt)?;
    Some((b, spec, nbt))
}

/// Merge fields into a block entity's saved data (and send them to clients).
pub fn merge(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, fields: mod_api::Compound) {
    if let Some(entity) = host.block_entity(level, pos.to_array()) {
        // Merging reloads the block entity, which zeroes its menu data until
        // its next tick writes it again: an open screen flashed all zeros.
        // Put the data back at once.
        let data = entity
            .data_slots(&*host)
            .filter(|d| d.iter().any(|v| *v != 0));
        let _ = entity.merge_data(host, fields);
        if let Some(data) = data
            && entity.data_slots(&*host).as_ref() != Some(&data)
        {
            let _ = entity.set_data_slots(host, data);
        }
    }
}

/// After a level's tick: do the queued work, run its networks and tell the parts.
pub fn level_tick(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(dim) = dim_of(&*host, level) else {
        return;
    };
    let ticks = with(|s| {
        s.ticks += 1;
        s.ticks
    });
    super::world::queued(host, level, dim);
    if ticks % 40 == 0 {
        // Water wheels read their water again every 40 ticks (spec 8.2).
        with_level(dim, |l| {
            let wheels: Vec<Pos> = l.wheel_torque.keys().copied().collect();
            l.wheels.extend(wheels);
        });
    }
    super::world::wheels(host, level, dim);
    super::steam::queued(host, level, dim);
    if ticks % 40 == 0 {
        // Flywheels check their plane every 2 s (a block placed diagonally
        // gives them no neighbour update).
        with_level(dim, |l| {
            let fly: Vec<Pos> = l
                .steam
                .works
                .rot_parts()
                .into_iter()
                .filter(|q| {
                    l.mech
                        .part(*q)
                        .is_some_and(|p| p.spec.kind == crate::sim::rot::Kind::Flywheel)
                })
                .collect();
            l.steam.flywheels.extend(fly);
        });
    }
    super::steam::before_rot(dim);

    let (events, sync) = {
        let view = GameView {
            host: &*host,
            level,
        };
        with_level(dim, |l| {
            // Cranks push while their time lasts.
            l.cranks.retain(|_, c| c.ticks > 0);
            let mut stopped = Vec::new();
            for (pos, c) in l.cranks.iter_mut() {
                c.ticks -= 1;
                if c.ticks == 0 {
                    stopped.push(*pos);
                }
                l.mech.set_drive(
                    *pos,
                    Drive {
                        stall: CRANK_STALL,
                        top: CRANK_TOP,
                        dir: c.dir,
                    },
                );
            }
            for pos in stopped {
                l.mech.set_drive(pos, Drive::default());
            }
            for (pos, torque) in l.wheel_torque.iter() {
                let drive = if torque.abs() > 1e-9 {
                    Drive {
                        stall: torque.abs(),
                        top: crate::sim::rot::part::WHEEL_TOP,
                        dir: torque.signum(),
                    }
                } else {
                    Drive::default()
                };
                l.mech.set_drive(*pos, drive);
            }
            // Creative engines ramp toward their setting from the speed they
            // turn at, so a reload or a resumed network doesn't jolt them.
            let motors = std::mem::take(&mut l.motors);
            let gone: Vec<Pos> = l
                .motor_ramps
                .keys()
                .filter(|q| !motors.contains_key(q))
                .copied()
                .collect();
            for pos in gone {
                l.motor_ramps.remove(&pos);
                l.mech.set_drive(pos, Drive::default());
            }
            for (pos, target) in motors {
                let now = l.mech.speed(pos).map_or(0.0, |s| s.0);
                let ramp = l.motor_ramps.get(&pos).copied().unwrap_or(now);
                let ramp = motor_ramp(ramp, target, crate::sim::rot::solve::DT);
                l.motor_ramps.insert(pos, ramp);
                l.mech.set_drive(pos, motor_drive(ramp));
            }
            for (pos, load) in l.loads.drain() {
                l.mech.set_load(pos, load);
            }
            l.mech.tick(&view);
            (l.mech.take_events(), l.mech.take_sync())
        })
    };
    super::steam::tick_governors(dim);
    super::rust::ticks(host, level, dim);
    for event in events {
        super::effects::event(host, level, &event);
    }
    super::steam::after_rot(host, level, dim, ticks);
    if ticks % 10 == 0 {
        super::effects::warnings(host, level, dim);
    }
    for (pos, sent) in sync {
        let (over, shear) = with_level(dim, |l| {
            l.mech
                .part(pos)
                .map_or((0.0, 0.0), |p| (p.over_t, p.shear_t))
        });
        let fields = blocks::part_fields(sent.rpm, sent.jam, sent.warn, sent.sag, over, shear);
        merge(host, level, pos, fields);
    }
}
