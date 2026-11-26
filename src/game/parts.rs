//! The parts FRAS attaches to its blocks and block entities.
//!
//! Each block has its own block entity type (spike lesson 1). Block hooks
//! that only get a read-only level queue their work in `server`.

use mod_api::{
    AnalogOutputEvent, BlockEntityEvent, BlockEntityOnLoadEvent, BlockEntityPart,
    BlockEntityRenderBoundingBoxEvent, BlockEntitySubmitEvent, BlockEntityTickEvent,
    BlockEntityTickReply, BlockEntityTicksEvent, BlockEntityUpdateTagEvent,
    BlockEntityViewDistanceEvent, BlockPart, BlockState, ClientHost, Compound,
    HasAnalogOutputEvent, Host, HostSide, InteractionResult, LevelHost, LevelRandomHost, LevelSide,
    LevelTicksHost, NeighborChangedEvent, OnBlockStateChangeEvent, OnChunkUnloadedEvent,
    PreRemoveSideEffectsEvent, ScreenDrawEvent, ScreenEvent, ServerHost, StateForPlacementEvent,
    UseItemOnEvent, UseWithoutItemEvent,
};

use super::blocks::{self, Block};
use super::server::{self, Cranking, with, with_level};
use super::{dir, yaw_dir};
use crate::sim::geom::{Axis, Dir, Pos};

fn pos_of(p: mod_api::BlockPos) -> Pos {
    Pos::new(p.x, p.y, p.z)
}

/// A state's properties, read from its serialized form
/// (`fras:wooden_shaft[axis=x,waterlogged=false]`).
pub fn state_props(level: &dyn LevelHost, state: BlockState) -> (String, Vec<(String, String)>) {
    let s = level.state_string(state).unwrap_or_default();
    let (name, rest) = match s.find('[') {
        Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']')),
        None => (s.as_str(), ""),
    };
    let props = rest
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    (name.to_string(), props)
}

fn with_prop(level: &dyn LevelHost, state: BlockState, key: &str, value: &str) -> BlockState {
    level
        .state_with_property(state, key, value)
        .unwrap_or(state)
}

/// The horizontal side with a FRAS shaft-like part whose axis points at
/// `pos` (a machine's socket goes there, grinder spec 5.1).
/// Whether a vertical shaft-like part stands under `pos`.
fn shaft_below(level: &dyn LevelHost, pos: Pos) -> bool {
    level
        .block_state(pos.offset(Dir::Down).to_array())
        .is_some_and(|s| {
            let (name, props) = state_props(level, s);
            name.starts_with("fras:") && props.iter().any(|(k, v)| k == "axis" && v == "y")
        })
}

fn socket_toward_shaft(level: &dyn LevelHost, pos: Pos) -> Option<Dir> {
    Dir::HORIZONTAL.into_iter().find(|d| {
        let Some(s) = level.block_state(pos.offset(*d).to_array()) else {
            return false;
        };
        let (name, props) = state_props(level, s);
        name.starts_with("fras:")
            && props
                .iter()
                .any(|(k, v)| k == "axis" && v == d.axis().name())
    })
}

// ---- Block hooks -------------------------------------------------------------

/// The block side of a part: placement, use, neighbours, changes.
pub struct BlockHooks(pub Block);

