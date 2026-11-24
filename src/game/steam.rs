//! The server's side of the steam plant (steam and heat spec 14, fluids
//! spec 11, multiblocks spec 9): one `Works` per level, fed by the plant
//! blocks' load, unload and changes; stepped every 4 ticks after the
//! rotational solve; its results written back to blocks, the rotational
//! network and the world (explosions, scalding, effects).
//!
//! As in `server`, hooks only queue work here and the level tick does it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU8, Ordering};

use mod_api::{
    Compound, ExplosionInteraction, ExplosionSettings, Host, HostBlockStates, LevelRef,
    ParticleOptions, ServerHost, SoundSource,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, GameView, block_at, with_level};
use super::world::{break_block, set_property};
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::multi::engine as engine_geom;
use crate::sim::plant::{Event, Rot, Spec, Works};
use crate::sim::rot::Jam;
use crate::sim::steam::{STEP_TICKS, fail};

/// A scalding steam cloud left by a rupture (spec 2, 7.6).
#[derive(Clone, Debug)]
pub struct Cloud {
    pub center: [f64; 3],
    pub radius: f64,
    pub ticks: u32,
}

/// One level's steam plant and its queued work.
#[derive(Default)]
pub struct Steam {
    pub works: Works,
    /// Plant parts to read from the world again, and whether a player placed them.
    pub pending: BTreeMap<Pos, bool>,
    /// Blocks just placed: connect the pipes around them (fluids spec 3.3).
    pub connect: BTreeSet<Pos>,
    /// Pumps whose water sources may have changed.
    pub pumps: BTreeSet<Pos>,
    /// Flywheels whose plane may have changed.
    pub flywheels: BTreeSet<Pos>,
    /// Redstone valves whose signal may have changed.
    pub redstone: BTreeSet<Pos>,
    /// Fireboxes whose lit state changed.
    pub lit: BTreeMap<Pos, bool>,
    /// Comparator values of gauges and sight glasses.
    pub signals: HashMap<Pos, i32>,
    pub clouds: Vec<Cloud>,
    /// Short scalding puffs: position and ticks left.
    pub puffs: Vec<(Pos, u32)>,
}

// ---- Explosion mode (spec 2, 7.5) -------------------------------------------------

/// The explosion mode: 0 full, 1 contained, 2 off. The setting lives on the
/// settings page, which only the client reads; the client copies it here
/// each tick, so the integrated server follows it. A dedicated server keeps
/// the default (full).
static EXPLOSION_MODE: AtomicU8 = AtomicU8::new(0);

pub const SETTING: &str = "explosion_mode";
pub const MODES: [&str; 3] = [
    "fras.settings.explosion.full",
    "fras.settings.explosion.contained",
    "fras.settings.explosion.off",
];

pub fn set_explosion_mode(mode: u8) {
    EXPLOSION_MODE.store(mode.min(2), Ordering::Relaxed);
}

pub fn explosion_mode() -> u8 {
    EXPLOSION_MODE.load(Ordering::Relaxed)
}

// ---- Reading the world -------------------------------------------------------------

