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
pub const ITEMS: [&str; 8] = [
    "wrench",
    "leather_belt",
    "hand_hammer",
    "crushed_iron",
    "crushed_copper",
    "crushed_gold",
    "iron_plate",
    "copper_plate",
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
            if block == Block::Firebox {
                r.block_container(&key, game::firebox::container());
            }
        }
        // The recipe files' `type` is the serializer; it publishes them, JSON
        // intact, under the recipe type of the same id.
        r.menu_part("fras:grinder", game::grinder::InputSlot);
        r.menu_part("fras:firebox", game::firebox::FuelSlot);
        r.register_recipe_type("fras:grinding");
        r.register_raw_recipe_serializer("fras:grinding", "fras:grinding");
        let wrench = r.item_key("fras:wrench");
        let leather_belt = r.item_key("fras:leather_belt");
        r.item_part(wrench, game::tools::Wrench);
        r.item_part(leather_belt, game::tools::LeatherBelt);
        for item in game::blocks::Block::ALL
            .map(Block::name)
            .into_iter()
            .chain(ITEMS)
            // An item has one part: these two draw their tooltip themselves.
            .filter(|item| !matches!(*item, "wrench" | "leather_belt"))
        {
            let key = r.item_key(&format!("{ID}:{item}"));
            r.item_part(key, game::tools::Tooltip(item));
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
            smoke: r.particle_key("minecraft:smoke"),
            crit: r.particle_key("minecraft:crit"),
            cloud: r.particle_key("minecraft:poof"),
            ember: r.particle_key("minecraft:lava"),
            wisp: r.particle_key("minecraft:white_smoke"),
        });
    }

    fn server_about_to_start(&self, _: &mut dyn ServerHost, _: &mut ServerAboutToStartEvent) {
        game::server::clear();
        game::grinder::clear();
        game::firebox::clear();
    }

    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut ServerStoppedEvent) {
        game::server::clear();
        game::grinder::clear();
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
    }

    /// Recipes and tags are final: read the grinding recipes.
    fn tags_updated(&self, host: HostSide<'_>, _: &mut TagsUpdatedEvent) {
        if let HostSide::Server(host) = host {
            game::grinder::load(&*host);
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
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            game::server::level_tick(host, event.level);
        }
    }
}