#[mod_api::horizon_part]
impl BlockPart for BlockHooks {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let ctx = event.context;
        let face = dir(ctx.face);
        let facing = yaw_dir(ctx.yaw);
        let mut state = event.state;
        let set = |state: BlockState, k: &str, v: &str| with_prop(level, state, k, v);
        match self.0 {
            Block::ShaftWood
            | Block::ShaftIron
            | Block::Bearing
            | Block::Clutch
            | Block::Pulley
            | Block::LargePulley
            | Block::IronPulley
            | Block::LargeIronPulley => {
                // Placed against a face, it lies along that face's axis.
                state = set(state, "axis", face.axis().name());
            }
            Block::BevelGearWood | Block::BevelGearIron => {
                let input = face.opposite();
                let output = if facing.axis() != input.axis() {
                    facing
                } else {
                    input.clockwise()
                };
                state = set(state, "input", input.name());
                state = set(state, "output", output.name());
            }
            Block::Gearbox => {
                let slow = if ctx.sneaking { face.opposite() } else { face };
                state = set(state, "facing", slow.name());
            }
            Block::Crank => state = set(state, "facing", face.opposite().name()),
            Block::Tachometer => state = set(state, "facing", face.name()),
            Block::Washer
            | Block::TripHammer
            | Block::Lathe
            | Block::Sawmill
            | Block::InlineGrinder => {
                // Placed like a door: the drive here, the machine above it,
                // which must be free; the built-in belt runs away from the
                // player; the socket faces an adjacent shaft (machines spec 2.3).
                let pos = pos_of(ctx.pos);
                let free = level
                    .block_state(pos.offset(Dir::Up).to_array())
                    .is_some_and(|s| {
                        let (name, _) = state_props(level, s);
                        matches!(
                            name.as_str(),
                            "minecraft:air" | "minecraft:cave_air" | "minecraft:short_grass"
                        )
                    });
                if !free {
                    return Some(None);
                }
                let socket = socket_toward_shaft(level, pos)
                    .or_else(|| shaft_below(level, pos).then_some(Dir::Down))
                    .unwrap_or(Dir::Down);
                state = set(state, "facing", facing.name());
                state = set(state, "half", "lower");
                state = set(state, "socket", socket.name());
            }
            // ---- 0.4: belts (logistics 3.1, 8.1, 9) ----
            Block::ConveyorBelt => state = set(state, "facing", facing.name()),
            Block::Diverter => {
                state = set(state, "facing", facing.name());
                state = set(state, "flap", "left");
            }
            Block::BucketElevator => state = set(state, "facing", facing.name()),
            Block::Trommel => {
                // Formed with its neighbours once placed (machines spec 8.2).
                state = set(state, "part", "none");
                state = set(state, "socket", "none");
                state = set(state, "axis", facing.axis().name());
            }
            Block::Grinder => {
                // Facing an adjacent shaft-like part whose axis points at it,
                // else facing the player (grinder spec 5.1).
                let pos = pos_of(ctx.pos);
                let socket = Dir::HORIZONTAL
                    .into_iter()
                    .find(|d| {
                        let Some(s) = level.block_state(pos.offset(*d).to_array()) else {
                            return false;
                        };
                        let (name, props) = state_props(level, s);
                        name.starts_with("fras:")
                            && props
                                .iter()
                                .any(|(k, v)| k == "axis" && v == d.axis().name())
                    })
                    .unwrap_or(facing.opposite());
                state = set(state, "facing", socket.name());
            }
            Block::WaterWheel => {
                let axis = if face.axis() != Axis::Y {
                    face.axis()
                } else {
                    facing.axis()
                };
                // Its 8 paddle positions must be air or water (spec 8.2).
                let pos = pos_of(ctx.pos);
                let clear = super::world::ring(pos, axis).into_iter().all(|q| {
                    level.block_state(q.to_array()).is_some_and(|s| {
                        let (name, _) = state_props(level, s);
                        matches!(
                            name.as_str(),
                            "minecraft:air" | "minecraft:cave_air" | "minecraft:water"
                        )
                    })
                });
                if !clear {
                    return Some(None);
                }
                state = set(state, "axis", axis.name());
            }
            // ---- 0.2 ----
            Block::CopperBoiler
            | Block::IronBoiler
            | Block::StorageTank
            | Block::SteamDome
            | Block::CopperPipe
            | Block::IronPipe => {}
            // A furnace's front faces the player.
            Block::Firebox => state = set(state, "facing", facing.opposite().name()),
            Block::ManualValve | Block::RedstoneValve => {
                state = set(state, "axis", face.axis().name());
            }
            // A vent's pipe face is the face it was placed against.
            Block::SteamRelease | Block::EmergencyValve => {
                state = set(state, "facing", face.opposite().name());
            }
            Block::Pump => state = set(state, "facing", face.opposite().name()),
            Block::PressureGauge => state = set(state, "facing", face.name()),
            Block::SightGlass => {
                let f = if face.axis() != Axis::Y {
                    face
                } else {
                    facing.opposite()
                };
                state = set(state, "facing", f.name());
            }
            Block::SteamCylinder => {
                // Beside a flywheel's hub (two blocks off, in its plane), it
                // faces the hub by itself (multiblocks spec 4.2).
                let pos = pos_of(ctx.pos);
                let toward = Dir::HORIZONTAL
                    .into_iter()
                    .find(|d| {
                        let Some(s) = level.block_state(pos.offset_by(*d, 2).to_array()) else {
                            return false;
                        };
                        let (name, props) = state_props(level, s);
                        name == "fras:flywheel"
                            && props
                                .iter()
                                .any(|(k, v)| k == "axis" && v != d.axis().name())
                    })
                    .unwrap_or(facing);
                state = set(state, "facing", toward.name());
            }
            // ---- 0.3 ----
            Block::Governor => {
                // Along the pipe it was placed against; its socket toward an
                // adjacent shaft-like part pointing at it, else on top.
                let axis = face.axis();
                let pos = pos_of(ctx.pos);
                let socket = Dir::ALL
                    .into_iter()
                    .filter(|d| d.axis() != axis)
                    .find(|d| {
                        let Some(s) = level.block_state(pos.offset(*d).to_array()) else {
                            return false;
                        };
                        let (name, props) = state_props(level, s);
                        name.starts_with("fras:")
                            && !name.ends_with("_pipe")
                            && props
                                .iter()
                                .any(|(k, v)| k == "axis" && v == d.axis().name())
                    })
                    .unwrap_or(if axis == Axis::Y {
                        facing.opposite()
                    } else {
                        Dir::Up
                    });
                state = set(state, "axis", axis.name());
                state = set(state, "socket", socket.name());
            }
            // Like a gauge: on the face of the boiler block it was placed against.
            Block::FurnaceDamper | Block::CopperRadiator | Block::IronRadiator => {
                state = set(state, "facing", face.name());
            }
            Block::HeatExchanger => state = set(state, "facing", face.name()),
            // The nozzle (bellows) or pipe face (drain) toward the block it
            // was placed against.
            Block::Bellows | Block::Drain => state = set(state, "facing", face.opposite().name()),
            // ---- Creative ----
            // The socket toward the block it was placed against.
            Block::CreativeEngine | Block::LoadBank => {
                state = set(state, "facing", face.opposite().name())
            }
            Block::CreativeSteam
            | Block::CreativeWater
            | Block::CreativeDrain
            | Block::CreativeFirebox => {}
            Block::Flywheel => {
                let axis = if face.axis() != Axis::Y {
                    face.axis()
                } else {
                    facing.axis()
                };
                // Its plane must be clear (multiblocks spec 4.1).
                let pos = pos_of(ctx.pos);
                let clear = crate::sim::multi::engine::rim(pos, axis)
                    .into_iter()
                    .all(|q| {
                        level.block_state(q.to_array()).is_some_and(|s| {
                            let (name, _) = state_props(level, s);
                            matches!(
                                name.as_str(),
                                "minecraft:air"
                                    | "minecraft:cave_air"
                                    | "minecraft:short_grass"
                                    | "minecraft:snow"
                            )
                        })
                    });
                if !clear {
                    return Some(None);
                }
                state = set(state, "axis", axis.name());
            }
        }
        // A pre-painted item places its part painted (rust spec 7.2): the
        // item's block state component isn't applied to a mod block.
        if self.0.rusts()
            && let (Some(item), Some(k)) = (ctx.item, super::keys())
            && k.painted.contains(&item)
        {
            state = set(state, "rust", "painted");
        }
        Some(Some(state))
    }

    fn on_block_state_change(&self, _: LevelSide, event: &mut OnBlockStateChangeEvent) {
        if event.predicted {
            return;
        }
        let dim = event.dimension.key().id();
        let pos = pos_of(event.pos);
        let same_block = event.old.block() == event.new.block();
        let mine_new = event.state.block() == event.new.block();
        if self.0.is_plant() {
            if mine_new {
                super::steam::queue(dim, pos, !same_block);
            } else if !same_block {
                super::steam::forget(dim, pos);
            }
        }
        if matches!(self.0, Block::PressureGauge | Block::SightGlass) && !mine_new && !same_block {
            super::steam::forget(dim, pos);
        }
        // 0.4: belt lines trace again; the trommel's halves follow each other.
        if self.0.is_belt() || self.0 == Block::BucketElevator {
            if mine_new {
                super::belts::queue(dim, pos);
            } else if !same_block {
                super::belts::broke(dim, pos);
            }
        }
        if self.0 == Block::Trommel && !same_block {
            super::machines::trommel_changed(dim, pos);
        }
        if self.0.is_line_machine() && !same_block {
            super::machines::line_changed(dim, pos, mine_new);
        }
        if (self.0.is_line_machine() || self.0 == Block::Trommel) && mine_new && !same_block {
            super::machines::neighbour(dim, pos);
        }
        with_level(dim, |l| {
            if mine_new {
                // Placed, or one of its properties changed.
                l.pending.insert(pos, true);
                if !same_block {
                    match self.0 {
                        Block::Bearing => {
                            l.bearings.insert(pos);
                        }
                        Block::Clutch => {
                            l.clutches.insert(pos);
                        }
                        Block::Gearbox => {
                            l.gearboxes.insert(pos, true);
                        }
                        Block::LargePulley | Block::LargeIronPulley => {
                            l.large_pulleys.insert(pos);
                        }
                        _ => {}
                    }
                }
            } else if !same_block {
                // Broken or replaced.
                l.mech.remove(pos);
                l.steam.rust_counts.remove(&pos);
                l.cranks.remove(&pos);
                l.wheel_torque.remove(&pos);
                l.tachometers.remove(&pos);
                l.histories.remove(&pos);
                l.machines.signals.remove(&pos);
                l.grinder_signal.remove(&pos);
            }
        });
    }

    fn neighbor_changed(&self, _: &mut dyn LevelTicksHost, event: &mut NeighborChangedEvent) {
        let dim = event.dimension.key().id();
        let pos = pos_of(event.pos);
        if self.0.is_belt() || self.0 == Block::BucketElevator {
            super::belts::neighbour(dim, pos, self.0 == Block::Diverter);
        }
        if self.0.is_line_machine() || self.0 == Block::Trommel {
            super::machines::neighbour(dim, pos);
        }
        with_level(dim, |l| match self.0 {
            Block::Bearing => {
                l.bearings.insert(pos);
            }
            Block::Clutch => {
                l.clutches.insert(pos);
            }
            Block::Gearbox => {
                l.gearboxes.entry(pos).or_insert(false);
            }
            Block::WaterWheel => {
                l.wheels.insert(pos);
            }
            Block::RedstoneValve | Block::FurnaceDamper => {
                l.steam.redstone.insert(pos);
            }
            Block::HeatExchanger => {
                l.steam.pending.entry(pos).or_insert(false);
            }
            Block::Pump => {
                l.steam.pumps.insert(pos);
            }
            Block::Flywheel => {
                l.steam.flywheels.insert(pos);
            }
            _ => {}
        });
    }

    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let keys = super::keys()?;
        let item = event.click.stack.map(|s| s.item());
        // The wrench is answered here, so a machine never opens its menu
        // for it; the belt is the item's to answer.
        if item == Some(keys.wrench) {
            let click = event.click;
            super::tools::wrench(host, click.player, click.pos, click.hit.face);
            return Some(InteractionResult::Success);
        }
        if item == Some(keys.leather_belt) {
            return Some(InteractionResult::Pass);
        }
        // Paint is the item's to answer (rust spec 7.1).
        if item == Some(keys.paint) {
            return None;
        }
        match self.0 {
            Block::ShaftWood | Block::ShaftIron if item == Some(keys.bearing) => {
                super::tools::swap_bearing(host, event.click)
            }
            Block::Crank => Some(crank(host, event.click)),
            b if b.is_belt() || b == Block::BucketElevator => {
                super::belts::use_on(host, event.click)
            }
            Block::Washer if item == Some(keys.water_bucket) => {
                super::machines::bucket(host, event.click)
            }
            b if b.is_machine04() => Some(super::machines::use_on(host, event.click)),
            b if b.has_screen() => Some(super::grinder::open(host, event.click)),
            Block::CopperBoiler | Block::IronBoiler | Block::StorageTank => {
                super::pipes::bucket(host, event.click)
            }
            // Horizon asks here, once per hand with no stack, rather than in
            // `use_without_item`: the empty main hand works the valve.
            Block::ManualValve | Block::SteamRelease
                if item.is_none() && event.click.hand == mod_api::Hand::Main =>
            {
                hand_on_vent(self.0, host, event.click)
            }
            _ => None,
        }
    }

    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        match self.0 {
            Block::Crank => Some(crank(host, event.click)),
            b if b.is_belt() || b == Block::BucketElevator => {
                super::belts::use_on(host, event.click)
            }
            b if b.is_machine04() => Some(super::machines::use_on(host, event.click)),
            b if b.has_screen() => Some(super::grinder::open(host, event.click)),
            Block::ManualValve | Block::SteamRelease => hand_on_vent(self.0, host, event.click),
            _ => None,
        }
    }

    /// Rusting parts count their exposure on random ticks (rust spec 5.1):
    /// queued here, done by the level tick.
    fn random_tick(
        &self,
        _: &mut dyn mod_api::LevelWriteHost,
        event: &mut mod_api::RandomTickEvent,
    ) -> mod_api::Flow {
        if self.0.rusts() {
            let dim = event.dimension.key().id();
            super::rust::queue(dim, pos_of(event.pos));
        }
        mod_api::Flow::Continue
    }

    /// The grinder's slots changed (a player, a hopper, the grinder itself):
    /// read its comparator signal again before comparators ask.
    fn contents_changed(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::ContentsChangedEvent,
    ) {
        if self.0 == Block::Grinder {
            super::grinder::refresh_signal(&*host, event.level, pos_of(event.pos));
        }
    }

    fn has_analog_output(&self, _: &dyn LevelHost, event: &mut HasAnalogOutputEvent) {
        if matches!(
            self.0,
            Block::Tachometer
                | Block::Grinder
                | Block::PressureGauge
                | Block::SightGlass
                | Block::Diverter
        ) || self.0.is_machine04()
        {
            event.output = true;
        }
    }

    fn analog_output(&self, _: &dyn LevelHost, event: &mut AnalogOutputEvent) {
        let dim = event.dimension.key().id();
        let pos = pos_of(event.pos);
        match self.0 {
            Block::Tachometer => {
                event.output = with_level(dim, |l| l.tachometers.get(&pos).copied().unwrap_or(0));
            }
            Block::Grinder => {
                event.output =
                    with_level(dim, |l| l.grinder_signal.get(&pos).copied().unwrap_or(0));
            }
            b if b.is_machine04() => {
                event.output =
                    with_level(dim, |l| l.machines.signals.get(&pos).copied().unwrap_or(0));
            }
            Block::Diverter => event.output = super::belts::diverter_signal(dim, pos),
            Block::PressureGauge | Block::SightGlass => {
                event.output = with_level(dim, |l| l.steam.signals.get(&pos).copied().unwrap_or(0));
            }
            _ => {}
        }
    }
}

