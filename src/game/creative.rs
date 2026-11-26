//! The creative-only debugging blocks: an engine that holds a set speed, a
//! load bank that draws a set torque, endless steam and water sources, a
//! drain that takes anything, and a firebox that needs no fuel. Each has a
//! screen with its settings (− and + buttons, shift-click for big steps)
//! and what it is doing now.
//!
//! Settings live in the block entity's saved data (`c_*` keys); the menu's
//! data is [setting 0, setting 1, status 0, status 1], whole numbers.

use mod_api::{
    BlockEntityTickEvent, BlockEntityTickReply, ClientHost, ModContainer, ScreenClickEvent,
    ScreenDrawEvent, ScreenEvent, ServerHost,
};

use super::blocks::{self, Block};
use super::server::with_level;
use crate::sim::fluid::Content;
use crate::sim::geom::Pos;
use crate::sim::rot::part::{LOAD_BANK_MAX, MOTOR_MAX};

pub const DATA_SLOTS: usize = 4;

pub const DEFAULT_RPM: f64 = 64.0;
pub const DEFAULT_LOAD: f64 = 16.0;
pub const DEFAULT_STEAM_RATE: f64 = 200.0;
pub const DEFAULT_BAR: f64 = 4.0;
pub const DEFAULT_WATER_RATE: f64 = 100.0;
pub const DEFAULT_KW: f64 = 1000.0;
const MAX_RATE: f64 = 4000.0;
const MAX_BAR: f64 = 16.0;
const MAX_KW: f64 = 10_000.0;

/// One hidden slot that no face reaches (menus need one).
pub fn container(block: Block) -> ModContainer {
    ModContainer {
        slots: 1,
        faces: [vec![], vec![], vec![], vec![], vec![], vec![]],
        inputs: vec![],
        outputs: vec![],
        menu: Some(format!("fras:{}", block.name())),
    }
}

/// A setting row: its translation key, saved key, default, small and big
/// step, and range. A step of 0 makes it a switch.
struct Setting {
    label: &'static str,
    key: &'static str,
    default: f64,
    step: f64,
    big: f64,
    max: f64,
}

fn settings(block: Block) -> &'static [Setting] {
    const ENGINE: &[Setting] = &[
        Setting {
            label: "fras.creative.speed",
            key: "c_rpm",
            default: DEFAULT_RPM,
            step: 2.0,
            big: 10.0,
            max: MOTOR_MAX,
        },
        Setting {
            label: "fras.creative.direction",
            key: "c_rev",
            default: 0.0,
            step: 0.0,
            big: 0.0,
            max: 1.0,
        },
    ];
    const LOAD: &[Setting] = &[Setting {
        label: "fras.creative.load",
        key: "c_load",
        default: DEFAULT_LOAD,
        step: 1.0,
        big: 10.0,
        max: LOAD_BANK_MAX,
    }];
    const STEAM: &[Setting] = &[
        Setting {
            label: "fras.creative.rate",
            key: "c_rate",
            default: DEFAULT_STEAM_RATE,
            step: 10.0,
            big: 100.0,
            max: MAX_RATE,
        },
        Setting {
            label: "fras.creative.pressure",
            key: "c_bar",
            default: DEFAULT_BAR,
            step: 0.5,
            big: 2.0,
            max: MAX_BAR,
        },
    ];
    const WATER: &[Setting] = &[
        Setting {
            label: "fras.creative.rate",
            key: "c_rate",
            default: DEFAULT_WATER_RATE,
            step: 10.0,
            big: 100.0,
            max: MAX_RATE,
        },
        Setting {
            label: "fras.creative.fluid",
            key: "c_hot",
            default: 0.0,
            step: 0.0,
            big: 0.0,
            max: 1.0,
        },
    ];
    const FIRE: &[Setting] = &[
        Setting {
            label: "fras.creative.heat",
            key: "c_kw",
            default: DEFAULT_KW,
            step: 50.0,
            big: 250.0,
            max: MAX_KW,
        },
        Setting {
            label: "fras.creative.dampers",
            key: "c_free",
            default: 0.0,
            step: 0.0,
            big: 0.0,
            max: 1.0,
        },
    ];
    match block {
        Block::CreativeEngine => ENGINE,
        Block::LoadBank => LOAD,
        Block::CreativeSteam => STEAM,
        Block::CreativeWater => WATER,
        Block::CreativeFirebox => FIRE,
        _ => &[],
    }
}

