//! THROWAWAY SPIKE for FRAS milestone 0. Not released, not the start of the
//! real mod: it exists to find out what Horizon's mod API makes easy, hard or
//! impossible for a shaft-power mod. See ../FINDINGS.md.
//!
//! What it tries, on the real game:
//! - a ticking block entity (`shaft`, `crank`, `grinder` share one type);
//! - a shaft drawn by its block entity renderer and turned on the client's
//!   animation clock (`spin` + `keep`), so a shaft costs nothing per frame;
//! - a shaft network (`network.rs`, game-free and unit-tested) that pauses
//!   when any part is in a chunk that isn't simulated, and comes back at its
//!   saved speed after an unload or a restart;
//! - a block menu (the grinder, shown in a vanilla 3x3 menu with a screen
//!   drawn by the mod);
//! - a mod screen of its own (a handbook page, opened with F9).

pub mod network;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntitySubmitEvent, BlockEntityTickEvent, BlockEntityTicksEvent,
    BlockEntityTypeKey, BlockEntityUpdateTagEvent, BlockPart, BlocksEvent, ClientHost, Compound,
    CubeFace, Hooks, HostSide, InputKeyEvent, InteractionResult, ItemKey, ItemStack, KeyAction,
    LevelRef, LevelTickPostEvent, ModContainer, Registrar, RegistrationContext, ScreenDrawEvent,
    ScreenEvent, ScreenRenderPostEvent, ServerHost, Tag, UseWithoutItemEvent,
};
use network::{Axis, Kind, Lookup, Networks, Pos, World};

pub const ID: &str = "fras_spike";

#[derive(Default)]
struct Mod;

/// What the registration hands out, for the hooks.
struct Keys {
    gravel: ItemKey,
    cobblestone: ItemKey,
}
static KEYS: OnceLock<Keys> = OnceLock::new();

/// The server's view: every level's networks, and what this tick saw.
#[derive(Default)]
struct Server {
    nets: HashMap<LevelRef, Networks>,
    /// Parts whose block entity ticked this tick.
    ticked: HashMap<LevelRef, BTreeSet<Pos>>,
    /// Grinders with something to grind this tick.
    working: HashMap<LevelRef, BTreeSet<Pos>>,
}
static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn with_server<R>(f: impl FnOnce(&mut Server) -> R) -> R {
    f(SERVER.lock().unwrap().get_or_insert_with(Server::default))
}

/// The game's world as the network reads it. A part outside the simulation
/// distance counts as unloaded: its block entity doesn't tick there.
struct GameWorld<'a> {
    host: &'a dyn mod_api::Host,
    level: LevelRef,
}

impl World for GameWorld<'_> {
    fn lookup(&self, pos: Pos) -> Lookup {
        if !self.level.is_loaded(self.host, pos)
            || self.level.should_tick_blocks_at(self.host, pos) != Some(true)
        {
            return Lookup::Unloaded;
        }
        let Some(block) = self.level.block(self.host, pos) else {
            return Lookup::Unloaded;
        };
        match block.id(self.host).as_str() {
            "fras_spike:shaft" => {
                let axis = self.level.block_property(self.host, pos, "axis");
                Lookup::Part(Kind::Shaft(
                    axis.as_deref().and_then(Axis::from_name).unwrap_or(Axis::Y),
                ))
            }
            "fras_spike:crank" => Lookup::Part(Kind::Crank),
            "fras_spike:grinder" => Lookup::Part(Kind::Grinder),
            _ => Lookup::Empty,
        }
    }
}

fn get_f32(nbt: &Compound, key: &str) -> Option<f32> {
    match nbt.get(&key.into()) {
        Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
        _ => None,
    }
}

fn get_str(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}

/// The grinder's output slot, in a 3x3 (dispenser) menu: slot 0 is the input.
const GRINDER_OUT: usize = 8;

/// Grinding takes this much progress (RPM-ticks / 32 per tick).
const GRIND_WORK: f32 = 100.0;

/// The block entity Part of every spike part. Each block has a block entity
/// type of its own with the block's name: the hooks name the *type*
/// (`event.entity.id()`, and `event.block` in some), so one shared type
/// can't tell a shaft from a grinder.
struct Part;