/// Whether a part takes the item clicked on it itself (the server answers),
/// so the client mustn't predict placing it: asked on the client.
pub fn takes_item(host: &dyn mod_api::Host, click: &mod_api::BlockClick<'_>) -> bool {
    let (Some(keys), Some(stack)) = (super::keys(), click.stack) else {
        return false;
    };
    let item = stack.item();
    if item == keys.leather_belt {
        return false;
    }
    let level = host
        .level(click.dimension.key().id())
        .or_else(|| host.player(click.player).and_then(|p| p.level(host)));
    let Some(level) = level else {
        return false;
    };
    match super::server::block_at(host, level, pos_of_array(click.pos)) {
        _ if item == keys.paint => false,
        Some(Block::Crank) => true,
        Some(b) if b.is_belt() || b == Block::BucketElevator => true,
        Some(b) if b.has_screen() => true,
        Some(Block::CopperBoiler | Block::IronBoiler | Block::StorageTank) => {
            item == keys.wrench || item == keys.water_bucket || item == keys.bucket
        }
        Some(Block::ShaftWood | Block::ShaftIron) => item == keys.bearing,
        Some(_) => item == keys.wrench,
        None => false,
    }
}

fn pos_of_array(p: [i32; 3]) -> Pos {
    Pos::new(p[0], p[1], p[2])
}

