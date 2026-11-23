//! Fire, Rust and Steam (mod id `fras`): shaft power for Horizon.
//!
//! `sim` holds the rules, free of the game and unit-tested; `game` is the
//! thin layer that puts them on Horizon through hooks and parts (no mixins).

pub mod game;
pub mod sim;

use mod_api::{
    self, Hooks, HostSide, LevelTickPostEvent, Registrar, RegistrationContext,
    ServerAboutToStartEvent, ServerHost, ServerStoppedEvent, TagsUpdatedEvent,
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
        }
        r.register_recipe_type("grinding");
        let wrench = r.item_key("fras:wrench");
        let leather_belt = r.item_key("fras:leather_belt");
        r.item_part(wrench, game::tools::Wrench);
        r.item_part(leather_belt, game::tools::LeatherBelt);
        for item in game::blocks::Block::ALL
            .map(Block::name)
            .into_iter()
            .chain(ITEMS)
        {
            let key = r.item_key(&format!("{ID}:{item}"));
            r.item_part(key, game::tools::Tooltip(item));
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
            smoke: r.particle_key("minecraft:smoke"),
            crit: r.particle_key("minecraft:crit"),
            cloud: r.particle_key("minecraft:poof"),
        });
    }

    fn server_about_to_start(&self, _: &mut dyn ServerHost, _: &mut ServerAboutToStartEvent) {
        game::server::clear();
        game::grinder::clear();
    }

    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut ServerStoppedEvent) {
        game::server::clear();
        game::grinder::clear();
    }

    /// Recipes and tags are final: read the grinding recipes.
    fn tags_updated(&self, host: HostSide<'_>, _: &mut TagsUpdatedEvent) {
        if let HostSide::Server(host) = host {
            game::grinder::load(&*host);
        }
    }

    /// After each level's tick: run its networks.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            game::server::level_tick(host, event.level);
        }
    }
}
