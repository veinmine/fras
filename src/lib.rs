//! Fire, Rust and Steam (mod id `fras`): shaft power for Horizon.
//!
//! `sim` holds the rules, free of the game and unit-tested; `game` is the
//! thin layer that puts them on Horizon through hooks and parts (no mixins).

pub mod game;
pub mod sim;

use mod_api::{
    self, EntityPlaceEvent, Flow, Hooks, HostSide, InteractionResult, LevelTickPostEvent,
    Registrar, RegistrationContext, ServerAboutToStartEvent, ServerHost, ServerStoppedEvent,
    TagsUpdatedEvent, UseItemOnBlockEvent, UsePhase,
};

use game::blocks::Block;
use game::parts::{BlockHooks, EntityHooks};

pub const ID: &str = "fras";

/// FRAS's items that are not blocks.
pub const ITEMS: [&str; 11] = [
    "wrench",
    "leather_belt",
    "hand_hammer",
    "crushed_iron",
    "crushed_copper",
    "crushed_gold",
    "iron_plate",
    "copper_plate",
    "rust_flakes",
    "red_oxide_pigment",
    "red_oxide_paint",
];

/// The rusting blocks, each with a pre-painted item `painted_<name>` that
/// places it painted (rust spec 7.2).
pub const PAINTED: [&str; 19] = [
    "iron_shaft",
    "bearing",
    "clutch",
    "iron_bevel_gear",
    "gearbox",
    "iron_pipe",
    "manual_valve",
    "redstone_valve",
    "steam_release",
    "emergency_valve",
    "governor",
    "iron_boiler",
    "storage_tank",
    "steam_cylinder",
    "flywheel",
    "pump",
    "heat_exchanger",
    "iron_radiator",
    "furnace_damper",
];

#[derive(Default)]
struct Fras;