/// Hold the use button on a crank to turn it; sneak to turn it the other
/// way. Vanilla repeats a held use on a block every 4 ticks, so each use
/// pushes a little longer than that (spec 8.1).
fn crank(host: &mut dyn ServerHost, click: &mod_api::BlockClick<'_>) -> InteractionResult {
    let Some(player) = host.player(click.player) else {
        return InteractionResult::Pass;
    };
    let dim = click.dimension.key().id();
    let Some(level) = host.level(dim) else {
        return InteractionResult::Pass;
    };
    let pos = Pos::from_array(click.pos);
    let reverse = player.is_secondary_use_active(&*host) == Some(true);
    let fresh = with_level(dim, |l| {
        let fresh = !l.cranks.contains_key(&pos);
        l.cranks.insert(
            pos,
            Cranking {
                ticks: CRANK_PUSH_TICKS,
                dir: if reverse { -1.0 } else { 1.0 },
            },
        );
        fresh
    });
    // About sprinting's hunger for the time spent cranking.
    player.add_exhaustion(host, 0.1);
    if fresh || with(|s| s.ticks % 8 == 0) {
        super::effects::sound(
            host,
            level,
            pos,
            "minecraft:block.wooden_trapdoor.close",
            0.3,
            1.8,
        );
    }
    InteractionResult::Success
}

