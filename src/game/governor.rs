//! The governor on Horizon (regulation spec 3): its block entity keeps the
//! menu's data and the look clients draw (ball step, throttle lever), and its
//! settings screen: target and sensitivity, − and + buttons sent to the
//! server as menu button clicks.

use mod_api::{
    BlockEntityTickEvent, BlockEntityTickReply, ClientHost, ModContainer, ScreenClickEvent,
    ScreenDrawEvent, ScreenEvent, ServerHost,
};

use super::blocks;
use super::server::with_level;
use crate::sim::geom::Pos;
use crate::sim::steam::govern;

/// One hidden slot that no face reaches (Horizon's menus need at least one):
/// the menu only carries data [target, sensitivity, speed × 10, ball × 10,
/// throttle ‰, engines, swing × 10].
pub fn container() -> ModContainer {
    ModContainer {
        slots: 1,
        faces: [vec![], vec![], vec![], vec![], vec![], vec![]],
        inputs: vec![],
        outputs: vec![],
        menu: Some("fras:governor".into()),
    }
}

pub const DATA_SLOTS: usize = 7;

/// Menu data and look, every tick.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let pos = Pos::from_array(event.pos);
    let (info, look) = with_level(dim, |l| {
        (
            l.steam.works.governor_info(pos),
            l.steam.works.governor_look(pos),
        )
    });
    let nbt = &event.entity.nbt;
    let target = blocks::get_f64(nbt, "target").unwrap_or(govern::DEFAULT_TARGET);
    let sens = blocks::get_i32(nbt, "sens").unwrap_or(govern::DEFAULT_SENS as i32);
    let data = match &info {
        Some(i) => vec![
            i.target.round() as i32,
            i.sens as i32,
            (i.speed * 10.0).round() as i32,
            (i.ball * 10.0).round() as i32,
            (i.throttle * 1000.0).round() as i32,
            i.engines as i32,
            (i.swing * 10.0).round() as i32,
        ],
        None => vec![target.round() as i32, sens, 0, 0, 1000, 0, 0],
    };
    let data_changed = event.entity.data != data;
    if data_changed {
        event.entity.data = data;
    }
    let nbt = &event.entity.nbt;
    let look_changed = blocks::get_i32(nbt, "ball") != Some(look.0 as i32)
        || blocks::get_i32(nbt, "lever") != Some(look.1 as i32);
    if look_changed {
        let nbt = &mut event.entity.nbt;
        blocks::put_i32(nbt, "ball", look.0 as i32);
        blocks::put_i32(nbt, "lever", look.1 as i32);
        host.block_entity_sync(event.pos);
    }
    if !(data_changed || look_changed) {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// A screen with its one slot hidden and no inventory below.
pub fn screen(_: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: 96.0,
        slots: vec![None],
        inventory_y: 1000.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// The four buttons: (id, x, y) from the screen's top left, 12 × 12.
const BUTTONS: [(i32, f32, f32); 4] = [
    (0, 96.0, 18.0),
    (1, 150.0, 18.0),
    (2, 96.0, 34.0),
    (3, 150.0, 34.0),
];
const BUTTON: f32 = 12.0;

fn tr(host: &dyn ClientHost, key: &str, args: &[String]) -> String {
    let mut t = host.translate(key).unwrap_or_else(|| key.to_string());
    for (i, a) in args.iter().enumerate() {
        t = t.replacen("%s", a, 1).replace(&format!("%{}$s", i + 1), a);
    }
    t
}

pub fn screen_draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (target, sens, speed, ball, throttle, engines, swing) =
        (d(0), d(1), d(2), d(3), d(4), d(5), d(6));
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let grey = 0xFF40_4040;
    draw.text(
        &tr(host, "fras.governor.target", &[]),
        [x + 8.0, y + 20.0],
        grey,
        false,
    );
    draw.text(
        &tr(host, "fras.governor.sensitivity", &[]),
        [x + 8.0, y + 36.0],
        grey,
        false,
    );
    draw.text(&format!("{target} RPM"), [x + 112.0, y + 20.0], grey, false);
    draw.text(
        &format!("{sens} ({})", govern::band(sens.clamp(1, 5) as u8)),
        [x + 112.0, y + 36.0],
        grey,
        false,
    );
    for (id, bx, by) in BUTTONS {
        draw.fill([x + bx, y + by, BUTTON, BUTTON], 0xFF8B_8B8B);
        draw.fill(
            [x + bx + 1.0, y + by + 1.0, BUTTON - 2.0, BUTTON - 2.0],
            0xFFC6_C6C6,
        );
        let label = if id % 2 == 0 { "-" } else { "+" };
        draw.text(label, [x + bx + 3.5, y + by + 2.0], grey, false);
    }
    // Speed and balls, then the throttle as a bar.
    draw.text(
        &tr(
            host,
            "fras.governor.speed",
            &[
                super::num(speed as f64 / 10.0),
                super::num(ball as f64 / 10.0),
            ],
        ),
        [x + 8.0, y + 54.0],
        grey,
        false,
    );
    let (bx, by, bw) = (x + 8.0, y + 68.0, 120.0);
    draw.fill([bx, by, bw, 6.0], 0xFF40_4040);
    draw.fill(
        [bx, by, bw * (throttle.clamp(0, 1000) as f32 / 1000.0), 6.0],
        0xFF3C_A03C,
    );
    draw.text(
        &format!("{}%", throttle / 10),
        [bx + bw + 4.0, by - 1.0],
        grey,
        false,
    );
    let band = govern::band(sens.clamp(1, 5) as u8);
    let note = if speed == 0 {
        tr(host, "fras.governor.not_turning", &[])
    } else if swing as f64 / 10.0 > band / 2.0 {
        tr(
            host,
            "fras.governor.hunting",
            &[super::num(swing as f64 / 20.0)],
        )
    } else {
        tr(host, "fras.governor.engines", &[engines.to_string()])
    };
    draw.text(&note, [x + 8.0, y + 80.0], grey, false);
}

pub fn screen_click(_: &dyn ClientHost, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    // `at` is in GUI pixels from the screen's top left, as the buttons are.
    let [ax, ay] = event.at;
    BUTTONS
        .into_iter()
        .find(|(_, x, y)| ax >= *x && ax < x + BUTTON && ay >= *y && ay < y + BUTTON)
        .map(|(id, _, _)| id)
}

/// A button pressed: step the target or sensitivity and tell the plant.
pub fn menu_button(host: &mut dyn ServerHost, event: &mut mod_api::MenuButtonEvent<'_>) {
    let id = event.context.id;
    let pos = Pos::from_array(event.context.pos);
    let nbt = &mut event.entity.nbt;
    let target =
        govern::clean_target(blocks::get_f64(nbt, "target").unwrap_or(govern::DEFAULT_TARGET));
    let sens = blocks::get_i32(nbt, "sens").unwrap_or(govern::DEFAULT_SENS as i32);
    match id {
        0 | 1 => blocks::put_f64(nbt, "target", govern::next_target(target, id == 1)),
        2 | 3 => blocks::put_i32(
            nbt,
            "sens",
            (sens + if id == 3 { 1 } else { -1 }).clamp(1, 5),
        ),
        _ => return,
    }
    let dim = host
        .player(event.context.player)
        .and_then(|p| p.level(&*host))
        .and_then(|level| super::server::dim_of(&*host, level))
        .unwrap_or_else(|| host.dimension());
    // The block entity is written back after this call; read it again then.
    with_level(dim, |l| {
        l.steam.governors_dirty.insert(pos);
    });
}
