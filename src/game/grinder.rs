//! The grinder on Horizon (grinder spec 4 to 6 and 9): recipes read from
//! `fras:grinding` data, the tick, the push down, the menu and its screen.

use std::collections::HashMap;
use std::sync::RwLock;

use mod_api::{
    BlockClick, BlockEntityTickEvent, BlockEntityTickReply, ClientHost, Host, HostTransaction,
    Identifier, Ingredient, InteractionResult, ItemKey, ItemResource, ItemStack, ModContainer,
    STORAGE, ScreenDrawEvent, ScreenEvent, ServerHost,
};

use super::blocks;
use super::server::{self, with_level};
use crate::sim::geom::{Dir, Pos};
use crate::sim::grind::{self, Items, Output, Recipe, Stack, Status};

/// Slots: input, then four outputs.
pub const SLOTS: usize = 1 + grind::OUTPUTS;

/// The grinder's container: a hopper's five slots; anything inserts into
/// the input, anything pulling takes from the outputs (spec 5.1).
pub fn container() -> ModContainer {
    let all: Vec<usize> = (0..SLOTS).collect();
    ModContainer {
        slots: SLOTS,
        faces: [
            vec![1, 2, 3, 4],
            all.clone(),
            all.clone(),
            all.clone(),
            all.clone(),
            all,
        ],
        inputs: vec![0],
        outputs: vec![1, 2, 3, 4],
        menu: Some("minecraft:hopper".into()),
    }
}

// ---- Recipes -------------------------------------------------------------------

#[derive(Default)]
struct Book {
    recipes: Vec<Recipe>,
    /// Tag ingredients, by tag, for matching.
    tags: HashMap<String, Ingredient>,
}

static BOOK: RwLock<Option<Book>> = RwLock::new(None);

pub fn clear() {
    *BOOK.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Read one recipe file's JSON.
pub fn parse(id: &str, json: &mod_api::serde_json::Value) -> Result<Recipe, String> {
    let ingredient = match &json["ingredient"] {
        mod_api::serde_json::Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => grind::Ingredient::Tag(tag.into()),
            None => grind::Ingredient::Item(s.clone()),
        },
        v if v["tag"].is_string() => {
            grind::Ingredient::Tag(v["tag"].as_str().unwrap_or_default().into())
        }
        v if v["item"].is_string() => {
            grind::Ingredient::Item(v["item"].as_str().unwrap_or_default().into())
        }
        _ => return Err("no ingredient".into()),
    };
    let results = json["results"]
        .as_array()
        .ok_or("no results")?
        .iter()
        .map(|r| {
            Ok(Output {
                item: r["id"].as_str().ok_or("result without id")?.into(),
                count: r["count"].as_u64().unwrap_or(1) as u32,
                chance: r["chance"].as_f64().unwrap_or(1.0),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let recipe = Recipe {
        id: id.into(),
        ingredient,
        count: json["count"].as_u64().unwrap_or(1) as u32,
        results,
        revolutions: json["revolutions"].as_f64().ok_or("no revolutions")?,
        load: json["load"].as_f64().unwrap_or(grind::DEFAULT_LOAD),
    };
    grind::validate(&recipe)?;
    Ok(recipe)
}

/// Read the recipes after the server's data loaded or reloaded.
pub fn load(host: &dyn Host) {
    let mut book = Book::default();
    for r in host.recipes_of_type("fras:grinding").iter() {
        match parse(&r.id, &r.json) {
            Ok(recipe) => {
                if let grind::Ingredient::Tag(tag) = &recipe.ingredient
                    && !book.tags.contains_key(tag)
                    && let Ok(Some(ing)) = host.recipe_ingredient(
                        &mod_api::serde_json::json!(format!("#{tag}")).to_string(),
                    )
                {
                    book.tags.insert(tag.clone(), ing);
                }
                book.recipes.push(recipe);
            }
            Err(e) => eprintln!("[fras] grinding recipe {} skipped: {e}", r.id),
        }
    }
    for (a, b) in grind::duplicates(&book.recipes) {
        eprintln!("[fras] grinding recipes {a} and {b} take the same input; the first wins");
    }
    *BOOK.write().unwrap_or_else(|e| e.into_inner()) = Some(book);
}

pub fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(id).ok()?)
}

struct HostItems<'a> {
    host: &'a dyn Host,
    tags: &'a HashMap<String, Ingredient>,
}

impl Items for HostItems<'_> {
    fn has_tag(&self, item: &str, tag: &str) -> bool {
        let (Some(ing), Some(key)) = (self.tags.get(tag), item_key(self.host, item)) else {
            return false;
        };
        ItemStack::new(key, 1).is_some_and(|s| ing.test(self.host, &s))
    }

    fn max_stack(&self, item: &str) -> u32 {
        item_key(self.host, item)
            .and_then(|k| self.host.max_stack_size(k))
            .unwrap_or(64)
    }
}

// ---- The tick --------------------------------------------------------------------