/// Ticks of push from one use of the crank.
pub const CRANK_PUSH_TICKS: u32 = 6;

// ---- Block entity hooks -----------------------------------------------------

/// The block entity side of a part.
pub struct EntityHooks(pub Block);

#[mod_api::horizon_part]
impl BlockPart for EntityHooks {}

#[mod_api::horizon_part]
impl BlockEntityPart for EntityHooks {
    /// A new part starts with a version mark, so a reload tells saved data
    /// from none.
    fn block_entity(&self, _: LevelSide, _: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let mut c = Compound::new();
        blocks::put_i32(&mut c, "v", 1);
        Some(c)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = self.0.ticks();
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        match self.0 {
            Block::Grinder => super::grinder::tick(host, event),
            b if b.is_machine04() => super::machines::tick(b, host, event),
            Block::Tachometer => tachometer_tick(host, event),
            Block::Firebox => super::firebox::tick(host, event),
            Block::Governor => super::governor::tick(host, event),
            Block::PressureGauge | Block::SightGlass => gauge_tick(self.0, host, event),
            b if b.is_creative() => super::creative::tick(b, host, event),
            b if super::setpoint::has(b) => super::setpoint::tick(b, host, event),
            _ => *event.reply = BlockEntityTickReply::Unchanged,
        }
    }