#[mod_api::horizon_part]
impl BlockPart for Part {}

/// The crank's own block part: right-clicking it turns it for a second.
struct Crank;

#[mod_api::horizon_part]
impl BlockPart for Crank {
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        with_server(|s| s.nets.entry(level).or_default().crank(event.click.pos));
        Some(InteractionResult::Success)
    }
}

/// The grinder's own block part: right-clicking it opens its menu.
struct Grinder;

#[mod_api::horizon_part]
impl BlockPart for Grinder {
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let entity = host.block_entity(level, event.click.pos)?;
        let player = host.player(event.click.player)?;
        match player.open_menu(host, entity) {
            Ok(true) => Some(InteractionResult::Success),
            other => {
                eprintln!("[{ID}] grinder menu didn't open: {other:?}");
                None
            }
        }
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Part {
    fn block_entity_ticks(&self, _: &dyn mod_api::Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// The ticking block entity: report that the part is alive, take the
    /// network's speed into the saved data (so it survives an unload), and
    /// tell clients when the speed changed noticeably.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let pos = event.pos;
        let id = event.entity.id().to_owned();
        // No network yet (the first tick after loading): keep the saved
        // speed, which the network is about to be rebuilt from.
        let net = with_server(|s| {
            s.ticked.entry(level).or_default().insert(pos);
            let net = s.nets.get(&level).and_then(|n| n.at(pos));
            net.map(|n| (n.rpm, n.paused))
        });
        let saved = get_f32(&event.entity.nbt, "rpm").unwrap_or(0.0);
        let (rpm, paused) = net.unwrap_or((saved, true));
        // Grinder: count it working when its input slot holds cobblestone.
        let mut changed = false;
        if id.ends_with(":grinder") {
            let keys = KEYS.get().unwrap();
            let has_input = event
                .entity
                .items
                .first()
                .and_then(|s| s.as_ref())
                .is_some_and(|s| s.item() == keys.cobblestone);
            if has_input {
                with_server(|s| s.working.entry(level).or_default().insert(pos));
                let progress =
                    get_f32(&event.entity.nbt, "progress").unwrap_or(0.0) + rpm.abs() / 32.0;
                let (progress, done) = if progress >= GRIND_WORK {
                    (0.0, true)
                } else {
                    (progress, false)
                };
                if done {
                    let input = event.entity.items[0].take().unwrap();
                    event.entity.items[0] = input.shrink(1);
                    let out = &mut event.entity.items[GRINDER_OUT];
                    match out {
                        Some(stack) if stack.item() == keys.gravel => {
                            let _ = stack.grow(1);
                        }
                        Some(_) => {}
                        None => *out = ItemStack::new(keys.gravel, 1),
                    }
                }
                event
                    .entity
                    .nbt
                    .insert("progress".into(), Tag::Float(progress.to_bits()));
                // The screen's progress arrow reads data slot 0.
                if event.entity.data.is_empty() {
                    event.entity.data.push(0);
                }
                event.entity.data[0] = (progress / GRIND_WORK * 24.0) as i32;
                changed = true;
            }
        }
        // Save the speed with the part; resend when it moved by more than
        // 1 RPM or crossed zero, so clients animate between updates.
        let shown = if paused { 0.0 } else { rpm };
        let sent = get_f32(&event.entity.nbt, "shown").unwrap_or(0.0);
        if net.is_some() && saved != rpm {
            event
                .entity
                .nbt
                .insert("rpm".into(), Tag::Float(rpm.to_bits()));
            changed = true;
        }
        let mut resend = false;
        if (shown - sent).abs() > 1.0 || (shown == 0.0) != (sent == 0.0) {
            event
                .entity
                .nbt
                .insert("shown".into(), Tag::Float(shown.to_bits()));
            resend = true;
        }
        if id.ends_with(":shaft") {
            let axis = level
                .block_property(&*host, pos, "axis")
                .unwrap_or_else(|| "y".into());
            if get_str(&event.entity.nbt, "axis").as_deref() != Some(axis.as_str()) {
                event
                    .entity
                    .nbt
                    .insert("axis".into(), Tag::String(axis.as_str().into()));
                resend = true;
            }
        }
        if resend {
            host.block_entity_sync(pos);
            changed = true;
        }
        if !changed {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
        }
    }

    /// What a client gets: the shown speed and the shaft's axis.
    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        let mut tag = Compound::new();
        for key in ["shown", "axis"] {
            if let Some(v) = event.entity.nbt.get(&key.into()) {
                tag.insert(key.into(), v.clone());
            }
        }
        Some(tag)
    }

