//! The firebox on Horizon (steam and heat spec 3): one fuel slot, burning
//! whenever it holds fuel, its heat handed to the plant; the fuel table read
//! from data; its menu and screen.

use std::sync::RwLock;

use mod_api::{
    BlockEntityTickEvent, BlockEntityTickReply, ClientHost, Host, Ingredient, ItemStack,
    ModContainer, PackType, ScreenDrawEvent, ScreenEvent, ServerHost,
};

use super::blocks;
use super::server::with_level;
use super::steam;
use crate::sim::geom::Pos;
use crate::sim::steam::fuel::{self, Table};

/// The firebox's container: one fuel slot, filled from the top and sides.
/// Nothing takes from it (the empty bucket a lava bucket leaves stays for
/// the player).
pub fn container() -> ModContainer {
    ModContainer {
        slots: 1,
        faces: [vec![], vec![0], vec![0], vec![0], vec![0], vec![0]],
        inputs: vec![0],
        outputs: vec![],
        menu: Some("fras:firebox".into()),
    }
}

struct Fuels {
    table: Table,
    tags: Vec<(String, Ingredient)>,
}

static FUELS: RwLock<Option<Fuels>> = RwLock::new(None);

pub fn clear() {
    *FUELS.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Read the fuel table after the server's data loaded.
pub fn load(host: &dyn Host) {
    let id = mod_api::Identifier::parse("fras:fras/firebox_fuels.json").ok();
    let table = id
        .and_then(|id| host.resource(PackType::ServerData, &id))
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| fuel::parse(&text))
        .and_then(|r| {
            r.map_err(|e| eprintln!("[fras] firebox_fuels.json: {e}"))
                .ok()
        })
        .unwrap_or_else(fuel::builtin);
    let mut tags = Vec::new();
    for tag in table.tags() {
        if let Ok(Some(ing)) =
            host.recipe_ingredient(&mod_api::serde_json::json!(format!("#{tag}")).to_string())
        {
            tags.push((tag, ing));
        }
    }
    *FUELS.write().unwrap_or_else(|e| e.into_inner()) = Some(Fuels { table, tags });
}

/// The heat rate of a fuel stack, kW.
pub fn heat(host: &dyn Host, stack: &ItemStack) -> f64 {
    let guard = FUELS.read().unwrap_or_else(|e| e.into_inner());
    let id = stack.item().id(host);
    match guard.as_ref() {
        Some(f) => f.table.heat(&id, &|_, tag| {
            f.tags
                .iter()
                .find(|(t, _)| t == tag)
                .is_some_and(|(_, ing)| ing.test(host, stack))
        }),
        None => fuel::builtin().heat(&id, &|_, _| false),
    }
}

/// Burn: take a new item when the last has burnt, count down while the
/// plant runs, tell the plant the heat, keep the menu's data.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let pos = Pos::from_array(event.pos);
    let nbt = &event.entity.nbt;
    let mut burn = blocks::get_i32(nbt, "burn").unwrap_or(0);
    let mut total = blocks::get_i32(nbt, "burn_total").unwrap_or(0);
    let mut kw = blocks::get_f64(nbt, "kw").unwrap_or(0.0);
    let (old_burn, old_total, old_kw) = (burn, total, kw);
    let paused = steam::paused(dim, pos);
    if event.entity.items.is_empty() {
        event.entity.items.resize(1, None);
    }
    let mut items_changed = false;
    if burn <= 0 && !paused {
        let stack = event.entity.items[0].clone();
        if let Some(stack) = stack {
            let time = host.item_burn_time(stack.item()).unwrap_or(0);
            if time > 0 {
                kw = heat(&*host, &stack);
                burn = time;
                total = time;
                let lava = stack.item().id(&*host) == "minecraft:lava_bucket";
                event.entity.items[0] = if lava {
                    super::keys().and_then(|k| ItemStack::new(k.bucket, 1))
                } else {
                    stack.shrink(1)
                };
                items_changed = true;
            }
        }
    }
    if burn > 0 && !paused {
        burn -= 1;
    }
    let lit = burn > 0;
    steam::set_heat(dim, pos, if lit { kw } else { 0.0 });
    let boilers = with_level(dim, |l| l.steam.works.firebox_boilers(pos)) as i32;
    if lit != (old_burn > 0) {
        with_level(dim, |l| {
            l.steam.lit.insert(pos, lit);
        });
    }
    let data = vec![burn, total.max(1), kw.round() as i32, boilers];
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let changed = burn != old_burn || total != old_total || kw != old_kw;
    if changed {
        let nbt = &mut event.entity.nbt;
        blocks::put_i32(nbt, "burn", burn);
        blocks::put_i32(nbt, "burn_total", total);
        blocks::put_f64(nbt, "kw", kw);
    }
    if !(changed || items_changed || data_changed) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// The furnace's layout with only the fuel slot.
pub fn screen(_: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some([56.0, 53.0])],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// The flame (burn time left) and the lines "1000 kW" and "Heating ...".
pub fn screen_draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (burn, total, kw, boilers) = (d(0), d(1).max(1), d(2), d(3));
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    // The flame: a bar that shrinks as the fuel burns.
    let frac = (burn as f32 / total as f32).clamp(0.0, 1.0);
    draw.fill([x + 57.0, y + 36.0, 14.0, 14.0], 0xFF40_4040);
    if burn > 0 {
        let h = 14.0 * frac;
        draw.fill([x + 57.0, y + 36.0 + 14.0 - h, 14.0, h], 0xFFFF_8A1E);
    }
    let tr = |key: &str, arg: Option<String>| {
        let t = host.translate(key).unwrap_or_else(|| key.to_string());
        match arg {
            Some(a) => t.replace("%s", &a).replace("%1$s", &a),
            None => t,
        }
    };
    let heat = if burn > 0 {
        tr("fras.firebox.heat", Some(format!("{kw}")))
    } else {
        tr("fras.firebox.out", None)
    };
    draw.text(&heat, [x + 80.0, y + 30.0], 0xFF40_4040, false);
    let heating = if boilers == 0 {
        tr("fras.firebox.heating_nothing", None)
    } else {
        tr("fras.firebox.heating", Some(format!("{boilers}")))
    };
    let color = if boilers == 0 {
        0xFFB0_2020
    } else {
        0xFF40_4040
    };
    draw.text(&heating, [x + 80.0, y + 42.0], color, false);
    if burn > 0 {
        let secs = burn / 20;
        draw.text(
            &tr(
                "fras.firebox.left",
                Some(format!("{}:{:02}", secs / 60, secs % 60)),
            ),
            [x + 80.0, y + 54.0],
            0xFF40_4040,
            false,
        );
    }
}

/// The firebox's menu (`fras:firebox`): its slot takes only what burns
/// (an item with no burn time is refused, spec 3.1).
pub struct FuelSlot;

#[mod_api::horizon_part]
impl mod_api::MenuPart for FuelSlot {
    fn menu_slot_rules(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::MenuSlotRulesEvent<'_>,
    ) {
        if event.container == mod_api::MenuContainer::Main && event.slot == 0 {
            event.may_place &= host.item_burn_time(event.stack.item()).unwrap_or(0) > 0;
        }
    }
}