    fn block_entity_on_load(&self, host: HostSide, event: &mut BlockEntityOnLoadEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(pos) = event.entity.position(&*host) else {
            return;
        };
        let Some(level) = event.entity.level(&*host) else {
            return;
        };
        let Some(dim) = server::dim_of(&*host, level) else {
            return;
        };
        if self.0.kind().is_some() {
            with_level(dim, |l| {
                l.pending.entry(Pos::from_array(pos)).or_insert(false);
            });
        }
        if self.0.is_plant() {
            super::steam::queue(dim, Pos::from_array(pos), false);
        }
        if self.0.is_belt() || self.0 == Block::BucketElevator {
            let nbt = event.entity.data(&*host).unwrap_or_default();
            let saved = super::belts::saved_items(&*host, &nbt);
            super::belts::loaded(dim, Pos::from_array(pos), saved);
        }
    }

    fn on_chunk_unloaded(&self, host: HostSide, event: &mut OnChunkUnloadedEvent<'_>) {
        let HostSide::Server(host) = host else {
            return;
        };
        let pos = Pos::from_array(event.pos);
        if self.0.is_belt() || self.0 == Block::BucketElevator {
            let levels: Vec<server::Dim> = with(|s| {
                s.levels
                    .iter()
                    .filter(|(_, l)| l.belts.by_block.contains_key(&pos))
                    .map(|(d, _)| *d)
                    .collect()
            });
            for dim in levels {
                super::belts::unloaded(dim, pos);
            }
        }
        // The chunk may have left its level already: unload the part from
        // every level that holds it where that position is no longer loaded.
        let levels: Vec<server::Dim> = with(|s| {
            s.levels
                .iter()
                .filter(|(_, l)| {
                    l.mech.contains(pos)
                        || l.cranks.contains_key(&pos)
                        || l.steam.works.contains(pos)
                })
                .map(|(d, _)| *d)
                .collect()
        });
        for dim in levels {
            let gone = host
                .level(dim)
                .is_none_or(|level| !level.is_loaded(&*host, pos.to_array()));
            if gone || levels_len() == 1 {
                with_level(dim, |l| {
                    l.mech.unload(pos);
                    l.cranks.remove(&pos);
                    l.wheel_torque.remove(&pos);
                    l.steam.works.unload(pos);
                    l.steam.signals.remove(&pos);
                });
            }
        }
    }

