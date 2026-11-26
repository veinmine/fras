//! The readout screen of the tachometer, pressure gauge and sight glass:
//! right-click one (with anything but the wrench) for the number it shows
//! and a chart of the last 1, 5 or 10 minutes (buttons on the screen).
//!
//! Each block entity keeps its history in the server's memory (not saved):
//! 120 samples per range, one every half second, 2.5 s or 5 s (the reading
//! at that moment). The range shown is saved with the block (`range`). The
//! menu's data is [value, red line, extra, head, range, sample 0 … 119]: a
//! ring, so a new sample changes two slots, not all of them. Values are
//! fixed point (see `Readout::scale`) and stay within a short, as vanilla
//! syncs menu data.

use mod_api::{ClientHost, ModContainer, ScreenClickEvent, ScreenDrawEvent, ScreenEvent};

use super::blocks::Block;
use super::server::with_level;
use crate::sim::geom::Pos;

/// Samples kept per range.
pub const SAMPLES: usize = 120;
/// The ranges: minutes shown, ticks between two samples, and the minutes
/// between two upright lines.
pub const RANGES: [(u32, u64, u32); 3] = [(1, 10, 0), (5, 50, 1), (10, 100, 2)];
/// Slots before the samples.
const HEAD: usize = 5;
pub const DATA_SLOTS: usize = HEAD + SAMPLES;
/// A sample slot not filled yet.
const NONE: i32 = -32768;

/// One gauge's samples, one ring per range.
#[derive(Clone, Debug)]
pub struct History {
    pub rings: [Ring; 3],
}

impl Default for History {
    fn default() -> Self {
        History {
            rings: std::array::from_fn(|_| Ring::default()),
        }
    }
}

impl History {
    /// Sample `v` into every range due at `ticks`.
    pub fn sample(&mut self, ticks: u64, v: i32) {
        for (ring, (_, every, _)) in self.rings.iter_mut().zip(RANGES) {
            if ticks.is_multiple_of(every) {
                ring.push(v);
            }
        }
    }
}

/// One range's samples.
#[derive(Clone, Debug)]
pub struct Ring {
    pub samples: Vec<i32>,
    /// Where the next sample goes.
    pub head: usize,
}

impl Default for Ring {
    fn default() -> Self {
        Ring {
            samples: vec![NONE; SAMPLES],
            head: 0,
        }
    }
}

impl Ring {
    pub fn push(&mut self, v: i32) {
        self.samples[self.head] = v;
        self.head = (self.head + 1) % SAMPLES;
    }