fn stack_of(host: &dyn Host, s: &Option<ItemStack>) -> Option<Stack> {
    s.as_ref()
        .map(|s| Stack::new(&s.item().id(host), s.count()))
}

/// A small random source for chance rolls.
fn random() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let mut x = STATE.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let Some(level) = host.level(dim) else {
        return;
    };
    let pos = Pos::from_array(event.pos);
    let saved = blocks::saved(&event.entity.nbt);
    let (rpm, paused, jammed) = with_level(dim, |l| match l.mech.part(pos) {
        Some(p) => {
            let (w, paused) = l.mech.speed(pos).unwrap_or((p.omega, true));
            (w, paused, p.jam.is_some())
        }
        // No network yet (the first tick after a load): keep what it saved.
        None => (
            saved.map_or(0.0, |s| s.omega),
            true,
            saved.is_some_and(|s| s.jam.is_some()),
        ),
    });

    let before = event.entity.items.clone();
    if event.entity.items.len() < SLOTS {
        event.entity.items.resize(SLOTS, None);
    }
    let input = stack_of(&*host, &event.entity.items[0]);
    let mut outputs: Vec<Option<Stack>> = (1..SLOTS)
        .map(|i| stack_of(&*host, &event.entity.items[i]))
        .collect();
    let mut work = grind::Work {
        progress: blocks::get_f64(&event.entity.nbt, "progress").unwrap_or(0.0),
        recipe: blocks::get_str(&event.entity.nbt, "recipe").filter(|s| !s.is_empty()),
    };
    let old_work = work.clone();
    let result = {
        let book = BOOK.read().unwrap_or_else(|e| e.into_inner());
        let empty = Book::default();
        let book = book.as_ref().unwrap_or(&empty);
        let items = HostItems {
            host: &*host,
            tags: &book.tags,
        };
        grind::tick(
            &mut work,
            input.as_ref(),
            &mut outputs,
            rpm,
            jammed,
            paused,
            &book.recipes,
            &items,
            &mut random,
        )
    };
    with_level(dim, |l| {
        l.loads.insert(pos, result.load);
    });

    // Write the items back where they changed.
    if result.consumed > 0
        && let Some(stack) = event.entity.items[0].take()
    {
        event.entity.items[0] = stack.shrink(result.consumed);
    }
    for (i, out) in outputs.iter().enumerate() {
        let slot = &mut event.entity.items[i + 1];
        if stack_of(&*host, slot) == *out {
            continue;
        }
        *slot = match out {
            None => None,
            Some(s) => match slot.as_mut() {
                Some(existing) if existing.item().id(&*host) == s.item => {
                    let _ = existing.set_count(s.count);
                    slot.take()
                }
                _ => item_key(&*host, &s.item).and_then(|k| ItemStack::new(k, s.count)),
            },
        };
    }

    // Every 8 ticks: push one item down out of the chute (spec 5.1).
    let tick = server::with(|s| s.ticks);
    if (tick + pos.x as u64 + pos.z as u64).is_multiple_of(grind::TRANSFER_EVERY) {
        push_down(host, level, pos, &mut event.entity.items);
    }

    // Menu data: progress ‰, RPM × 10, status, max RPM.
    let progress = if result.needed > 0.0 {
        (work.progress / result.needed * 1000.0) as i32
    } else {
        0
    };
    let data = vec![
        progress,
        (rpm * 10.0).round() as i32,
        result.status.code(),
        grind::working_range(false).1 as i32,
    ];
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let grinding = result.status == Status::Grinding;
    let input_id = input.as_ref().map(|s| s.item.clone()).unwrap_or_default();
    let nbt = &event.entity.nbt;
    let client_changed = blocks::get_bool(nbt, "grinding") != grinding
        || blocks::get_str(nbt, "input").unwrap_or_default() != input_id;
    let work_changed = work != old_work;
    if work_changed || client_changed {
        let nbt = &mut event.entity.nbt;
        blocks::put_f64(nbt, "progress", work.progress);
        blocks::put_str(nbt, "recipe", work.recipe.as_deref().unwrap_or(""));
        blocks::put_bool(nbt, "grinding", grinding);
        blocks::put_str(nbt, "input", &input_id);
    }
    if client_changed {
        host.block_entity_sync(event.pos);
    }
    let items_changed = event.entity.items != before;
    if items_changed {
        let slots: Vec<Option<(u32, u32)>> = event
            .entity
            .items
            .iter()
            .map(|s| {
                s.as_ref()
                    .map(|s| (s.count(), host.max_stack_size(s.item()).unwrap_or(64)))
            })
            .collect();
        let signal = grind::comparator(&slots) as i32;
        with_level(dim, |l| {
            l.grinder_signal.insert(pos, signal);
        });
    }
    if !(items_changed || work_changed || client_changed || data_changed) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// Move one item from the outputs into the inventory below, if it takes it.