/// A plant part's block from the level: its block, spec and saved data.
pub fn read_plant(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<(Block, Spec, Compound)> {
    let b = block_at(host, level, pos)?;
    let get = |key: &str| server::property(host, level, pos, key);
    let nbt = host
        .block_entity(level, pos.to_array())
        .and_then(|e| e.data(host))
        .unwrap_or_default();
    let spec = b.plant_spec(&get, &nbt)?;
    Some((b, spec, nbt))
}

/// Queue a plant block for reading.
pub fn queue(dim: Dim, pos: Pos, placed: bool) {
    with_level(dim, |l| {
        let e = l.steam.pending.entry(pos).or_insert(false);
        *e |= placed;
        if placed {
            l.steam.connect.insert(pos);
        }
    });
}

/// The queued work of one level, before the rotational tick.
pub fn queued(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let (redstone, lit) = with_level(dim, |l| {
        (
            std::mem::take(&mut l.steam.redstone),
            std::mem::take(&mut l.steam.lit),
        )
    });
    for pos in redstone {
        if block_at(&*host, level, pos) == Some(Block::RedstoneValve) {
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
    for (pos, on) in lit {
        if block_at(&*host, level, pos) == Some(Block::Firebox) {
            set_property(host, level, pos, "lit", if on { "true" } else { "false" });
        }
    }
    let (pending, connect, pumps, flywheels) = with_level(dim, |l| {
        (
            std::mem::take(&mut l.steam.pending),
            std::mem::take(&mut l.steam.connect),
            std::mem::take(&mut l.steam.pumps),
            std::mem::take(&mut l.steam.flywheels),
        )
    });
    let mut new_vents = Vec::new();
    for (pos, placed) in pending {
        let read = read_plant(&*host, level, pos);
        if let Some((Block::EmergencyValve, spec, nbt)) = &read
            && placed
            && blocks::get_f64(nbt, "set").is_none()
        {
            new_vents.push((pos, spec.dir));
        }
        with_level(dim, |l| match read {
            None => {
                l.steam.works.remove(pos);
            }
            Some((block, spec, nbt)) => {
                let w = &mut l.steam.works;
                if w.contains(pos) {
                    w.update(pos, spec);
                } else {
                    w.insert(pos, spec, blocks::plant_saved(&nbt));
                }
                match block {
                    Block::Pump => {
                        l.steam.pumps.insert(pos);
                    }
                    Block::Flywheel => {
                        l.steam.flywheels.insert(pos);
                    }
                    _ => {}
                }
            }
        });
    }
    for pos in connect {
        super::pipes::connect_around(host, level, dim, pos);
    }
    // A new emergency valve opens half a bar under the red line of what it
    // sits on (fluids spec 6.4).
    for (pos, facing) in new_vents {
        let red = with_level(dim, |l| l.steam.works.gauge(pos.offset(facing)).red);
        let set = crate::sim::fluid::valve::default_set((red > 0.0).then_some(red));
        let mut c = Compound::new();
        blocks::put_f64(&mut c, "set", set);
        server::merge(host, level, pos, c);
        queue(dim, pos, false);
    }
    for pos in pumps {
        if block_at(&*host, level, pos) != Some(Block::Pump) {
            continue;
        }
        let n = pump_sources(&*host, level, pos);
        with_level(dim, |l| l.steam.works.set_pump_sources(pos, n));
    }
    for pos in flywheels {
        if let Some(blocker) = flywheel_blocker(&*host, level, pos) {
            with_level(dim, |l| l.mech.jam_part(pos, Jam::Blocked));
            let _ = blocker;
        }
    }
}

/// Water source blocks among a pump's four intake faces (fluids spec 7.2).
pub fn pump_sources(host: &dyn Host, level: LevelRef, pos: Pos) -> u32 {
    let socket = server::property(host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::North);
    Dir::ALL
        .into_iter()
        .filter(|d| d.axis() != socket.axis())
        .filter(|d| {
            level
                .fluid_state(host, pos.offset(*d).to_array())
                .is_some_and(|f| f.source && f.amount > 0 && f.fluid.id(host).ends_with("water"))
        })
        .count() as u32
}

/// The first solid block in a flywheel's plane, if any (multiblocks spec 4.1).
pub fn flywheel_blocker(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<Pos> {
    let axis = server::property(host, level, pos, "axis")
        .as_deref()
        .and_then(Axis::from_name)
        .unwrap_or(Axis::X);
    let states = host.block_states();
    engine_geom::rim(pos, axis).into_iter().find(|q| {
        !level
            .block_state(host, q.to_array())
            .is_some_and(|s| states.replaceable(s) == Some(true))
            && level.is_loaded(host, q.to_array())
    })
}

// ---- The step ----------------------------------------------------------------------

/// Before the rotational tick: the plant's holds, drives and loads.
pub fn before_rot(dim: Dim) {
    with_level(dim, |l| {
        l.mech.set_held(l.steam.works.held_parts().clone());
        for (pos, drive) in l.steam.works.drives() {
            l.mech.set_drive(pos, drive);
        }
        for (pos, load) in l.steam.works.pump_loads() {
            l.mech.set_load(pos, load);
        }
    });
}

/// After the rotational tick: every 4 ticks, the steam step and what follows.
pub fn after_rot(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, ticks: u64) {
    if ticks.is_multiple_of(10) {
        scald(host, level, dim);
    }
    clouds(host, level, dim);
    if !ticks.is_multiple_of(STEP_TICKS) {
        return;
    }
    let (events, formations, sync, saves) = {
        let view = GameView {
            host: &*host,
            level,
        };
        with_level(dim, |l| {
            for q in l.steam.works.rot_parts() {
                let part = l.mech.part(q);
                let jammed = part.is_some_and(|p| p.jam.is_some());
                let rot = match (part, l.mech.own_pause(q)) {
                    (None, _) => Rot {
                        paused: true,
                        ..Rot::default()
                    },
                    (Some(_), Some((net, paused))) => Rot {
                        omega: l.mech.speed(q).map_or(0.0, |s| s.0),
                        net: Some(net),
                        paused,
                        jammed,
                    },
                    (Some(_), None) => Rot {
                        jammed,
                        ..Rot::default()
                    },
                };
                l.steam.works.set_rot(q, rot);
            }
            l.steam.works.step(&view);
            let saves = if ticks.is_multiple_of(100) {
                shell_saves(&l.steam.works)
            } else {
                Vec::new()
            };
            (
                l.steam.works.take_events(),
                l.steam.works.take_formations(),
                l.steam.works.take_sync(),
                saves,
            )
        })
    };
    for (pos, f) in formations {
        apply_formation(host, level, dim, pos, f.map(|f| f.cuboid));
    }
    for event in events {
        apply_event(host, level, dim, event);
    }
    for pos in sync {
        let (level_, glow) = with_level(dim, |l| {
            l.steam.works.part(pos).map_or((0, false), |p| p.sent)
        });
        let mut c = Compound::new();
        blocks::put_i32(&mut c, "pwarn", level_ as i32);
        blocks::put_bool(&mut c, "glow", glow);
        server::merge(host, level, pos, c);
    }
    for (pos, fields) in saves {
        server::merge(host, level, pos, fields);
    }
    if ticks.is_multiple_of(20) {
        warnings(host, level, dim);
    }
    vent_effects(host, level, dim);
}

/// Every shell block's share, to save (every 5 s).
fn shell_saves(works: &Works) -> Vec<(Pos, Compound)> {
    let mut out = Vec::new();
    for q in shell_blocks(works) {
        if let Some(s) = works.saved(q) {
            out.push((q, blocks::plant_fields(&s)));
        }
    }
    out
}

fn shell_blocks(works: &Works) -> Vec<Pos> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for q in works.shell_positions() {
        if seen.insert(q) {
            out.push(q);
        }
    }
    out
}

/// Tell a block its multiblock (spec 4, 6.1): the formed flag, the joined
/// faces and the long axis in its state, the formation in its saved data.
fn apply_formation(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    cuboid: Option<crate::sim::multi::shape::Cuboid>,
) {
    let Some(block) = block_at(&*host, level, pos) else {
        return;
    };
    let saved = with_level(dim, |l| l.steam.works.saved(pos));
    if let Some(s) = saved {
        server::merge(host, level, pos, blocks::plant_fields(&s));
    }
    if block == Block::SteamCylinder {
        let v = if cuboid.is_some() { "true" } else { "false" };
        set_property(host, level, pos, "formed", v);
        return;
    }
    if !block.is_shell() {
        return;
    }
    let mut props: Vec<(String, String)> = Vec::new();
    match cuboid {
        Some(c) => {
            props.push(("formed".into(), "true".into()));
            props.push(("axis".into(), c.long_axis().name().into()));
            for d in Dir::ALL {
                let joined = c.contains(pos.offset(d));
                props.push((format!("joined_{}", d.name()), joined.to_string()));
            }
        }
        None => {
            props.push(("formed".into(), "false".into()));
            for d in Dir::ALL {
                props.push((format!("joined_{}", d.name()), "false".into()));
            }
        }
    }
    let refs: Vec<(&str, &str)> = props
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    super::world::set_properties(host, level, pos, &refs);
}

fn center(pos: Pos) -> [f64; 3] {
    [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5]
}

fn sound_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    at: [f64; 3],
    id: &str,
    volume: f32,
    pitch: f32,
) {
    level.play_sound(host, id, at, SoundSource::Blocks, volume, pitch);
}

fn puff_particles(
    host: &mut dyn ServerHost,
    level: LevelRef,
    at: [f64; 3],
    spread: f64,
    count: i32,
    speed: f64,
) {
    let Some(keys) = super::keys() else {
        return;
    };
    let options = ParticleOptions::simple(keys.cloud);
    level.send_particles(
        host,
        &options,
        None,
        at,
        [spread, spread, spread],
        speed,
        count,
    );
}

fn random() -> f64 {
    use std::sync::atomic::AtomicU64;
    static STATE: AtomicU64 = AtomicU64::new(0x2545_F491_4F6C_DD1D);
    let mut x = STATE.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// What the plant reported this step.
fn apply_event(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, event: Event) {
    match event {
        Event::Rupture {
            blocks: shell,
            center: at,
            power,
            cloud,
            ..
        } => {
            let mode = explosion_mode();
            sound_at(
                host,
                level,
                at,
                "minecraft:entity.generic.explode",
                2.0,
                0.7,
            );
            puff_particles(host, level, at, cloud * 0.5, 120, 0.15);
            if mode == 2 {
                // Off: every block goes into the damaged state.
                for q in &shell {
                    set_property(host, level, *q, "damaged", "true");
                }
            } else {
                // The multiblock's own blocks go, dropping with the
                // vanilla explosion chance.
                // Its domes, gauges and sight glasses count as the boiler's
                // (multiblocks spec 3.2): they go with it.
                let mut parts = shell.clone();
                for q in &shell {
                    for d in Dir::ALL {
                        let r = q.offset(d);
                        let attached = match block_at(&*host, level, r) {
                            Some(Block::SteamDome) => d == Dir::Up,
                            Some(Block::PressureGauge | Block::SightGlass) => {
                                server::property(&*host, level, r, "facing").as_deref()
                                    == Some(d.name())
                            }
                            _ => false,
                        };
                        if attached && !parts.contains(&r) {
                            parts.push(r);
                        }
                    }
                }
                for q in &parts {
                    let drop = random() < 1.0 / power.max(1.0);
                    let _ = level.destroy_block(host, q.to_array(), drop, None, false);
                }
                let interaction = if mode == 0 {
                    ExplosionInteraction::Block
                } else {
                    ExplosionInteraction::None
                };
                let _ = level.explode(
                    host,
                    None,
                    at,
                    power as f32,
                    false,
                    interaction,
                    ExplosionSettings::default(),
                );
            }
            with_level(dim, |l| {
                l.steam.clouds.push(Cloud {
                    center: at,
                    radius: cloud,
                    ticks: (fail::CLOUD_SECONDS * 20.0) as u32,
                })
            });
        }
        Event::Burst(pos) => {
            set_property(host, level, pos, "damaged", "true");
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.fire.extinguish",
                1.5,
                0.6,
            );
            puff_particles(host, level, center(pos), 0.3, 30, 0.1);
        }
        Event::CylinderJam(pos) => {
            set_property(host, level, pos, "damaged", "true");
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.anvil.land",
                0.8,
                0.8,
            );
        }
        Event::Puff(pos) => {
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.fire.extinguish",
                1.0,
                1.0,
            );
            puff_particles(host, level, center(pos), 0.4, 25, 0.05);
            with_level(dim, |l| l.steam.puffs.push((pos, 20)));
        }
        Event::ValveOpen(pos) => {
            set_property(host, level, pos, "open", "true");
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:entity.generic.extinguish_fire",
                1.2,
                1.6,
            );
        }
        Event::ValveShut(pos) => {
            set_property(host, level, pos, "open", "false");
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.iron_trapdoor.close",
                0.6,
                1.4,
            );
        }
    }
}