#[mod_api::horizon_mod]
impl Hooks for Fras {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for block in Block::ALL {
            let name = block.name();
            let kind = r.block_entity_type(name).blocks(&[name]).build();
            if let Err(e) = r.block_entity_part(&kind, EntityHooks(block)) {
                eprintln!("[{ID}] block entity part {name}: {e:?}");
            }
            let key = r.block_key(&block.id());
            r.block_part(key, BlockHooks(block));
            if block == Block::Grinder {
                r.block_container(&key, game::grinder::container());
            }
            if block.is_machine04() {
                r.block_container(&key, game::machines::container(block));
            }
            if block.is_belt() || block == Block::BucketElevator {
                r.block_container(&key, game::belts::inbox());
            }
            if block == Block::Firebox {
                r.block_container(&key, game::firebox::container());
            }
            if block == Block::Governor {
                r.block_container(&key, game::governor::container());
            }
            if block.is_creative() {
                r.block_container(&key, game::creative::container(block));
            }
            if game::readout::Readout::of(block).is_some() {
                r.block_container(&key, game::readout::container(block));
            }
            if game::setpoint::has(block) {
                r.block_container(&key, game::setpoint::container(block));
            }
        }
        // The recipe files' `type` is the serializer; it publishes them, JSON
        // intact, under the recipe type of the same id.
        r.menu_part("fras:grinder", game::grinder::InputSlot);
        r.menu_part("fras:firebox", game::firebox::FuelSlot);
        r.register_recipe_type("fras:grinding");
        r.register_raw_recipe_serializer("fras:grinding", "fras:grinding");
        for m in crate::sim::machine::Machine::ALL {
            if m == crate::sim::machine::Machine::Grinder {
                continue;
            }
            if let Some(kind) = m.recipe_type() {
                r.register_recipe_type(kind);
                r.register_raw_recipe_serializer(kind, kind);
            }
            r.menu_part(&format!("fras:{}", m.name()), game::machines::InputSlot(m));
        }
        let wrench = r.item_key("fras:wrench");
        let leather_belt = r.item_key("fras:leather_belt");
        r.item_part(wrench, game::tools::Wrench);
        r.item_part(leather_belt, game::tools::LeatherBelt);
        let paint = r.item_key("fras:red_oxide_paint");
        r.item_part(paint, game::rust::Paint);
        for item in game::blocks::Block::ALL
            .map(Block::name)
            .into_iter()
            .chain(ITEMS)
            // An item has one part: these draw their tooltip themselves.
            .filter(|item| !matches!(*item, "wrench" | "leather_belt" | "red_oxide_paint"))
        {
            let key = r.item_key(&format!("{ID}:{item}"));
            r.item_part(key, game::tools::Tooltip(item));
        }
        let mut painted = Vec::new();
        for item in PAINTED {
            let key = r.item_key(&format!("{ID}:painted_{item}"));
            r.item_part(key, game::tools::Tooltip("painted"));
            painted.push(key);
        }
        let setting = mod_api::SettingSpec::choice(
            game::steam::SETTING,
            "fras.settings.explosion",
            game::steam::MODES,
            0,
        )
        .tooltip("fras.settings.explosion.tooltip");
        if let Err(e) = r.register_setting(setting) {
            eprintln!("[{ID}] setting: {e}");
        }
        let rust_on = mod_api::SettingSpec::choice(
            game::rust::SETTING_ON,
            "fras.settings.rust",
            game::rust::ON_CHOICES,
            0,
        )
        .tooltip("fras.settings.rust.tooltip");
        let rust_speed = mod_api::SettingSpec::choice(
            game::rust::SETTING_SPEED,
            "fras.settings.rust_speed",
            game::rust::SPEED_CHOICES,
            1,
        )
        .tooltip("fras.settings.rust_speed.tooltip");
        for s in [rust_on, rust_speed] {
            if let Err(e) = r.register_setting(s) {
                eprintln!("[{ID}] setting: {e}");
            }
        }
        let _ = game::KEYS.set(game::Keys {
            wrench,
            leather_belt,
            bearing: r.item_key("fras:bearing"),
            wooden_shaft: r.item_key("fras:wooden_shaft"),
            iron_shaft: r.item_key("fras:iron_shaft"),
            bearing_block: r.block_key("fras:bearing"),
            stick: r.item_key("minecraft:stick"),
            iron_nugget: r.item_key("minecraft:iron_nugget"),
            flint: r.item_key("minecraft:flint"),
            water_bucket: r.item_key("minecraft:water_bucket"),
            bucket: r.item_key("minecraft:bucket"),
            copper_ingot: r.item_key("minecraft:copper_ingot"),
            copper_nugget: r.item_key("minecraft:copper_nugget"),
            iron_plate: r.item_key("fras:iron_plate"),
            leather: r.item_key("minecraft:leather"),
            paint,
            painted,
            rust_flakes: r.item_key("fras:rust_flakes"),
            smoke: r.particle_key("minecraft:smoke"),
            crit: r.particle_key("minecraft:crit"),
            cloud: r.particle_key("minecraft:poof"),
            ember: r.particle_key("minecraft:lava"),
            wisp: r.particle_key("minecraft:white_smoke"),
            drip: r.particle_key("minecraft:dripping_water"),
        });
    }

    fn server_about_to_start(&self, _: &mut dyn ServerHost, _: &mut ServerAboutToStartEvent) {
        game::server::clear();
        game::grinder::clear();
        game::machines::clear();
        game::firebox::clear();
    }

    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut ServerStoppedEvent) {
        game::server::clear();
        game::grinder::clear();
        game::machines::clear();
        game::firebox::clear();
    }

    /// The boiler and tank shells' model loader (multiblocks spec 6.1).
    fn register_model_loaders(
        &self,
        _: &RegistrationContext,
        event: &mut mod_api::RegisterModelLoadersEvent<'_>,
    ) {
        event.loaders.push(game::shell_model::LOADER.to_string());
    }

    fn bake_model_loader(
        &self,
        _: &RegistrationContext,
        event: &mut mod_api::BakeModelLoaderEvent<'_>,
    ) {
        if event.loader != game::shell_model::LOADER {
            return;
        }
        if let Some((mask, axis)) = game::shell_model::config(event.config) {
            event
                .quads
                .extend(game::shell_model::quads(mask, axis, "side", "end"));
        }
    }

    /// The explosion mode is a client setting: copy it where the
    /// integrated server reads it.
    fn client_tick_post(
        &self,
        host: &mut dyn mod_api::ClientHost,
        _: &mut mod_api::ClientTickPostEvent,
    ) {
        if let Some(mode) = host.setting_choice(game::steam::SETTING) {
            game::steam::set_explosion_mode(mode as u8);
        }
        let on = host.setting_choice(game::rust::SETTING_ON).unwrap_or(0);
        let speed = host.setting_choice(game::rust::SETTING_SPEED).unwrap_or(1);
        game::rust::set_settings(on as u8, speed as u8);
    }

    /// Recipes and tags are final: read the grinding recipes.
    fn tags_updated(&self, host: HostSide<'_>, _: &mut TagsUpdatedEvent) {
        if let HostSide::Server(host) = host {
            game::grinder::load(&*host);
            game::machines::load(&*host);
            game::firebox::load(&*host);
        }
    }

    /// On the client, a click with an item on a part that takes it (the
    /// grinder's menu, the crank, a bearing onto a shaft) is the server's to
    /// answer: don't predict placing the held block meanwhile.
    fn use_item_on_block(&self, host: HostSide, event: &mut UseItemOnBlockEvent<'_>) -> Flow {
        let HostSide::Client(host) = host else {
            return Flow::Continue;
        };
        if !matches!(event.phase, UsePhase::Block) || !game::parts::takes_item(&*host, event.click)
        {
            return Flow::Continue;
        }
        *event.cancellation_result = InteractionResult::Success;
        Flow::Cancel
    }

    /// Nothing but liquid goes into a water wheel's paddles.
    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        let Some(level) = event.placer.and_then(|e| e.level(&*host)) else {
            return Flow::Continue;
        };
        if game::world::in_wheel(&*host, level, event.pos, event.state) {
            event.cancelled = true;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// After each level's tick: run its networks.
    /// Belt items are written to their blocks when the level saves
    /// (logistics 10.2).
    fn level_save(&self, host: &mut dyn ServerHost, event: &mut mod_api::LevelSaveEvent<'_>) {
        if let Some(level) = host.level(event.dimension)
            && let Some(dim) = game::server::dim_of(&*host, level)
        {
            game::belts::save(host, level, dim);
        }
    }

    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            game::server::level_tick(host, event.level);
        }
    }
}