    /// The shaft, drawn once and then turned by the game on the animation
    /// clock: no per-frame call while its speed stays the same.
    fn block_entity_submit(&self, _: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        if !event.block.ends_with(":shaft") {
            return;
        }
        let rpm = get_f32(&event.entity.nbt, "shown").unwrap_or(0.0);
        let axis = get_str(&event.entity.nbt, "axis").unwrap_or_else(|| "y".into());
        let (v, from, to) = match axis.as_str() {
            "x" => ([1.0, 0.0, 0.0], [0.0, 6.0, 6.0], [16.0, 10.0, 10.0]),
            "z" => ([0.0, 0.0, 1.0], [6.0, 6.0, 0.0], [10.0, 10.0, 16.0]),
            _ => ([0.0, 1.0, 0.0], [6.0, 0.0, 6.0], [10.0, 16.0, 10.0]),
        };
        let speed = rpm * 0.3; // degrees per tick: 360 per turn, 1200 ticks a minute
        let draw = &mut *event.draw;
        let now = draw.animation_time();
        // Keep the shaft where it was when the speed changes.
        let phase = {
            let mut spins = CLIENT_SPINS.lock().unwrap();
            let entry = spins
                .get_or_insert_with(HashMap::new)
                .entry(event.pos)
                .or_insert((speed, 0.0));
            if entry.0 != speed {
                let angle = now * entry.0 as f64 + entry.1;
                *entry = (speed, angle - now * speed as f64);
            }
            (entry.1 % 360.0) as f32
        };
        let log = CubeFace::new("block/stripped_oak_log");
        let end = CubeFace::new("block/stripped_oak_log_top");
        let faces = match axis.as_str() {
            "x" => [
                Some(log),
                Some(log),
                Some(log),
                Some(log),
                Some(end),
                Some(end),
            ],
            "z" => [
                Some(log),
                Some(log),
                Some(end),
                Some(end),
                Some(log),
                Some(log),
            ],
            _ => [
                Some(end),
                Some(end),
                Some(log),
                Some(log),
                Some(log),
                Some(log),
            ],
        };
        // Sprites are named without a namespace ("block/..."): with
        // "minecraft:" the cube silently draws nothing.
        draw.spin(v, [0.5, 0.5, 0.5], speed, phase);
        if !draw.cube(from, to, faces, 0xFFFFFF) {
            eprintln!("[{ID}] shaft cube refused a sprite");
        }
        draw.still();
        draw.keep();
    }

    /// The grinder's screen: the 3x3 menu's nine slots, seven of them hidden.
    fn screen(
        &self,
        _: &dyn mod_api::Host,
        event: &mut ScreenEvent<'_>,
    ) -> Option<mod_api::Screen> {
        if !event.block.ends_with(":grinder") {
            return None;
        }
        Some(mod_api::Screen {
            width: 176.0,
            height: 133.0,
            slots: (0..9)
                .map(|i| match i {
                    0 => Some([44.0, 20.0]),
                    GRINDER_OUT => Some([116.0, 20.0]),
                    _ => None,
                })
                .collect(),
            inventory_y: 51.0,
            inventory_x: 8.0,
            own_labels: false,
            own_background: false,
            recipe_book: None,
            recipe_book_toggle: None,
        })
    }

    /// The progress arrow, from data slot 0.
    fn screen_draw(&self, _: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        if !event.block.ends_with(":grinder") {
            return;
        }
        let progress = event.data.first().copied().unwrap_or(0) as f32;
        let [x, y] = event.origin;
        event
            .draw
            .fill([x + 72.0, y + 26.0, 24.0, 4.0], 0xFF55_5555);
        event
            .draw
            .fill([x + 72.0, y + 26.0, progress, 4.0], 0xFFFF_AA00);
    }
}