fn push_down(
    host: &mut dyn ServerHost,
    level: mod_api::LevelRef,
    pos: Pos,
    items: &mut [Option<ItemStack>],
) {
    let Some(i) = (1..SLOTS).find(|i| items[*i].is_some()) else {
        return;
    };
    let Some(stack) = items[i].clone() else {
        return;
    };
    let below = pos.offset(Dir::Down);
    let resource = ItemResource::of(&stack);
    let Some(storage) = level.capability(
        host,
        &STORAGE,
        below.to_array(),
        Some(super::direction(Dir::Up)),
    ) else {
        return;
    };
    let moved: Result<u64, ()> =
        host.transaction(|host, tx| Ok(storage.insert(host, tx, &resource, 1)));
    if moved == Ok(1) {
        items[i] = stack.shrink(1);
    }
}

// ---- Menu and screen -------------------------------------------------------------

/// Right-click with anything but the wrench opens the menu (spec 5.2).
pub fn open(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> InteractionResult {
    let Some(level) = host.level(click.dimension.key().id()) else {
        return InteractionResult::Pass;
    };
    let (Some(entity), Some(player)) = (
        host.block_entity(level, click.pos),
        host.player(click.player),
    ) else {
        return InteractionResult::Pass;
    };
    match player.open_menu(host, entity) {
        Ok(true) => InteractionResult::Success,
        _ => InteractionResult::Pass,
    }
}

/// Where the slots go: input on the left, a 2×2 output grid on the right.
pub fn screen(_: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: 166.0,
        slots: vec![
            Some([44.0, 26.0]),
            Some([104.0, 17.0]),
            Some([122.0, 17.0]),
            Some([104.0, 35.0]),
            Some([122.0, 35.0]),
        ],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// Progress arrow, speed bar and status line, from the menu data.
pub fn screen_draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (progress, rpm10, status, max) = (d(0), d(1), Status::from_code(d(2)), d(3).max(1));
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    // Arrow.
    draw.fill([x + 66.0, y + 30.0, 30.0, 6.0], 0xFF55_5555);
    draw.fill(
        [
            x + 66.0,
            y + 30.0,
            30.0 * (progress.clamp(0, 1000) as f32 / 1000.0),
            6.0,
        ],
        0xFFFF_AA00,
    );
    // Speed bar: 0 to 128 RPM, the working band green, red above the max.
    let (bx, by, bw) = (x + 44.0, y + 58.0, 96.0);
    let scale = |rpm: f32| bw * (rpm / 128.0).clamp(0.0, 1.0);
    let (min, maxf) = (crate::sim::rot::part::GRINDER_MIN_RPM as f32, max as f32);
    draw.fill([bx, by, bw, 4.0], 0xFF40_4040);
    draw.fill(
        [bx + scale(min), by, scale(maxf) - scale(min), 4.0],
        0xFF3C_A03C,
    );
    draw.fill([bx + scale(maxf), by, bw - scale(maxf), 4.0], 0xFFB0_3030);
    let rpm = (rpm10 as f32 / 10.0).abs();
    draw.fill([bx + scale(rpm) - 1.0, by - 2.0, 2.0, 8.0], 0xFFFF_FFFF);
    draw.text(
        &format!("{} RPM", super::num(rpm as f64)),
        [bx + bw + 4.0, by - 2.0],
        0xFF40_4040,
        false,
    );
    // Status line.
    let key = format!("fras.grinder.status.{}", status.key());
    let text = host.translate(&key).unwrap_or(key);
    let color = match status {
        Status::Jammed | Status::Overspeed => 0xFFB0_2020,
        Status::Grinding => 0xFF20_7020,
        _ => 0xFF40_4040,
    };
    draw.text(&text, [x + 44.0, y + 66.0], color, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_files_parse() {
        let json = mod_api::serde_json::json!({
            "type": "fras:grinding",
            "ingredient": "minecraft:raw_iron",
            "results": [{"id": "fras:crushed_iron"}, {"id": "fras:crushed_iron", "chance": 0.5}],
            "revolutions": 4
        });
        let r = parse("fras:grinding/raw_iron", &json).unwrap();
        assert_eq!(
            r.ingredient,
            grind::Ingredient::Item("minecraft:raw_iron".into())
        );
        assert_eq!(r.results.len(), 2);
        assert_eq!(r.load, grind::DEFAULT_LOAD);
        let tag = mod_api::serde_json::json!({"ingredient": "#minecraft:wool", "results": [{"id": "minecraft:string", "count": 4}], "revolutions": 2, "load": 3});
        assert_eq!(
            parse("x", &tag).unwrap().ingredient,
            grind::Ingredient::Tag("minecraft:wool".into())
        );
        assert!(parse("x", &mod_api::serde_json::json!({"results": []})).is_err());
    }

    #[test]
    fn the_data_files_match_the_built_in_recipes() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/data/fras/recipe/grinding");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let json: mod_api::serde_json::Value =
                mod_api::serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let id = format!(
                "fras:grinding/{}",
                path.file_stem().unwrap().to_str().unwrap()
            );
            found.push(parse(&id, &json).unwrap());
        }
        found.sort_by(|a, b| a.id.cmp(&b.id));
        let mut builtin = grind::builtin();
        builtin.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(found, builtin);
    }
}