// ---- Effects and scalding ------------------------------------------------------------

/// The outlet of a vent: the face opposite its pipe face.
fn outlet(host: &dyn Host, level: LevelRef, pos: Pos) -> Dir {
    server::property(host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::Down)
        .opposite()
}

/// Steam from open vents, burst parts and running cylinders.
fn vent_effects(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let (vents, exhausts) = with_level(dim, |l| (l.steam.works.vents(), l.steam.works.exhausts()));
    for (pos, v) in vents {
        let is_vent = matches!(
            block_at(&*host, level, pos),
            Some(Block::SteamRelease | Block::EmergencyValve)
        );
        if v.rate > 1.0 {
            let (at, spread) = if is_vent {
                let d = outlet(&*host, level, pos).step();
                let c = center(pos);
                (
                    [
                        c[0] + d[0] as f64 * 0.7,
                        c[1] + d[1] as f64 * 0.7,
                        c[2] + d[2] as f64 * 0.7,
                    ],
                    0.15,
                )
            } else {
                (center(pos), 0.35)
            };
            let count = (v.rate / 10.0).ceil() as i32;
            puff_particles(host, level, at, spread, count, 0.12);
            if random() < 0.25 {
                sound_at(host, level, at, "minecraft:block.fire.extinguish", 0.4, 1.8);
            }
        } else if v.simmer && random() < 0.3 {
            puff_particles(host, level, center(pos), 0.1, 1, 0.02);
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.lava.extinguish",
                0.2,
                2.0,
            );
        }
    }
    for c in exhausts {
        if random() < 0.5 {
            let at = center(c.offset(Dir::Up));
            puff_particles(host, level, [at[0], at[1] - 0.4, at[2]], 0.1, 2, 0.04);
        }
    }
}