    /// A pulley broken gives back its belt and frees its partner.
    fn pre_remove_side_effects(
        &self,
        _: &mut dyn LevelRandomHost,
        event: &mut PreRemoveSideEffectsEvent<'_>,
    ) {
        if !self.0.is_pulley() {
            return;
        }
        let nbt = &event.entity.nbt;
        let Some(belt) = blocks::belt(nbt) else {
            return;
        };
        let cost = blocks::get_i32(nbt, "belt_cost").unwrap_or(0).max(0) as u32;
        if cost > 0
            && let Some(keys) = super::keys()
            && let Some(stack) = mod_api::ItemStack::new(keys.leather_belt, cost)
        {
            event.drops.push(stack);
        }
        with(|s| s.orphan_belts.push((belt.partner, pos_of(event.pos))));
    }

    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        let mut tag = Compound::new();
        for key in blocks::CLIENT_KEYS {
            if let Some(v) = event.entity.nbt.get(&(*key).into()) {
                tag.insert((*key).into(), v.clone());
            }
        }
        Some(tag)
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        super::draw::submit(self.0, host, event);
    }

    fn block_entity_render_bounding_box(
        &self,
        _: &dyn ClientHost,
        event: &mut BlockEntityRenderBoundingBoxEvent<'_>,
    ) {
        super::draw::bounds(self.0, event);
    }

    fn block_entity_view_distance(
        &self,
        _: &dyn ClientHost,
        event: &mut BlockEntityViewDistanceEvent<'_>,
    ) {
        if matches!(
            self.0,
            Block::WaterWheel
                | Block::LargePulley
                | Block::Pulley
                | Block::IronPulley
                | Block::LargeIronPulley
                | Block::Flywheel
                | Block::SteamCylinder
                | Block::Governor
        ) {
            event.distance = event.distance.max(96.0);
        }
    }

    fn screen(&self, _: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
        match self.0 {
            Block::Grinder => super::grinder::screen(event),
            b if b.is_machine04() => super::machines::screen(b, event),
            Block::Firebox => super::firebox::screen(event),
            Block::Governor => super::governor::screen(event),
            b if b.is_creative() => super::creative::screen(event),
            b if super::readout::Readout::of(b).is_some() => super::readout::screen(event),
            b if super::setpoint::has(b) => super::setpoint::screen(b, event),
            _ => None,
        }
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        match self.0 {
            Block::Grinder => super::grinder::screen_draw(host, event),
            b if b.is_machine04() => super::machines::screen_draw(b, host, event),
            Block::Firebox => super::firebox::screen_draw(host, event),
            Block::Governor => super::governor::screen_draw(host, event),
            b if b.is_creative() => super::creative::screen_draw(b, host, event),
            b if super::setpoint::has(b) => super::setpoint::screen_draw(b, host, event),
            b => super::readout::screen_draw(b, host, event),
        }
    }

    fn screen_click(
        &self,
        host: &dyn ClientHost,
        event: &mut mod_api::ScreenClickEvent<'_>,
    ) -> Option<i32> {
        match self.0 {
            Block::Governor => super::governor::screen_click(host, event),
            b if b.is_creative() => super::creative::screen_click(b, host, event),
            b if super::readout::Readout::of(b).is_some() => super::readout::screen_click(event),
            b if super::setpoint::has(b) => super::setpoint::screen_click(b, event),
            _ => None,
        }
    }

    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut mod_api::MenuButtonEvent<'_>) {
        if self.0 == Block::Governor {
            super::governor::menu_button(host, event);
        } else if self.0.is_creative() {
            super::creative::menu_button(self.0, host, event);
        } else if super::readout::Readout::of(self.0).is_some() {
            super::readout::menu_button(event);
        } else if super::setpoint::has(self.0) {
            super::setpoint::menu_button(self.0, host, event);
        }
    }
}

fn levels_len() -> usize {
    with(|s| s.levels.len())
}

// ---- Tachometer -------------------------------------------------------------

