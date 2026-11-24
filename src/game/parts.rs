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
            | Block::LargePulley => {
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
                        Block::LargePulley => {
                            l.large_pulleys.insert(pos);
                        }
                        _ => {}
                    }
                }
            } else if !same_block {
                // Broken or replaced.
                l.mech.remove(pos);
                l.cranks.remove(&pos);
                l.wheel_torque.remove(&pos);
                l.tachometers.remove(&pos);
            }
        });
    }

    fn neighbor_changed(&self, _: &mut dyn LevelTicksHost, event: &mut NeighborChangedEvent) {
        let dim = event.dimension.key().id();
        let pos = pos_of(event.pos);
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
        match self.0 {
            Block::ShaftWood | Block::ShaftIron if item == Some(keys.bearing) => {
                super::tools::swap_bearing(host, event.click)
            }
            Block::Crank => Some(crank(host, event.click)),
            Block::Grinder => Some(super::grinder::open(host, event.click)),
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
            Block::Grinder => Some(super::grinder::open(host, event.click)),
            _ => None,
        }
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
        if matches!(self.0, Block::Tachometer | Block::Grinder) {
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
        Some(Block::Grinder | Block::Crank) => true,
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
            Block::Tachometer => tachometer_tick(host, event),
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
    }

    fn on_chunk_unloaded(&self, host: HostSide, event: &mut OnChunkUnloadedEvent<'_>) {
        let HostSide::Server(host) = host else {
            return;
        };
        let pos = Pos::from_array(event.pos);
        // The chunk may have left its level already: unload the part from
        // every level that holds it where that position is no longer loaded.
        let levels: Vec<server::Dim> = with(|s| {
            s.levels
                .iter()
                .filter(|(_, l)| l.mech.contains(pos) || l.cranks.contains_key(&pos))
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
        if !matches!(self.0, Block::Pulley | Block::LargePulley) {
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
            Block::WaterWheel | Block::LargePulley | Block::Pulley
        ) {
            event.distance = event.distance.max(96.0);
        }
    }

    fn screen(&self, _: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
        (self.0 == Block::Grinder).then(|| super::grinder::screen(event))?
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        if self.0 == Block::Grinder {
            super::grinder::screen_draw(host, event);
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
    } else if old_signal == Some(signal) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
    // A changed signal is written back (Changed), which updates comparators.
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