/// Hiss, shake and glow for multiblocks near or over their rating (spec 2, 7.1, 7.4).
fn warnings(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let warned: Vec<(Pos, (u8, bool))> = with_level(dim, |l| {
        l.steam
            .works
            .shell_positions()
            .into_iter()
            .filter_map(|q| l.steam.works.part(q).map(|p| (q, p.sent)))
            .filter(|(_, s)| s.0 > 0 || s.1)
            .collect()
    });
    // One sound per multiblock is enough: take every fourth block.
    for (i, (pos, (w, glow))) in warned.into_iter().enumerate() {
        if i % 4 != 0 {
            continue;
        }
        if w > 0 {
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.fire.extinguish",
                0.2 + 0.2 * w as f32,
                1.2 + 0.25 * w as f32,
            );
            if w >= 2 {
                puff_particles(host, level, center(pos), 0.5, 2 * w as i32, 0.05);
            }
        }
        if glow {
            sound_at(
                host,
                level,
                center(pos),
                "minecraft:block.chain.hit",
                0.4,
                0.5,
            );
        }
    }
}

/// Scald damage per 10 ticks (2 per second, spec 2, 8).
const SCALD: f32 = 1.0;
/// Hot pipes burn half as hard.
const HOT_PIPE: f32 = 0.5;