/// A setting's value from saved data (switches are saved as booleans).
fn read(nbt: &mod_api::Compound, s: &Setting) -> f64 {
    if s.step == 0.0 {
        blocks::get_bool(nbt, s.key) as i32 as f64
    } else {
        blocks::get_f64(nbt, s.key).unwrap_or(s.default)
    }
}

/// The next value of a setting after a click.
fn next(s: &Setting, v: f64, up: bool, big: bool) -> f64 {
    if s.step == 0.0 {
        return if v > 0.5 { 0.0 } else { 1.0 };
    }
    let step = if big { s.big } else { s.step };
    // Back onto the step's grid first, so shift-clicks land on round numbers.
    let snapped = (v / step).round() * step;
    let moved = if (snapped - v).abs() > 1e-9 && (snapped > v) == up {
        snapped
    } else if up {
        snapped + step
    } else {
        snapped - step
    };
    moved.clamp(0.0, s.max)
}

/// Fixed point for the menu's settings (pressure in tenths).
fn to_data(s: &Setting, v: f64) -> i32 {
    if s.key == "c_bar" {
        (v * 10.0).round() as i32
    } else {
        v.round() as i32
    }
}

fn short(v: f64) -> i32 {
    v.round().clamp(-32767.0, 32767.0) as i32
}