    /// The samples oldest first, filled ones only (with their place: 0 is
    /// the oldest).
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
#[allow(clippy::too_many_arguments)]
pub fn update(
    dim: super::server::Dim,
    pos: Pos,
    kind: Readout,
    value: f64,
    red: f64,
    extra: f64,
    range: usize,
    data: &mut Vec<i32>,
) -> bool {
    let s = kind.scale();
    let v = fixed(value, s);
    let range = range.min(RANGES.len() - 1);
    let ticks = super::server::with(|s| s.ticks);
    let (samples, head) = with_level(dim, |l| {
        let h = l.histories.entry(pos).or_default();
        h.sample(ticks, v);
        let ring = &h.rings[range];
        (ring.samples.clone(), ring.head)
    });
    let extra = if kind == Readout::Level {
        extra.round() as i32
    } else {
        fixed(extra, s)
    };
    let mut next = Vec::with_capacity(DATA_SLOTS);
    next.extend([v, fixed(red, s), extra, head as i32, range as i32]);
    next.extend(samples);
    if *data != next {
        *data = next;
        true
    } else {
        false
    }
}

/// The range a readout's block saved.
pub fn range(nbt: &mod_api::Compound) -> usize {
    super::blocks::get_i32(nbt, "range")
        .unwrap_or(0)
        .clamp(0, RANGES.len() as i32 - 1) as usize
}

/// The range buttons: (x, width) from the screen's left, in the title row.
const BUTTONS: [(f32, f32); 3] = [(108.0, 18.0), (128.0, 18.0), (148.0, 20.0)];
const BUTTON_Y: f32 = 4.0;
const BUTTON_H: f32 = 11.0;

pub fn screen_click(event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    let [ax, ay] = event.at;
    if ay < BUTTON_Y || ay >= BUTTON_Y + BUTTON_H {
        return None;
    }
    BUTTONS
        .iter()
        .position(|(bx, w)| ax >= *bx && ax < bx + w)
        .map(|i| i as i32)
}

/// A range button pressed: show that range.
pub fn menu_button(event: &mut mod_api::MenuButtonEvent<'_>) {
    let id = event.context.id;
    if (0..RANGES.len() as i32).contains(&id) {
        super::blocks::put_i32(&mut event.entity.nbt, "range", id);
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
/// The area under the line: the line's green, translucent.
const SHADE: u32 = 0x4060_E060;

pub fn screen_draw(block: Block, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let Some(kind) = Readout::of(block) else {
        return;
    };
    let d = |i: usize| event.data.get(i).copied().unwrap_or(0);
    let (value, red, extra, head) = (d(0), d(1), d(2), d(3).max(0) as usize);
    let range = (d(4).max(0) as usize).min(RANGES.len() - 1);
    let samples: Vec<i32> = (0..SAMPLES).map(|i| d(HEAD + i)).collect();
    let points = Ring::ordered(&samples, head % SAMPLES);
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    // The range buttons, the one shown pressed.
    for (i, (bx, w)) in BUTTONS.iter().enumerate() {
        let (bx, by) = (x + bx, y + BUTTON_Y);
        let on = i == range;
        draw.fill([bx, by, *w, BUTTON_H], 0xFF8B_8B8B);
        draw.fill(
            [bx + 1.0, by + 1.0, w - 2.0, BUTTON_H - 2.0],
            if on { 0xFF60_6060 } else { 0xFFC6_C6C6 },
        );
        let text = format!("{}m", RANGES[i].0);
        let tw = draw.text_width(&text);
        let color = if on { 0xFFFF_FFFF } else { GREY };
        draw.text(&text, [bx + (w - tw) / 2.0, by + 2.0], color, false);
    }
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
    // Faint upright lines: every 15 s over a minute, every 1 or 2 minutes
    // over 5 or 10.
    let (minutes, _, every) = RANGES[range];
    let parts = if every == 0 { 4 } else { minutes / every };
    for q in 1..parts {
        let gx = cx + (cw * q as f32 / parts as f32).round();
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
    // The samples: one column per pixel, the area under the line shaded
    // translucent and a 1-pixel line on top, inside the frame.
    let mut at_sample: Vec<Option<i32>> = vec![None; SAMPLES];
    for (i, v) in &points {
        at_sample[*i] = Some(*v);
    }
    let columns = cw.floor().max(2.0) as usize;
    let mut last: Option<f32> = None;
    for c in 0..columns {
        let t = c as f32 * (SAMPLES - 1) as f32 / (columns - 1) as f32;
        let (i0, i1) = (t.floor() as usize, (t.ceil() as usize).min(SAMPLES - 1));
        let (Some(a), Some(b)) = (at_sample[i0], at_sample[i1]) else {
            last = None;
            continue;
        };
        let f = t - i0 as f32;
        let py = (at(a) * (1.0 - f) + at(b) * f).round().min(cy + ch - 1.0);
        let px = cx + c as f32;
        draw.fill([px, py + 1.0, 1.0, (cy + ch - py - 1.0).max(0.0)], SHADE);
        let (lo, hi) = match last {
            Some(ly) => (ly.min(py), ly.max(py)),
            None => (py, py),
        };
        draw.fill([px, lo, 1.0, hi - lo + 1.0], LINE);
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
    let ago = tr(host, "fras.readout.ago", &[minutes.to_string()]);
    draw.text(&ago, [cx, below], GREY, false);
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
        let mut h = Ring::default();
        for v in 0..(SAMPLES as i32 + 5) {
            h.push(v);
        }
        let o = Ring::ordered(&h.samples, h.head);
        assert_eq!(o.len(), SAMPLES);
        assert_eq!(o[0], (0, 5));
        assert_eq!(o[SAMPLES - 1], (SAMPLES - 1, SAMPLES as i32 + 4));
        let mut h = Ring::default();
        h.push(7);
        let o = Ring::ordered(&h.samples, h.head);
        assert_eq!(o, vec![(SAMPLES - 1, 7)]);
    }

    #[test]
    fn each_range_samples_at_its_pace() {
        let mut h = History::default();
        for t in 1..=100u64 {
            h.sample(t, t as i32);
        }
        let n = |r: &Ring| r.samples.iter().filter(|v| **v != NONE).count();
        assert_eq!(n(&h.rings[0]), 10);
        assert_eq!(n(&h.rings[1]), 2);
        assert_eq!(n(&h.rings[2]), 1);
        // Every range spans its minutes with its 120 samples.
        for (minutes, every, _) in RANGES {
            assert_eq!(SAMPLES as u64 * every, minutes as u64 * 60 * 20);
        }
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
