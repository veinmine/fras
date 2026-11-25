//! Rust on Horizon (`specs/rust.md`): random ticks counted per part, the
//! stage in the block state, leaks and jams, scraping with the wrench,
//! painting, and the rust settings.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use mod_api::{
    HostSide, InteractionResult, ItemPart, ItemStack, LevelRef, PlayerRef, ServerHost, Text,
    UseOnEvent,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, with_level};
use super::world::set_property;
use super::{effects, keys};
use crate::sim::geom::{Dir, Pos};
use crate::sim::rot::Jam;
use crate::sim::rust::{self, Exposure, Scrape, Stage};

// ---- Settings (spec 5.2) --------------------------------------------------------

static ON: AtomicBool = AtomicBool::new(true);
static SPEED: AtomicU8 = AtomicU8::new(1);

pub const SETTING_ON: &str = "rust";
pub const SETTING_SPEED: &str = "rust_speed";
pub const ON_CHOICES: [&str; 2] = ["fras.settings.rust.on", "fras.settings.rust.off"];
pub const SPEED_CHOICES: [&str; 4] = [
    "fras.settings.rust_speed.half",
    "fras.settings.rust_speed.normal",
    "fras.settings.rust_speed.double",
    "fras.settings.rust_speed.quadruple",
];

pub fn set_settings(on_choice: u8, speed: u8) {
    ON.store(on_choice == 0, Ordering::Relaxed);
    SPEED.store(speed.min(3), Ordering::Relaxed);
}

fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

fn speed() -> usize {
    SPEED.load(Ordering::Relaxed) as usize
}

// ---- Random ticks (spec 5.1, 11) ------------------------------------------------------

/// A rusting part took a random tick: done by the level tick.
pub fn queue(dim: Dim, pos: Pos) {
    with_level(dim, |l| {
        l.steam.rust_ticks.insert(pos);
    });
}

/// The `randomTickSpeed` game rule (3 when it can't be read).
pub fn random_tick_speed(host: &dyn mod_api::Host, level: LevelRef) -> u32 {
    for name in ["minecraft:random_tick_speed", "randomTickSpeed"] {
        if let Ok(Some(mod_api::GameRuleValue::Int(v))) = level.game_rule(host, name) {
            return v.max(0) as u32;
        }
    }
    3
}

/// What exposes a part now (spec 4).
pub fn exposure(host: &dyn mod_api::Host, level: LevelRef, dim: Dim, pos: Pos) -> Exposure {
    let waterlogged = server::property(host, level, pos, "waterlogged").as_deref() == Some("true");
    let touching_water = Dir::ALL.into_iter().any(|d| {
        level
            .fluid_state(host, pos.offset(d).to_array())
            .is_some_and(|f| f.amount > 0 && f.fluid.id(host).ends_with("water"))
    });
    let up = pos.offset(Dir::Up).to_array();
    let rain =
        level.is_raining_at(host, up) == Some(true) && level.can_see_sky(host, up) == Some(true);
    let steam = with_level(dim, |l| l.steam.scalding.contains(&pos));
    Exposure {
        waterlogged,
        touching_water,
        rain,
        steam,
    }
}

/// A part's stage from the level.
pub fn stage(host: &dyn mod_api::Host, level: LevelRef, pos: Pos, block: Block) -> Stage {
    block.rust_stage(&|k| server::property(host, level, pos, k))
}

/// The queued random ticks of one level.
pub fn ticks(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let queued = with_level(dim, |l| std::mem::take(&mut l.steam.rust_ticks));
    for pos in queued {
        let Some(block) = block_at(&*host, level, pos).filter(|b| b.rusts()) else {
            continue;
        };
        let was = stage(&*host, level, pos, block);
        if matches!(was, Stage::Painted | Stage::Leaking) {
            continue;
        }
        let exp = exposure(&*host, level, dim, pos);
        let nbt = host
            .block_entity(level, pos.to_array())
            .and_then(|e| e.data(&*host))
            .unwrap_or_default();
        let count = blocks::get_i32(&nbt, "rust_n").unwrap_or(0).max(0) as u32;
        let (now, count2) = rust::tick(was, count, exp, on(), speed());
        if count2 != count {
            let mut c = mod_api::Compound::new();
            blocks::put_i32(&mut c, "rust_n", count2 as i32);
            server::merge(host, level, pos, c);
        }
        if now != was {
            set_stage(host, level, dim, pos, block, now);
        }
    }
}

/// Set a part's stage: its look, its leak or jam.
pub fn set_stage(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
    stage: Stage,
) {
    set_property(host, level, pos, "rust", stage.name());
    if block.kind().is_some() {
        let jammed = with_level(dim, |l| l.mech.part(pos).and_then(|p| p.jam));
        if stage.leaking() && jammed.is_none() {
            with_level(dim, |l| l.mech.jam_part(pos, Jam::Rusted));
            effects::sound(host, level, pos, "minecraft:block.anvil.land", 0.5, 0.6);
        } else if !stage.leaking() && jammed == Some(Jam::Rusted) {
            with_level(dim, |l| l.mech.repair(pos));
        }
    }
    if block.is_plant() {
        super::steam::queue(dim, pos, false);
    }
}