/// The tachometer reads the part it is attached to: needle, red line,
/// yellow (the working machines' minimum) and a comparator signal 0 to 15,
/// linear up to the red line (spec 11.1).
fn tachometer_tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let pos = Pos::from_array(event.pos);
    let Some(level) = host.level(dim) else {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    };
    let facing = server::property(&*host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(Dir::Up);
    let part = pos.offset(facing.opposite());
    let reading = with_level(dim, |l| l.mech.reading(part));
    let (rpm, red, yellow) = reading.as_ref().map_or((0.0, 0.0, 0.0), |r| {
        (
            r.rpm.abs(),
            r.red_line.unwrap_or(0.0).abs(),
            r.min_working.unwrap_or(0.0).abs(),
        )
    });
    let signal = comparator(rpm, red);
    let old_signal = with_level(dim, |l| l.tachometers.insert(pos, signal));
    let data_changed = super::readout::update(
        dim,
        pos,
        super::readout::Readout::Speed,
        rpm,
        red,
        yellow,
        super::readout::range(&event.entity.nbt),
        &mut event.entity.data,
    );
    let nbt = &event.entity.nbt;
    let shown = blocks::get_f64(nbt, "needle").unwrap_or(-1.0);
    let changed = (shown - rpm).abs() > (0.5f64).max(rpm * 0.03)
        || blocks::get_f64(nbt, "red").unwrap_or(-1.0) != red
        || blocks::get_f64(nbt, "yellow").unwrap_or(-1.0) != yellow;
    if changed {
        let nbt = &mut event.entity.nbt;
        blocks::put_f64(nbt, "needle", rpm);
        blocks::put_f64(nbt, "red", red);
        blocks::put_f64(nbt, "yellow", yellow);
        host.block_entity_sync(event.pos);
    } else if old_signal == Some(signal) && !data_changed {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
    // A changed signal is written back (Changed), which updates comparators.
}

/// A pressure gauge or sight glass reads the part it is on (spec 2, 10):
/// sent to clients when the reading moves by 0.1 bar or 3% (or 2% of the
/// glass), the comparator kept up to date.
fn gauge_tick(block: Block, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let pos = Pos::from_array(event.pos);
    let Some(level) = host.level(dim) else {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    };
    let default = if block == Block::PressureGauge {
        Dir::Up
    } else {
        Dir::North
    };
    let facing = server::property(&*host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(default);
    let part = pos.offset(facing.opposite());
    let nbt = &event.entity.nbt;
    let (signal, changed, data_changed) = if block == Block::PressureGauge {
        let g = with_level(dim, |l| l.steam.works.gauge(part));
        let shown = blocks::get_f64(nbt, "bar").unwrap_or(-1.0);
        let changed = (shown - g.bar).abs() > (0.1f64).max(g.bar * 0.03)
            || blocks::get_f64(nbt, "red").unwrap_or(-1.0) != g.red;
        if changed {
            let nbt = &mut event.entity.nbt;
            blocks::put_f64(nbt, "bar", g.bar);
            blocks::put_f64(nbt, "red", g.red);
        }
        let kind = super::readout::Readout::Pressure;
        let range = super::readout::range(&event.entity.nbt);
        let data = super::readout::update(
            dim,
            pos,
            kind,
            g.bar,
            g.red,
            0.0,
            range,
            &mut event.entity.data,
        );
        (g.signal() as i32, changed, data)
    } else {
        let (level_, warn) = with_level(dim, |l| l.steam.works.sight(part)).unwrap_or((0.0, 0));
        let shown = blocks::get_f64(nbt, "level").unwrap_or(-1.0);
        let changed = (shown - level_).abs() > 0.02
            || blocks::get_i32(nbt, "lwarn").unwrap_or(-1) != warn as i32;
        if changed {
            let nbt = &mut event.entity.nbt;
            blocks::put_f64(nbt, "level", level_);
            blocks::put_i32(nbt, "lwarn", warn as i32);
        }
        let signal = if level_ <= 0.0 {
            0
        } else {
            ((level_ * 15.0).ceil() as i32).clamp(1, 15)
        };
        let kind = super::readout::Readout::Level;
        let data = super::readout::update(
            dim,
            pos,
            kind,
            level_,
            0.0,
            warn as f64,
            super::readout::range(&event.entity.nbt),
            &mut event.entity.data,
        );
        (signal, changed, data)
    };
    let old = with_level(dim, |l| l.steam.signals.insert(pos, signal));
    if changed {
        host.block_entity_sync(event.pos);
    } else if old == Some(signal) && !data_changed {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// The comparator signal for a speed against the red line.
pub fn comparator(rpm: f64, red: f64) -> i32 {
    let red = if red > 0.0 {
        red
    } else {
        crate::sim::rot::part::HARD_CAP_RPM
    };
    ((rpm.abs() / red) * 15.0).floor().clamp(0.0, 15.0) as i32
}

/// An empty hand on a manual valve (one step open, sneaking one step closed)
/// or a steam release (open or close it).
fn hand_on_vent(
    block: Block,
    host: &mut dyn ServerHost,
    click: &mod_api::BlockClick<'_>,
) -> Option<InteractionResult> {
    match block {
        Block::ManualValve => {
            let close = host
                .player(click.player)
                .and_then(|p| p.is_secondary_use_active(&*host))
                == Some(true);
            Some(super::pipes::manual_valve(host, click, close))
        }
        Block::SteamRelease => Some(super::pipes::steam_release(host, click)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparator_is_linear_to_the_red_line() {
        assert_eq!(comparator(0.0, 64.0), 0);
        assert_eq!(comparator(32.0, 64.0), 7);
        assert_eq!(comparator(64.0, 64.0), 15);
        assert_eq!(comparator(-100.0, 64.0), 15);
        assert_eq!(comparator(128.0, 0.0), 7);
    }
}
