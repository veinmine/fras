//! The furnace damper's and emergency valve's screens: right-click one
//! (with anything but the wrench) to set its pressure in tenths of a bar,
//! with −1, −0.1, +0.1 and +1 buttons sent to the server as menu button
//! clicks. The damper's screen also charts its flap's opening over the last
//! 1, 5 or 10 minutes, like the gauges' readout (`readout.rs`), from the
//! same in-memory history.
//!
//! Menu data, fixed point, each within a short:
//! - damper: [set × 10, bar × 100 (−1 off a boiler), opening ‰, status,
//!   red line × 100, head, range, sample 0 … 119] (samples: opening ‰);
//! - emergency valve: [set × 10, bar × 100, red line × 100, state, mB/s].

use mod_api::{
    BlockEntityTickEvent, BlockEntityTickReply, ClientHost, ModContainer, ScreenClickEvent,
    ScreenDrawEvent, ScreenEvent, ServerHost,
};

use super::blocks::{self, Block};
use super::readout::{self, Chart, GREY, RED, Ring, SAMPLES};
use super::server::with_level;
use crate::sim::fluid::valve;
use crate::sim::geom::Pos;
use crate::sim::steam::damper;

/// Whether the block has this screen.
pub fn has(block: Block) -> bool {
    matches!(block, Block::FurnaceDamper | Block::EmergencyValve)
}

/// One hidden slot (menus need one) and the data.
pub fn container(block: Block) -> ModContainer {
    ModContainer {
        slots: 1,
        faces: [vec![], vec![], vec![], vec![], vec![], vec![]],
        inputs: vec![],
        outputs: vec![],
        menu: Some(format!("fras:{}", block.name())),
    }
}

/// The damper's slots before the samples.
const DAMPER_HEAD: usize = 7;

/// The damper's state as its screen shows it.
const WORKING: i32 = 0;
const ALONE: i32 = 1;
const REDSTONE: i32 = 2;
const STUCK: i32 = 3;