// ---- Scraping (spec 8) ---------------------------------------------------------

/// The wrench on a rusted part scrapes it: None when there is nothing to
/// scrape (clean, painted or not a rusting part), so the wrench goes on.
pub fn scrape(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
) -> Option<InteractionResult> {
    if !block.rusts() {
        return None;
    }
    let st = stage(&*host, level, pos, block);
    if !st.rusted() {
        return None;
    }
    let k = keys()?;
    let creative = super::tools::creative(&*host, player);
    let has_plate = creative
        || super::tools::carried(&*host, player, k.iron_plate)
            .iter()
            .any(|(_, s)| s.count() > 0);
    match rust::scrape(st, has_plate) {
        Scrape::Nothing => None,
        Scrape::NeedsPlate => {
            super::tools::bar(
                host,
                player,
                Text::translate("fras.rust.needs_plate", vec![]),
            );
            Some(InteractionResult::Fail)
        }
        Scrape::Done { flakes, plate } => {
            if plate && !creative {
                super::tools::take(host, player, k.iron_plate, 1);
            }
            if let Some(stack) = ItemStack::new(k.rust_flakes, flakes) {
                let _ = level.pop_resource(host, pos.to_array(), stack);
            }
            let mut c = mod_api::Compound::new();
            blocks::put_i32(&mut c, "rust_n", 0);
            server::merge(host, level, pos, c);
            set_stage(host, level, dim, pos, block, Stage::Clean);
            effects::sound(host, level, pos, "minecraft:item.axe.scrape", 1.0, 0.8);
            super::tools::bar(host, player, Text::translate("fras.rust.scraped", vec![]));
            Some(InteractionResult::Success)
        }
    }
}

// ---- Paint (spec 7.1) ------------------------------------------------------------

pub struct Paint;

#[mod_api::horizon_part]
impl ItemPart for Paint {
    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        super::tools::Tooltip("red_oxide_paint").tooltip(host, event);
    }

    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return Some(InteractionResult::Success);
        };
        let player = host.player(event.player)?;
        let level = player.level(&*host)?;
        let dim = server::dim_of(&*host, level)?;
        let pos = Pos::from_array(event.pos);
        let block = block_at(&*host, level, pos)?;
        if !block.rusts() {
            super::tools::bar(host, player, Text::translate("fras.rust.not_iron", vec![]));
            return Some(InteractionResult::Fail);
        }
        match rust::paint(stage(&*host, level, pos, block)) {
            Ok(st) => {
                set_stage(host, level, dim, pos, block, st);
                if !super::tools::creative(&*host, player) {
                    let slot = super::tools::hand_slot(&*host, player, event.hand);
                    let _ = player.set_inventory_slot(host, slot, event.stack.clone().shrink(1));
                }
                effects::sound(
                    host,
                    level,
                    pos,
                    "minecraft:item.honeycomb.wax_on",
                    1.0,
                    0.9,
                );
                Some(InteractionResult::Success)
            }
            Err(key) => {
                super::tools::bar(host, player, Text::translate(key, vec![]));
                Some(InteractionResult::Fail)
            }
        }
    }
}

/// The overlay's rust line for a part (spec 9).
pub fn line(
    host: &dyn mod_api::Host,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
) -> Option<Text> {
    if !block.rusts() {
        return None;
    }
    let st = stage(host, level, pos, block);
    let name = Text::translate(format!("fras.rust.stage.{}", st.name()), vec![]);
    Some(match st {
        Stage::Painted => Text::translate("fras.rust.painted", vec![]),
        Stage::Leaking => {
            let jam =
                with_level(dim, |l| l.mech.part(pos).and_then(|p| p.jam)) == Some(Jam::Rusted);
            if jam {
                Text::translate("fras.rust.rusted_through", vec![])
            } else if block == Block::FurnaceDamper {
                Text::translate("fras.rust.stuck", vec![])
            } else if block == Block::IronRadiator {
                Text::translate("fras.rust.no_cooling", vec![])
            } else {
                Text::translate(
                    "fras.rust.leaking",
                    vec![Text::literal(super::num(rust::LEAK_RATE))],
                )
            }
        }
        _ => {
            let exp = exposure(host, level, dim, pos);
            let nbt = host
                .block_entity(level, pos.to_array())
                .and_then(|e| e.data(host))
                .unwrap_or_default();
            let count = blocks::get_i32(&nbt, "rust_n").unwrap_or(0).max(0) as u32;
            match (exp.cause(), on()) {
                (Some(cause), true) => {
                    let days =
                        rust::days_to_next(count, exp, speed(), random_tick_speed(host, level))
                            .map_or("?".to_string(), |d| super::num(d.max(0.1)));
                    Text::translate(
                        "fras.rust.rusting",
                        vec![
                            name,
                            Text::translate(format!("fras.rust.cause.{}", cause.name()), vec![]),
                            Text::translate(
                                format!("fras.rust.stage.{}", st.next().name()),
                                vec![],
                            ),
                            Text::literal(days),
                        ],
                    )
                }
                (Some(_), false) => Text::translate("fras.rust.off", vec![name]),
                _ => Text::translate("fras.rust.not_exposed", vec![name]),
            }
        }
    })
}