/// The block entity's tick: tell the networks the settings, keep the menu's
/// data.
pub fn tick(block: Block, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let dim = host.dimension();
    let pos = Pos::from_array(event.pos);
    let nbt = &event.entity.nbt;
    let rows = settings(block);
    let value = |i: usize| rows.get(i).map_or(0.0, |s| read(nbt, s));
    let (a, b) = (value(0), value(1));
    let status: [i32; 2] = match block {
        Block::CreativeEngine => {
            let target = if b > 0.5 { -a } else { a };
            with_level(dim, |l| {
                l.motors.insert(pos, target);
                let speed = l.mech.speed(pos).map_or(0.0, |s| s.0);
                let torque = l.mech.drive_torque(pos).unwrap_or(0.0);
                // Torque in the direction it turns: positive while it drives.
                let torque = if speed.abs() > 1e-6 {
                    torque * speed.signum()
                } else {
                    torque.abs()
                };
                [short(speed * 10.0), short(torque)]
            })
        }
        Block::LoadBank => with_level(dim, |l| {
            l.loads.insert(pos, a);
            let speed = l.mech.speed(pos).map_or(0.0, |s| s.0);
            [short(speed * 10.0), 0]
        }),
        Block::CreativeSteam | Block::CreativeWater => with_level(dim, |l| {
            let w = &l.steam.works;
            [short(w.source_gave(pos)), short(w.gauge(pos).bar * 100.0)]
        }),
        Block::CreativeDrain => with_level(dim, |l| match l.steam.works.void_flow(pos) {
            Some((c, f)) if f > 1e-6 => [short(f), content_code(c)],
            _ => [0, 0],
        }),
        Block::CreativeFirebox => {
            super::steam::set_heat(dim, pos, a);
            with_level(dim, |l| {
                let w = &l.steam.works;
                let heat = w.fire(pos).map_or(0.0, |f| f.heat());
                [short(heat), w.firebox_boilers(pos) as i32]
            })
        }
        _ => [0, 0],
    };
    let mut data = vec![0; DATA_SLOTS];
    for (i, s) in rows.iter().enumerate().take(2) {
        data[i] = to_data(s, value(i));
    }
    data[2] = status[0];
    data[3] = status[1];
    if event.entity.data != data {
        event.entity.data = data;
    } else {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

fn content_code(c: Content) -> i32 {
    match c {
        Content::Water => 1,
        Content::HotWater => 2,
        Content::Exhaust => 3,
        Content::Steam => 4,
    }
}

fn content_key(code: i32) -> &'static str {
    match code {
        1 => "fras.content.water",
        2 => "fras.content.hot_water",
        3 => "fras.content.exhaust",
        4 => "fras.content.steam",
        _ => "fras.content.empty",
    }
}

/// A screen with its one slot hidden and no inventory below.
pub fn screen(_: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: 100.0,
        slots: vec![None],
        inventory_y: 1000.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

const BUTTON: f32 = 12.0;
const MINUS_X: f32 = 96.0;
const PLUS_X: f32 = 150.0;

fn row_y(row: usize) -> f32 {
    18.0 + 16.0 * row as f32
}

fn tr(host: &dyn ClientHost, key: &str, args: &[String]) -> String {
    let mut t = host.translate(key).unwrap_or_else(|| key.to_string());
    for (i, a) in args.iter().enumerate() {
        t = t.replacen("%s", a, 1).replace(&format!("%{}$s", i + 1), a);
    }
    t
}

/// Watts from a torque (Nm) and a speed (RPM), for players.
fn power(torque: f64, rpm: f64) -> String {
    let w = (torque * rpm * std::f64::consts::TAU / 60.0).abs();
    if w >= 1000.0 {
        format!("{} kW", super::num(w / 1000.0))
    } else {
        format!("{} W", w.round())
    }
}

pub fn screen_draw(block: Block, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let grey = 0xFF40_4040;
    let rows = settings(block);
    for (r, s) in rows.iter().enumerate() {
        let ry = y + row_y(r);
        draw.text(&tr(host, s.label, &[]), [x + 8.0, ry + 2.0], grey, false);
        let v = d(r);
        let shown = match s.key {
            "c_rpm" => format!("{v} RPM"),
            "c_load" => format!("{v} Nm"),
            "c_rate" => format!("{v} mB/s"),
            "c_bar" => format!("{} bar", super::num(v as f64 / 10.0)),
            "c_kw" => format!("{v} kW"),
            "c_rev" => tr(
                host,
                if v != 0 {
                    "fras.creative.reverse"
                } else {
                    "fras.creative.forward"
                },
                &[],
            ),
            "c_hot" => tr(host, content_key(if v != 0 { 2 } else { 1 }), &[]),
            "c_free" => tr(
                host,
                if v != 0 {
                    "fras.creative.ignored"
                } else {
                    "fras.creative.obeyed"
                },
                &[],
            ),
            _ => v.to_string(),
        };
        draw.text(&shown, [x + MINUS_X + 16.0, ry + 2.0], grey, false);
        for (bx, label) in [(MINUS_X, "-"), (PLUS_X, "+")] {
            draw.fill([x + bx, ry, BUTTON, BUTTON], 0xFF8B_8B8B);
            draw.fill(
                [x + bx + 1.0, ry + 1.0, BUTTON - 2.0, BUTTON - 2.0],
                0xFFC6_C6C6,
            );
            draw.text(label, [x + bx + 3.5, ry + 2.0], grey, false);
        }
    }
    // What it is doing now: one value per line, the label on the left and
    // the value right-aligned, so a changing number never moves the others.
    let (s0, s1) = (d(2), d(3));
    let rpm = s0 as f64 / 10.0;
    let speed = format!("{} RPM", super::num(rpm));
    let lines: Vec<(&str, String)> = match block {
        Block::CreativeEngine => vec![
            ("fras.creative.now.speed", speed),
            ("fras.creative.now.torque", format!("{s1} Nm")),
            ("fras.creative.now.power", power(s1 as f64, rpm)),
        ],
        Block::LoadBank => vec![
            ("fras.creative.now.speed", speed),
            ("fras.creative.now.power", power(d(0) as f64, rpm)),
        ],
        Block::CreativeSteam => vec![
            ("fras.creative.now.giving", format!("{s0} mB/s")),
            (
                "fras.creative.now.network",
                format!("{} bar", super::num(s1 as f64 / 100.0)),
            ),
        ],
        Block::CreativeWater => vec![("fras.creative.now.giving", format!("{s0} mB/s"))],
        Block::CreativeDrain => vec![
            ("fras.creative.now.taking", format!("{s0} mB/s")),
            ("fras.creative.now.fluid", tr(host, content_key(s1), &[])),
        ],
        Block::CreativeFirebox => vec![
            ("fras.creative.now.heat", format!("{s0} kW")),
            ("fras.creative.now.boilers", s1.to_string()),
        ],
        _ => Vec::new(),
    };
    let top = y + row_y(rows.len()) + 6.0;
    for (i, (label, value)) in lines.iter().enumerate() {
        let ly = top + i as f32 * 11.0;
        draw.text(&tr(host, label, &[]), [x + 8.0, ly], grey, false);
        let w = draw.text_width(value);
        draw.text(value, [x + 168.0 - w, ly], grey, false);
    }
}

pub fn screen_click(
    block: Block,
    host: &dyn ClientHost,
    event: &mut ScreenClickEvent<'_>,
) -> Option<i32> {
    let shift = host.modifiers().bits & 0x0001 != 0;
    let [ax, ay] = event.at;
    (0..settings(block).len()).find_map(|r| {
        let ry = row_y(r);
        if ay < ry || ay >= ry + BUTTON {
            return None;
        }
        let up = if (MINUS_X..MINUS_X + BUTTON).contains(&ax) {
            false
        } else if (PLUS_X..PLUS_X + BUTTON).contains(&ax) {
            true
        } else {
            return None;
        };
        // Button ids: 2 per row (− then +), plus 10 for a big step.
        Some(r as i32 * 2 + up as i32 + if shift { 10 } else { 0 })
    })
}

/// A button pressed: step the setting; plant parts are read again.
pub fn menu_button(
    block: Block,
    host: &mut dyn ServerHost,
    event: &mut mod_api::MenuButtonEvent<'_>,
) {
    let id = event.context.id;
    let (big, id) = (id >= 10, id % 10);
    let rows = settings(block);
    let Some(s) = rows.get((id / 2) as usize) else {
        return;
    };
    let nbt = &mut event.entity.nbt;
    let v = next(s, read(nbt, s), id % 2 == 1, big);
    if s.step == 0.0 {
        blocks::put_bool(nbt, s.key, v > 0.5);
    } else {
        blocks::put_f64(nbt, s.key, v);
    }
    if block.is_plant() {
        let pos = Pos::from_array(event.context.pos);
        let dim = host
            .player(event.context.player)
            .and_then(|p| p.level(&*host))
            .and_then(|level| super::server::dim_of(&*host, level))
            .unwrap_or_else(|| host.dimension());
        // Read again after the block entity is written back (as governors).
        with_level(dim, |l| {
            l.steam.settings_dirty.insert(pos);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_land_on_round_numbers() {
        let s = &settings(Block::CreativeEngine)[0];
        assert_eq!(next(s, 64.0, true, false), 66.0);
        assert_eq!(next(s, 64.0, true, true), 70.0);
        assert_eq!(next(s, 66.0, true, true), 70.0);
        assert_eq!(next(s, 66.0, false, true), 60.0);
        assert_eq!(next(s, 0.0, false, false), 0.0);
        assert_eq!(next(s, 254.0, true, true), MOTOR_MAX);
        let bar = &settings(Block::CreativeSteam)[1];
        assert_eq!(next(bar, 4.0, true, false), 4.5);
        let rev = &settings(Block::CreativeEngine)[1];
        assert_eq!(next(rev, 0.0, false, false), 1.0);
        assert_eq!(next(rev, 1.0, true, false), 0.0);
    }

    #[test]
    fn every_creative_block_has_its_settings() {
        for b in Block::ALL.into_iter().filter(|b| b.is_creative()) {
            let n = settings(b).len();
            assert!(n <= 2, "{b:?}");
            assert_eq!(n == 0, b == Block::CreativeDrain, "{b:?}");
        }
    }
}
