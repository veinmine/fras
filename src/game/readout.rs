//! The readout screen of the tachometer, pressure gauge and sight glass:
//! right-click one (with anything but the wrench) for the number it shows
//! and a chart of the last minute.
//!
//! Each block entity keeps its history in the server's memory (not saved):
//! one sample every half second, 120 of them. The menu's data is
//! [value, red line, extra, head, sample 0 … sample 119]: a ring, so a new
//! sample changes two slots, not all of them. Values are fixed point (see
//! `Readout::scale`) and stay within a short, as vanilla syncs menu data.

use mod_api::{ClientHost, ModContainer, ScreenDrawEvent, ScreenEvent};

use super::blocks::Block;
use super::server::with_level;
use crate::sim::geom::Pos;

/// Samples kept and the ticks between two.
pub const SAMPLES: usize = 120;
pub const EVERY: u64 = 10;
/// Slots before the samples.
const HEAD: usize = 4;
pub const DATA_SLOTS: usize = HEAD + SAMPLES;
/// A sample slot not filled yet.
const NONE: i32 = -32768;

/// One gauge's samples.
#[derive(Clone, Debug)]
pub struct History {
    pub samples: Vec<i32>,
    /// Where the next sample goes.
    pub head: usize,
}

impl Default for History {
    fn default() -> Self {
        History {
            samples: vec![NONE; SAMPLES],
            head: 0,
        }
    }
}

impl History {
    pub fn push(&mut self, v: i32) {
        self.samples[self.head] = v;
        self.head = (self.head + 1) % SAMPLES;
    }

    /// The samples oldest first, filled ones only (with their place: 0 is a
    /// minute ago).
    pub fn ordered(samples: &[i32], head: usize) -> Vec<(usize, i32)> {
        (0..SAMPLES)
            .filter_map(|i| {
                let v = *samples.get((head + i) % SAMPLES)?;
                (v != NONE).then_some((i, v))
            })
            .collect()
    }
}

/// What each readout shows and in what units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Readout {
    Speed,
    Pressure,
    Level,
}

impl Readout {
    pub fn of(block: Block) -> Option<Readout> {
        match block {
            Block::Tachometer => Some(Readout::Speed),
            Block::PressureGauge => Some(Readout::Pressure),
            Block::SightGlass => Some(Readout::Level),
            _ => None,
        }
    }

    /// Fixed point: RPM × 10, bar × 100, level in ‰.
    pub fn scale(self) -> f64 {
        match self {
            Readout::Speed => 10.0,
            Readout::Pressure => 100.0,
            Readout::Level => 1000.0,
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Readout::Speed => "RPM",
            Readout::Pressure => "bar",
            Readout::Level => "%",
        }
    }

    /// The value as players read it.
    fn show(self, v: i32) -> String {
        match self {
            Readout::Level => format!("{}%", super::num(v as f64 / 10.0)),
            _ => format!("{} {}", super::num(v as f64 / self.scale()), self.unit()),
        }
    }