/// The client's memory of each shaft's spin: degrees per tick and phase.
static CLIENT_SPINS: Mutex<Option<HashMap<Pos, (f32, f64)>>> = Mutex::new(None);

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn blocks(&self, _: &RegistrationContext, event: &mut BlocksEvent<'_>) {
        // Templates: an iron chain has the `axis` property and a thin shape.
        event.blocks.push(mod_api::Block::new(
            "fras_spike:shaft",
            "minecraft:iron_chain",
        ));
        event.blocks.push(mod_api::Block::new(
            "fras_spike:crank",
            "minecraft:oak_planks",
        ));
        event.blocks.push(mod_api::Block::new(
            "fras_spike:grinder",
            "minecraft:smooth_stone",
        ));
    }

    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for block in ["shaft", "crank", "grinder"] {
            let kind: BlockEntityTypeKey = r.block_entity_type(block).blocks(&[block]).build();
            if let Err(e) = r.block_entity_part(&kind, Part) {
                eprintln!("[{ID}] block entity part {block}: {e:?}");
            }
        }
        let crank = r.block_key("fras_spike:crank");
        r.block_part(crank, Crank);
        let grinder = r.block_key("fras_spike:grinder");
        r.block_part(grinder, Grinder);
        r.block_container(
            &grinder,
            ModContainer {
                slots: 9,
                faces: [
                    vec![GRINDER_OUT],
                    vec![0],
                    vec![0],
                    vec![0],
                    vec![0],
                    vec![0],
                ],
                inputs: vec![0],
                outputs: vec![GRINDER_OUT],
                menu: Some("minecraft:generic_3x3".into()),
            },
        );
        let _ = KEYS.set(Keys {
            gravel: r.item_key("minecraft:gravel"),
            cobblestone: r.item_key("minecraft:cobblestone"),
        });
    }

    /// After a level's block entities ticked: find what changed, rebuild
    /// those networks, and run the speed model one tick.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let level = event.level;
        let world = GameWorld {
            host: &*host,
            level,
        };
        with_server(|s| {
            let ticked = s.ticked.remove(&level).unwrap_or_default();
            let working = s.working.remove(&level).unwrap_or_default();
            let nets = s.nets.entry(level).or_default();
            // A part that ticked but has no network is new or just loaded.
            for pos in &ticked {
                if nets.at(*pos).is_none() {
                    nets.mark(*pos);
                }
            }
            // A member that didn't tick was broken, or its chunk left the
            // simulation: look again (a paused network already knows).
            let stale: Vec<Pos> = s
                .nets
                .get(&level)
                .map(|n| n.members_not_in(&ticked))
                .unwrap_or_default();
            let nets = s.nets.get_mut(&level).unwrap();
            for pos in stale {
                if nets.at(pos).is_some_and(|n| !n.paused) {
                    nets.mark(pos);
                }
            }
            let saved = |pos: Pos| {
                let entity = host.block_entity(level, pos)?;
                get_f32(&entity.data(&*host)?, "rpm")
            };
            nets.rebuild(&world, &saved);
            nets.tick(&working);
        });
    }

    /// F9 opens the handbook page, a mod screen drawn by the mod.
    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action == KeyAction::Press && event.key == "key.keyboard.f9" && host.playing() {
            host.mod_screen_open(ID);
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if host.mod_screen() != Some(ID) {
            return;
        }
        let [width, height] = event.draw.size();
        let (w, h) = (192.0, 180.0);
        let (x, y) = (((width - w) / 2.0).floor(), ((height - h) / 2.0).floor());
        event.draw.fill([x, y, w, h], 0xFFF3_E9D2);
        event.draw.fill([x, y, w, 2.0], 0xFF8B_5A2B);
        let lines = [
            "Engineer's Handbook (spike)",
            "",
            "Shafts turn at the speed of",
            "their network. Cranks push,",
            "machines pull; the network",
            "settles where they balance.",
        ];
        for (i, line) in lines.iter().enumerate() {
            event.draw.text(
                line,
                [x + 12.0, y + 12.0 + 11.0 * i as f32],
                0xFF3B_2A1A,
                false,
            );
        }
    }
}