/// Menu data, every tick for a damper (its chart samples on given ticks),
/// every 4th for an emergency valve.
pub fn tick(block: Block, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = Pos::from_array(event.pos);
    // An emergency valve's screen needs no more than every 4th tick, spread
    // by position.
    let ticks = super::server::with(|s| s.ticks);
    let spread = (pos.x ^ pos.z).rem_euclid(4) as u64;
    if block == Block::EmergencyValve && !(ticks + spread).is_multiple_of(4) {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    let dim = host.dimension();
    let nbt = &event.entity.nbt;
    let data = if block == Block::FurnaceDamper {
        let info = with_level(dim, |l| l.steam.works.damper_info(pos));
        let saved_set = blocks::get_f64(nbt, "set").unwrap_or(damper::DEFAULT_SET);
        let saved_opening = blocks::get_f64(nbt, "opening").unwrap_or(1.0);
        let (set, bar, opening, status, red) = match &info {
            Some(d) => (
                d.set,
                d.bar,
                d.opening,
                if d.stuck {
                    STUCK
                } else if d.powered {
                    REDSTONE
                } else if d.bar.is_none() {
                    ALONE
                } else {
                    WORKING
                },
                d.red.unwrap_or(0.0),
            ),
            None => (saved_set, None, saved_opening, ALONE, 0.0),
        };
        let opening = readout::fixed(opening.clamp(0.0, 1.0), 1000.0);
        let range = readout::range(nbt);
        let (samples, head) = readout::sample(dim, pos, opening, range);
        let mut data = Vec::with_capacity(DAMPER_HEAD + SAMPLES);
        data.extend([
            readout::fixed(set, 10.0),
            bar.map_or(-1, |b| readout::fixed(b, 100.0)),
            opening,
            status,
            readout::fixed(red, 100.0),
            head as i32,
            range.min(readout::RANGES.len() - 1) as i32,
        ]);
        data.extend(samples);
        data
    } else {
        let saved_set = blocks::get_f64(nbt, "set").unwrap_or(valve::DEFAULT_SET);
        let (set, gauge, vent) = with_level(dim, |l| {
            let w = &l.steam.works;
            let part = w.part(pos).map(|p| (p.spec.set, p.spec.dir));
            let gauge = part.map(|(_, dir)| w.gauge(pos.offset(dir)));
            (part.map(|(s, _)| s), gauge, w.vent_state(pos))
        });
        let (bar, red) = gauge.map_or((0.0, 0.0), |g| (g.bar, g.red));
        let state = if vent.open {
            2
        } else if vent.simmer {
            1
        } else {
            0
        };
        vec![
            readout::fixed(set.unwrap_or(saved_set), 10.0),
            readout::fixed(bar, 100.0),
            readout::fixed(red, 100.0),
            state,
            readout::fixed(vent.rate, 1.0),
        ]
    };
    if event.entity.data != data {
        event.entity.data = data;
    } else {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

/// A screen with its one slot hidden and no inventory below.
pub fn screen(block: Block, _: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: if block == Block::FurnaceDamper {
            150.0
        } else {
            74.0
        },
        slots: vec![None],
        inventory_y: 1000.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

/// The set pressure buttons: id, x from the screen's left, width, label and
/// step in tenths of a bar. All in one row.
const BUTTONS: [(i32, f32, f32, &str, i32); 4] = [
    (10, 8.0, 20.0, "-1", -10),
    (11, 30.0, 26.0, "-0.1", -1),
    (12, 58.0, 26.0, "+0.1", 1),
    (13, 86.0, 20.0, "+1", 10),
];
const BUTTON_Y: f32 = 30.0;
const BUTTON_H: f32 = 12.0;

/// Translate, filling `%s` and `%1$s` and turning `%%` into `%`.
fn tr(host: &dyn ClientHost, key: &str, args: &[String]) -> String {
    readout::tr(host, key, args).replace("%%", "%")
}

/// "4.3" from 43 (tenths) or 4.3 from 430 (hundredths): as players read it.
fn bar(v: i32, scale: f64) -> String {
    super::num(v as f64 / scale)
}

pub fn screen_draw(block: Block, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let damper = block == Block::FurnaceDamper;
    let (set, red) = if damper { (d(0), d(4)) } else { (d(0), d(2)) };
    // The set pressure and the buttons below it.
    let key = if damper {
        "fras.wrench.damper_set"
    } else {
        "fras.wrench.set_pressure"
    };
    draw.text(
        &tr(host, key, &[format!("{:.1}", set as f64 / 10.0)]),
        [x + 8.0, y + 18.0],
        GREY,
        false,
    );
    if red > 0 {
        let note = tr(
            host,
            "fras.readout.red_line",
            &[format!("{} bar", bar(red, 100.0))],
        );
        let w = draw.text_width(&note);
        draw.text(&note, [x + 168.0 - w, y + 18.0], GREY, false);
    }
    for (_, bx, w, label, _) in BUTTONS {
        let (bx, by) = (x + bx, y + BUTTON_Y);
        draw.fill([bx, by, w, BUTTON_H], 0xFF8B_8B8B);
        draw.fill([bx + 1.0, by + 1.0, w - 2.0, BUTTON_H - 2.0], 0xFFC6_C6C6);
        let tw = draw.text_width(label);
        draw.text(label, [bx + (w - tw) / 2.0, by + 2.0], GREY, false);
    }
    if damper {
        draw_damper(host, draw, [x, y], &d);
    } else {
        draw_valve(host, draw, [x, y], &d);
    }
}

fn draw_damper(
    host: &dyn ClientHost,
    draw: &mut dyn mod_api::DrawContext,
    origin: [f32; 2],
    d: &dyn Fn(usize) -> i32,
) {
    let [x, y] = origin;
    let (pressure, opening, status) = (d(1), d(2), d(3));
    let (head, range) = (d(5).max(0) as usize, d(6).max(0) as usize);
    let range = range.min(readout::RANGES.len() - 1);
    readout::range_buttons(draw, [x, y], range);
    // What it does now.
    let (line, color) = match status {
        STUCK => (tr(host, "fras.inspect.damper_stuck", &[]), RED),
        ALONE => (tr(host, "fras.inspect.damper_alone", &[]), RED),
        _ => (
            tr(
                host,
                "fras.inspect.damper",
                &[
                    bar(pressure.max(0), 100.0),
                    super::num(opening as f64 / 10.0),
                ],
            ),
            GREY,
        ),
    };
    draw.text(&line, [x + 8.0, y + 48.0], color, false);
    let note = match status {
        REDSTONE => tr(host, "fras.inspect.damper_redstone", &[]),
        WORKING if opening <= 0 => tr(host, "fras.inspect.banked", &[]),
        WORKING => tr(
            host,
            "fras.setpoint.damper_how",
            &[super::num(damper::BAND)],
        ),
        _ => String::new(),
    };
    if !note.is_empty() {
        super::text_in(draw, &note, [x + 8.0, y + 58.0], 160.0, 0xFF80_8080);
    }
    // The flap's opening over the range.
    let samples: Vec<i32> = (0..SAMPLES).map(|i| d(DAMPER_HEAD + i)).collect();
    let points = Ring::ordered(&samples, head % SAMPLES);
    draw.text(
        &tr(host, "fras.setpoint.opening_chart", &[]),
        [x + 8.0, y + 80.0],
        GREY,
        false,
    );
    let label = |v: i32| format!("{}", v / 10);
    readout::chart(
        draw,
        host,
        &Chart {
            area: [x + 8.0, y + 94.0, 160.0, 40.0],
            points: &points,
            top: 1000,
            step: 500,
            label: &label,
            range,
            limits: &[],
        },
    );
}

fn draw_valve(
    host: &dyn ClientHost,
    draw: &mut dyn mod_api::DrawContext,
    origin: [f32; 2],
    d: &dyn Fn(usize) -> i32,
) {
    let [x, y] = origin;
    let (pressure, state, rate) = (d(1), d(3), d(4));
    draw.text(
        &tr(host, "fras.setpoint.pressure", &[bar(pressure, 100.0)]),
        [x + 8.0, y + 48.0],
        GREY,
        false,
    );
    let rate = super::num(rate as f64);
    let (line, color) = match state {
        2 => (tr(host, "fras.inspect.vent_open", &[rate]), RED),
        1 => (tr(host, "fras.inspect.vent_simmer", &[rate]), 0xFFB0_7000),
        _ => (tr(host, "fras.inspect.vent_closed", &[rate]), GREY),
    };
    draw.text(&line, [x + 8.0, y + 60.0], color, false);
}

pub fn screen_click(block: Block, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    if block == Block::FurnaceDamper
        && let Some(id) = readout::range_click(event.at)
    {
        return Some(id);
    }
    // `at` is in GUI pixels from the screen's top left, as the buttons are.
    let [ax, ay] = event.at;
    if ay < BUTTON_Y || ay >= BUTTON_Y + BUTTON_H {
        return None;
    }
    BUTTONS
        .into_iter()
        .find(|(_, bx, w, _, _)| ax >= *bx && ax < bx + w)
        .map(|(id, ..)| id)
}

/// A button pressed: move the set pressure (or the damper's chart range)
/// and tell the plant.
pub fn menu_button(
    block: Block,
    host: &mut dyn ServerHost,
    event: &mut mod_api::MenuButtonEvent<'_>,
) {
    if block == Block::FurnaceDamper && readout::range_button(event) {
        return;
    }
    let id = event.context.id;
    let Some((.., tenths)) = BUTTONS.into_iter().find(|(b, ..)| *b == id) else {
        return;
    };
    let delta = tenths as f64 / 10.0;
    let pos = Pos::from_array(event.context.pos);
    let nbt = &mut event.entity.nbt;
    let next = if block == Block::FurnaceDamper {
        damper::adjust(
            blocks::get_f64(nbt, "set").unwrap_or(damper::DEFAULT_SET),
            delta,
        )
    } else {
        valve::adjust(
            blocks::get_f64(nbt, "set").unwrap_or(valve::DEFAULT_SET),
            delta,
        )
    };
    blocks::put_f64(nbt, "set", next);
    let dim = host
        .player(event.context.player)
        .and_then(|p| p.level(&*host))
        .and_then(|level| super::server::dim_of(&*host, level))
        .unwrap_or_else(|| host.dimension());
    // The block entity is written back after this call; read it again then.
    with_level(dim, |l| {
        l.steam.settings_dirty.insert(pos);
    });
}