fn hurt_in(host: &mut dyn ServerHost, level: LevelRef, min: [f64; 3], max: [f64; 3], amount: f32) {
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return;
    };
    for e in entities {
        let _ = e.hurt(host, "fras:scald", amount, None, None);
    }
}

fn block_box(pos: Pos, grow: f64) -> ([f64; 3], [f64; 3]) {
    (
        [
            pos.x as f64 - grow,
            pos.y as f64 - grow,
            pos.z as f64 - grow,
        ],
        [
            pos.x as f64 + 1.0 + grow,
            pos.y as f64 + 1.0 + grow,
            pos.z as f64 + 1.0 + grow,
        ],
    )
}

/// Every 10 ticks: vents, bursts, exhausts, puffs, clouds and hot pipes scald.
fn scald(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let (vents, exhausts, puffs, clouds, hot) = with_level(dim, |l| {
        let w = &l.steam.works;
        (
            w.vents(),
            w.exhausts(),
            l.steam.puffs.iter().map(|p| p.0).collect::<Vec<_>>(),
            l.steam.clouds.clone(),
            w.hot_pipes(),
        )
    });
    for (pos, v) in vents {
        if !v.open || v.rate <= 1.0 {
            continue;
        }
        match block_at(&*host, level, pos) {
            Some(Block::SteamRelease | Block::EmergencyValve) => {
                // Two blocks out of the outlet face.
                let d = outlet(&*host, level, pos);
                let a = pos.offset(d);
                let b = pos.offset_by(d, 2);
                let (min_a, max_a) = block_box(a, 0.0);
                let (min_b, max_b) = block_box(b, 0.0);
                let min = [
                    min_a[0].min(min_b[0]),
                    min_a[1].min(min_b[1]),
                    min_a[2].min(min_b[2]),
                ];
                let max = [
                    max_a[0].max(max_b[0]),
                    max_a[1].max(max_b[1]),
                    max_a[2].max(max_b[2]),
                ];
                hurt_in(host, level, min, max, SCALD);
            }
            _ => {
                let (min, max) = block_box(pos, 1.0);
                hurt_in(host, level, min, max, SCALD);
            }
        }
    }
    for c in exhausts {
        let (min, max) = block_box(c.offset(Dir::Up), 0.0);
        hurt_in(host, level, min, max, SCALD);
    }
    for p in puffs {
        let (min, max) = block_box(p, 1.0);
        hurt_in(host, level, min, max, SCALD);
    }
    for c in clouds {
        let r = c.radius;
        let min = [c.center[0] - r, c.center[1] - r, c.center[2] - r];
        let max = [c.center[0] + r, c.center[1] + r, c.center[2] + r];
        hurt_in(host, level, min, max, SCALD);
    }
    // Hot pipes: whatever stands on one or walks into one (sneaking on top is safe).
    for net in hot {
        let set: BTreeSet<Pos> = net.iter().copied().collect();
        let (mut min, mut max) = ([f64::MAX; 3], [f64::MIN; 3]);
        for q in &net {
            let (a, b) = block_box(*q, 0.0);
            for i in 0..3 {
                min[i] = min[i].min(a[i]);
                max[i] = max[i].max(b[i]);
            }
        }
        max[1] += 1.0;
        let Some(entities) = level.entities_in(&*host, min, max) else {
            continue;
        };
        for e in entities {
            let Some(p) = e.position(&*host) else {
                continue;
            };
            let feet = Pos::new(
                p[0].floor() as i32,
                p[1].floor() as i32,
                p[2].floor() as i32,
            );
            let below = Pos::new(feet.x, (p[1] - 0.05).floor() as i32, feet.z);
            let inside = set.contains(&feet);
            let on_top = !inside && set.contains(&below);
            if inside || (on_top && e.is_crouching(&*host) != Some(true)) {
                let _ = e.hurt(host, "fras:scald", HOT_PIPE, None, None);
            }
        }
    }
}