    /// The least the chart's scale goes to (fixed point).
    fn least_top(self) -> i32 {
        match self {
            Readout::Speed => 160,
            Readout::Pressure => 100,
            Readout::Level => 1000,
        }
    }
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

fn fixed(v: f64, scale: f64) -> i32 {
    (v * scale).round().clamp(-32767.0, 32767.0) as i32
}

/// Keep a gauge's history and menu data: `value`, `red` and `extra` (the
/// tachometer's yellow line, the sight glass's warning) in its units.
/// Returns whether the data changed.
pub fn update(
    dim: super::server::Dim,
    pos: Pos,
    kind: Readout,
    value: f64,
    red: f64,
    extra: f64,
    data: &mut Vec<i32>,
) -> bool {
    let s = kind.scale();
    let v = fixed(value, s);
    let ticks = super::server::with(|s| s.ticks);
    let (samples, head) = with_level(dim, |l| {
        let h = l.histories.entry(pos).or_default();
        if ticks.is_multiple_of(EVERY) {
            h.push(v);
        }
        (h.samples.clone(), h.head)
    });
    let extra = if kind == Readout::Level {
        extra.round() as i32
    } else {
        fixed(extra, s)
    };
    let mut next = Vec::with_capacity(DATA_SLOTS);
    next.extend([v, fixed(red, s), extra, head as i32]);
    next.extend(samples);
    if *data != next {
        *data = next;
        true
    } else {
        false
    }
}

/// The screen: no slots, no inventory.
pub fn screen(_: &mut ScreenEvent<'_>) -> Option<mod_api::Screen> {
    Some(mod_api::Screen {
        width: 176.0,
        height: 124.0,
        slots: vec![None],
        inventory_y: 1000.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        recipe_book: None,
        recipe_book_toggle: None,
    })
}

fn tr(host: &dyn ClientHost, key: &str, args: &[String]) -> String {
    let mut t = host.translate(key).unwrap_or_else(|| key.to_string());
    for (i, a) in args.iter().enumerate() {
        t = t.replacen("%s", a, 1).replace(&format!("%{}$s", i + 1), a);
    }
    t
}

const GREY: u32 = 0xFF40_4040;
const RED: u32 = 0xFFD0_3030;
const YELLOW: u32 = 0xFFD0_B020;
const LINE: u32 = 0xFF60_E060;

pub fn screen_draw(block: Block, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let Some(kind) = Readout::of(block) else {
        return;
    };
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (value, red, extra, head) = (d(0), d(1), d(2), d(3).max(0) as usize);
    let samples: Vec<i32> = (0..SAMPLES).map(|i| d(HEAD + i)).collect();
    let points = History::ordered(&samples, head % SAMPLES);
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    // The reading, and its limits.
    draw.text(&kind.show(value), [x + 8.0, y + 18.0], GREY, false);
    let note = match kind {
        Readout::Level if extra >= 2 => tr(host, "fras.readout.very_low", &[]),
        Readout::Level if extra == 1 => tr(host, "fras.readout.low", &[]),
        Readout::Level => String::new(),
        _ if red > 0 => tr(host, "fras.readout.red_line", &[kind.show(red)]),
        _ => String::new(),
    };
    if !note.is_empty() {
        let w = draw.text_width(&note);
        let color = if kind == Readout::Level { RED } else { GREY };
        draw.text(&note, [x + 168.0 - w, y + 18.0], color, false);
    }
    // The chart: the last minute, scaled to round numbers that hold the
    // samples and the red line, with a labelled line at each step.
    let most = points.iter().map(|(_, v)| v.abs()).max().unwrap_or(0);
    let (top, step) = if kind == Readout::Level {
        (1000, 250)
    } else {
        let want = most.max(red).max(value.abs()).max(kind.least_top()) as f64 * 1.1;
        axis(want)
    };
    let label = |v: i32| match kind {
        Readout::Level => format!("{}", v / 10),
        _ => super::num(v as f64 / kind.scale()),
    };
    let ticks: Vec<i32> = (0..=top / step).map(|i| i * step).collect();
    let gutter = ticks
        .iter()
        .map(|t| draw.text_width(&label(*t)))
        .fold(0.0f32, f32::max)
        + 4.0;
    let (cx, cy, cw, ch) = (x + 8.0 + gutter, y + 32.0, 160.0 - gutter, 72.0);
    draw.fill([cx - 1.0, cy - 1.0, cw + 2.0, ch + 2.0], 0xFF8B_8B8B);
    draw.fill([cx, cy, cw, ch], 0xFF20_2020);
    let at = |v: i32| cy + ch - (v.abs() as f32 / top as f32).clamp(0.0, 1.0) * ch;
    // Every 15 s, a faint upright line.
    for q in 1..4 {
        let gx = cx + (cw * q as f32 / 4.0).round();
        draw.fill([gx, cy, 1.0, ch], 0xFF2C_2C2C);
    }
    for t in &ticks {
        let gy = at(*t).min(cy + ch - 1.0);
        if *t > 0 && *t < top {
            draw.fill([cx, gy, cw, 1.0], 0xFF38_3838);
        }
        let text = label(*t);
        let w = draw.text_width(&text);
        draw.text(&text, [cx - 3.0 - w, gy - 3.5], GREY, false);
    }
    // The samples: a 2-pixel line with the area under it shaded, kept
    // inside the frame.
    let width = 2.0f32;
    let step_x = (cw - width) / (SAMPLES - 1) as f32;
    let mut last: Option<f32> = None;
    for (i, v) in &points {
        let px = cx + *i as f32 * step_x;
        let py = at(*v).min(cy + ch - width);
        let w = (step_x + 0.5).max(width).min(cx + cw - px);
        draw.fill([px, py, w, cy + ch - py], 0xFF24_4A2A);
        let (lo, hi) = match last {
            Some(ly) => (ly.min(py), ly.max(py)),
            None => (py, py),
        };
        draw.fill([px, lo, w, hi - lo + width], LINE);
        last = Some(py);
    }
    // Limits over the samples, so they stay visible.
    if kind != Readout::Level {
        if red > 0 && red <= top {
            draw.fill([cx, at(red), cw, 1.0], RED);
        }
        if kind == Readout::Speed && extra > 0 && extra <= top {
            draw.fill([cx, at(extra), cw, 1.0], YELLOW);
        }
    }
    // Time.
    let below = cy + ch + 3.0;
    draw.text(
        &tr(host, "fras.readout.minute_ago", &[]),
        [cx, below],
        GREY,
        false,
    );
    let half = tr(host, "fras.readout.half_minute_ago", &[]);
    let w = draw.text_width(&half);
    draw.text(&half, [cx + cw / 2.0 - w / 2.0, below], GREY, false);
    let now = tr(host, "fras.readout.now", &[]);
    let w = draw.text_width(&now);
    draw.text(&now, [cx + cw - w, below], GREY, false);
}

/// A chart scale for values up to `v` (fixed point): a round step (1, 2 or
/// 5 times a power of ten) giving at most 5 steps, and the top, a whole
/// number of steps.
fn axis(v: f64) -> (i32, i32) {
    let v = v.max(1.0);
    let raw = v / 5.0;
    let p = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * p)
        .find(|t| *t >= raw - 1e-9)
        .unwrap_or(10.0 * p)
        .max(1.0);
    let top = (v / step).ceil() * step;
    (top.min(32767.0) as i32, step as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_is_a_ring_read_oldest_first() {
        let mut h = History::default();
        for v in 0..(SAMPLES as i32 + 5) {
            h.push(v);
        }
        let o = History::ordered(&h.samples, h.head);
        assert_eq!(o.len(), SAMPLES);
        assert_eq!(o[0], (0, 5));
        assert_eq!(o[SAMPLES - 1], (SAMPLES - 1, SAMPLES as i32 + 4));
        let mut h = History::default();
        h.push(7);
        let o = History::ordered(&h.samples, h.head);
        assert_eq!(o, vec![(SAMPLES - 1, 7)]);
    }

    #[test]
    fn scales_are_round() {
        // 85 RPM under a 192 RPM red line: 0 to 250 RPM in 50s.
        assert_eq!(axis(1920.0 * 1.1), (2500, 500));
        // 4 bar: 0 to 5 in 1s.
        assert_eq!(axis(440.0), (500, 100));
        // The least scale (16 RPM): steps of 5.
        assert_eq!(axis(176.0), (200, 50));
        for v in [37.0, 999.0, 1234.0, 30000.0] {
            let (top, step) = axis(v);
            assert!(top as f64 >= v.min(32767.0) && top % step == 0);
            assert!((2..=5).contains(&(top / step)), "{v}: {top} {step}");
        }
    }
}