/// Clouds and puffs fade; clouds keep billowing.
fn clouds(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let live = with_level(dim, |l| {
        for c in l.steam.clouds.iter_mut() {
            c.ticks = c.ticks.saturating_sub(1);
        }
        l.steam.clouds.retain(|c| c.ticks > 0);
        for p in l.steam.puffs.iter_mut() {
            p.1 = p.1.saturating_sub(1);
        }
        l.steam.puffs.retain(|p| p.1 > 0);
        l.steam.clouds.clone()
    });
    for c in live {
        if c.ticks % 4 == 0 {
            puff_particles(host, level, c.center, c.radius * 0.6, 20, 0.05);
        }
        if c.ticks.is_multiple_of(20) {
            sound_at(
                host,
                level,
                c.center,
                "minecraft:block.fire.extinguish",
                1.0,
                0.5,
            );
        }
    }
}

/// A firebox's container changed or a fuel burnt: tell the plant its heat.
pub fn set_heat(dim: Dim, pos: Pos, kw: f64) {
    with_level(dim, |l| l.steam.works.set_heat(pos, kw));
}

/// Whether a plant part is paused (a firebox doesn't burn then).
pub fn paused(dim: Dim, pos: Pos) -> bool {
    with_level(dim, |l| {
        l.steam.works.contains(pos) && l.steam.works.paused(pos)
    })
}

/// A block broken: forget it everywhere.
pub fn forget(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.steam.works.remove(pos);
        l.steam.signals.remove(&pos);
    });
}

/// Break a block from the plant's side (unused blocks of a rupture).
pub fn break_part(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    break_block(host, level, pos);
}
