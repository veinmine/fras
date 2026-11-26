//! Belts, bucket elevators and diverters, free of the game (logistics spec):
//! a line's shape (blocks, slopes, drives, length), the items riding it, the
//! per-tick advance with spacing, stations (machines working on the belt,
//! diverters, counters) and a blocked or open end, insertion with the room
//! rule, load and drive count, saving per block and the client's
//! extrapolation (the same arithmetic the server steps with).
//!
//! A line is generic over its item type `T`: the game keeps item stacks, the
//! tests strings. Positions along a line are in blocks: 0 at the start's
//! back edge, `len` at the end's front edge; block `i` spans `[i, i + 1)` and
//! its centre is `i + 0.5`.

use crate::sim::geom::{Axis, Dir, Pos};

/// Items keep at least this far apart, blocks (logistics 4.1).
pub const SPACING: f64 = 0.5;
/// The longest line, blocks (3.1).
pub const MAX_LEN: usize = 64;
/// One drive per this many blocks (3.3).
pub const BLOCKS_PER_DRIVE: usize = 16;
/// Torque a belt carries between its drives, Nm (3.3).
pub const TORQUE_LIMIT: f64 = 24.0;
/// The belt's top speed, RPM (3.4).
pub const MAX_RPM: f64 = 128.0;
/// Load: per block of belt, per item on a flat block, per item going up a slope (3.4).
pub const LOAD_PER_BLOCK: f64 = 0.125;
pub const LOAD_PER_ITEM: f64 = 0.125;
pub const LOAD_PER_ITEM_RISING: f64 = 0.5;
/// Inertia and friction per block, as a shaft (3.4).
pub const INERTIA_PER_BLOCK: f64 = 0.25;
pub const FRICTION_PER_BLOCK: f64 = 0.0025;
/// A refused item at the end is offered again this often, ticks (5.1).
pub const RETRY_TICKS: u32 = 4;
/// Insertion looks for room within this distance of the block's centre (4.2).
pub const ROOM: f64 = 0.25;
/// Elevator: heights, load per item lifted, top speed (8).
pub const ELEVATOR_MIN: usize = 2;
pub const ELEVATOR_MAX: usize = 32;
pub const ELEVATOR_LOAD_PER_ITEM: f64 = 0.5;
pub const ELEVATOR_MAX_RPM: f64 = 96.0;
/// Diverter comparator pulse, ticks (9).
pub const DIVERTER_PULSE: u32 = 2;
/// Positions match within this.
const EPS: f64 = 1e-9;

/// Belt speed in blocks per second at a drive speed (3.4): `|rpm| / 32`.
pub fn blocks_per_second(rpm: f64) -> f64 {
    rpm.abs() / 32.0
}

/// Signed belt speed in blocks per tick: positive runs toward the end.
pub fn blocks_per_tick(rpm: f64) -> f64 {
    rpm / 32.0 / 20.0
}

/// Drives a line of `len` blocks needs (3.3).
pub fn drives_needed(len: usize) -> usize {
    len.div_ceil(BLOCKS_PER_DRIVE).max(1)
}

/// Which way positive rotation of a drive on `axis` moves a belt's top
/// surface: the right-hand rule, `axis × up` (3.2).
pub fn surface_dir(axis: Axis) -> Option<Dir> {
    match axis {
        // x × y = z; z × y = -x.
        Axis::X => Some(Dir::South),
        Axis::Z => Some(Dir::West),
        Axis::Y => None,
    }
}

/// The sign that turns a drive's speed about its positive axis into the
/// line's forward speed: +1 when positive rotation moves the top along the
/// heading.
pub fn drive_sign(heading: Dir) -> f64 {
    let axis = drive_axis(heading);
    if surface_dir(axis) == Some(heading) {
        1.0
    } else {
        -1.0
    }
}

/// The axis a drive of a belt heading `heading` turns about: horizontal,
/// across the heading.
pub fn drive_axis(heading: Dir) -> Axis {
    match heading.axis() {
        Axis::X => Axis::Z,
        _ => Axis::X,
    }
}

/// The shape of one belt block.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slope {
    Flat,
    /// Climbs one block over its length, toward the end (the next belt is higher).
    Rising,
    /// Drops one block over its length, toward the end (the previous belt is higher).
    Falling,
}

impl Slope {
    pub fn name(self) -> &'static str {
        match self {
            Slope::Flat => "flat",
            Slope::Rising => "rising",
            Slope::Falling => "falling",
        }
    }

    pub fn from_name(s: &str) -> Option<Slope> {
        match s {
            "flat" => Some(Slope::Flat),
            "rising" => Some(Slope::Rising),
            "falling" => Some(Slope::Falling),
            _ => None,
        }
    }
}

/// One item on a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry<T> {
    pub item: T,
    /// Position along the line, blocks.
    pub x: f64,
    /// The station holding it (a machine working on it), by block index.
    pub hold: Option<usize>,
}

/// Why a line doesn't move.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Fewer drives than it needs.
    UnderDriven,
    Jammed,
    Paused,
    /// No speed.
    Still,
}

/// What a station does with an item reaching its block's centre.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// It carries on.
    Pass,
    /// It stops here until the station lets it go (a machine works on it).
    Hold,
    /// The station took it (a diverter pushed it off).
    Remove,
}

/// The world as a line's step sees it: the game answers.
pub trait Stations<T> {
    /// An item reached the centre of block `index`, which has a station.
    fn arrive(&mut self, index: usize, item: &T) -> Action;
    /// An item held at block `index`: whether to keep holding it. The
    /// station may change the item in place (a finished recipe).
    fn poll(&mut self, index: usize, item: &mut T) -> bool;
    /// The front item reached the leading end: whether the block beyond took
    /// it (the station moved it into the world).
    fn deliver(&mut self, item: &T) -> bool;
}

/// What happened in a step that clients can't predict.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepOut {
    /// Items left at the end (delivered) or taken by a station.
    pub left: usize,
    /// An entry was held or released.
    pub holds_changed: bool,
    /// Items that crossed a station's centre, by block index (counters, the diverter's pulse).
    pub crossed: Vec<usize>,
}

impl StepOut {
    /// Whether clients need a new snapshot.
    pub fn eventful(&self) -> bool {
        self.left > 0 || self.holds_changed
    }
}

/// A straight belt line, or a bucket elevator's column (`vertical`).
#[derive(Clone, Debug)]
pub struct Line<T> {
    /// Blocks from the start (position 0) to the end.
    pub blocks: Vec<Pos>,
    /// The direction items move when the line runs forward (Up for an elevator).
    pub heading: Dir,
    pub slopes: Vec<Slope>,
    /// Items, sorted by position (ascending).
    pub entries: Vec<Entry<T>>,
    pub vertical: bool,
    /// Ticks until a refused item at the end is offered again.
    pub retry: u32,
}

impl<T: Clone> Line<T> {
    pub fn new(blocks: Vec<Pos>, heading: Dir, slopes: Vec<Slope>) -> Line<T> {
        debug_assert_eq!(blocks.len(), slopes.len());
        Line {
            blocks,
            heading,
            slopes,
            entries: Vec::new(),
            vertical: false,
            retry: 0,
        }
    }

    /// A bucket elevator's column, boot first.
    pub fn elevator(blocks: Vec<Pos>) -> Line<T> {
        let n = blocks.len();
        Line {
            blocks,
            heading: Dir::Up,
            slopes: vec![Slope::Flat; n],
            entries: Vec::new(),
            vertical: true,
            retry: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn length(&self) -> f64 {
        self.blocks.len() as f64
    }

    /// The block index holding `pos`.
    pub fn index_of(&self, pos: Pos) -> Option<usize> {
        self.blocks.iter().position(|p| *p == pos)
    }

    /// The block a position along the line falls in.
    pub fn block_at(&self, x: f64) -> usize {
        (x.max(0.0).floor() as usize).min(self.blocks.len().saturating_sub(1))
    }

    /// Room for an item at `x`: every entry at least `SPACING` away.
    pub fn room_at(&self, x: f64) -> bool {
        self.entries
            .iter()
            .all(|e| (e.x - x).abs() + EPS >= SPACING)
    }

    /// A free spot within `ROOM` of block `index`'s centre, nearest first (4.2).
    pub fn spot_near_centre(&self, index: usize) -> Option<f64> {
        let c = index as f64 + 0.5;
        self.free_spot(c, ROOM)
    }

    /// A free spot within `reach` of `x`, nearest first, inside the line.
    pub fn free_spot(&self, x: f64, reach: f64) -> Option<f64> {
        let lo = (x - reach).max(0.0);
        let hi = (x + reach).min(self.length());
        if lo > hi {
            return None;
        }
        // Candidates: x itself, and just clear of each nearby entry.
        let mut candidates = vec![x.clamp(lo, hi)];
        for e in &self.entries {
            for c in [e.x - SPACING, e.x + SPACING] {
                if c >= lo - EPS && c <= hi + EPS {
                    candidates.push(c.clamp(lo, hi));
                }
            }
        }
        candidates.sort_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()));
        candidates.into_iter().find(|c| self.room_at(*c))
    }

    /// Put an item at `x` if there is room; gives it back otherwise.
    pub fn insert(&mut self, x: f64, item: T) -> Result<(), T> {
        if x < -EPS || x > self.length() + EPS || !self.room_at(x) {
            return Err(item);
        }
        let x = x.clamp(0.0, self.length());
        let at = self.entries.partition_point(|e| e.x < x);
        self.entries.insert(
            at,
            Entry {
                item,
                x,
                hold: None,
            },
        );
        Ok(())
    }

    /// Put an item on block `index`, near its centre (4.2).
    pub fn insert_on(&mut self, index: usize, item: T) -> Result<f64, T> {
        match self.spot_near_centre(index) {
            Some(x) => self.insert(x, item).map(|_| x),
            None => Err(item),
        }
    }

    /// Put an item right behind `x` (behind: against the travel direction
    /// `forward`), at the spacing (logistics 7.3: byproducts).
    pub fn insert_behind(&mut self, x: f64, forward: bool, item: T) -> Result<f64, T> {
        let at = if forward { x - SPACING } else { x + SPACING };
        if at < -EPS || at > self.length() + EPS {
            return Err(item);
        }
        self.insert(at, item).map(|_| at)
    }

    /// The entry held by the station on block `index`.
    pub fn held_at(&mut self, index: usize) -> Option<&mut Entry<T>> {
        self.entries.iter_mut().find(|e| e.hold == Some(index))
    }

    /// Release whatever block `index`'s station holds.
    pub fn release(&mut self, index: usize) -> bool {
        let mut any = false;
        for e in self.entries.iter_mut().filter(|e| e.hold == Some(index)) {
            e.hold = None;
            any = true;
        }
        any
    }

    /// Take the item nearest `x` (a player's empty hand, 4.3).
    pub fn take_nearest(&mut self, x: f64) -> Option<T> {
        let i = (0..self.entries.len())
            .min_by(|a, b| {
                (self.entries[*a].x - x)
                    .abs()
                    .total_cmp(&(self.entries[*b].x - x).abs())
            })
            .filter(|i| (self.entries[*i].x - x).abs() <= 1.0)?;
        Some(self.entries.remove(i).item)
    }

    /// Take every item on block `index` (it was broken, 4.3).
    pub fn take_block(&mut self, index: usize) -> Vec<T> {
        let lo = index as f64;
        let hi = lo + 1.0;
        let last = index + 1 == self.blocks.len();
        let mut out = Vec::new();
        self.entries.retain(|e| {
            let inside = e.x >= lo - EPS && (e.x < hi - EPS || (last && e.x <= hi + EPS));
            if inside {
                out.push(e.item.clone());
            }
            !inside
        });
        out
    }

    /// Items on the line.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// How many items the line holds at most (two per block).
    pub fn capacity(&self) -> usize {
        (self.length() / SPACING).floor() as usize
    }

    /// The load the moving line puts on its network, Nm (3.4, 8.2), with
    /// the line running forward (`forward`) or backwards.
    pub fn load(&self, forward: bool) -> f64 {
        let mut load = LOAD_PER_BLOCK * self.length();
        for e in &self.entries {
            load += if self.vertical {
                ELEVATOR_LOAD_PER_ITEM
            } else {
                let s = self.slopes[self.block_at(e.x)];
                let climbing = matches!(
                    (s, forward),
                    (Slope::Rising, true) | (Slope::Falling, false)
                );
                if climbing {
                    LOAD_PER_ITEM_RISING
                } else {
                    LOAD_PER_ITEM
                }
            };
        }
        load
    }

    /// One tick at `v` blocks per tick (signed): every entry moves, front to
    /// back, stopping behind the one ahead at the spacing, at a station that
    /// holds it, or at the end if nothing takes it there (4.1, 5.1, 7).
    /// `stations` are the block indices whose centre a station watches.
    pub fn step(&mut self, v: f64, stations: &[usize], world: &mut dyn Stations<T>) -> StepOut {
        let mut out = StepOut::default();
        // Held entries first: a station may finish and let go.
        for e in self.entries.iter_mut() {
            if let Some(i) = e.hold
                && !world.poll(i, &mut e.item)
            {
                e.hold = None;
                out.holds_changed = true;
            }
        }
        if v.abs() < 1e-12 || self.entries.is_empty() {
            return out;
        }
        let forward = v > 0.0;
        let len = self.length();
        let step = v.abs();
        // Travel coordinate: t = x forward, len - x backwards; walk the
        // entries from the leading end.
        let n = self.entries.len();
        let order: Vec<usize> = if forward {
            (0..n).rev().collect()
        } else {
            (0..n).collect()
        };
        let to_t = |x: f64| if forward { x } else { len - x };
        let from_t = |t: f64| if forward { t } else { len - t };
        let mut ahead: Option<f64> = None;
        let mut remove: Vec<usize> = Vec::new();
        let mut leader = true;
        for i in order {
            let t0 = to_t(self.entries[i].x);
            let limit = match ahead {
                Some(a) => a - SPACING,
                None => len,
            };
            let held = self.entries[i].hold.is_some();
            let mut t1 = if held { t0 } else { (t0 + step).min(limit) };
            if t1 < t0 {
                // Already closer than the spacing (an insertion): wait.
                t1 = t0;
            }
            // Station centres crossed on the way, nearest first.
            if !held {
                for &s in stations_between(stations, t0, t1, forward, len).iter() {
                    let ct = to_t(s as f64 + 0.5);
                    out.crossed.push(s);
                    match world.arrive(s, &self.entries[i].item) {
                        Action::Pass => {}
                        Action::Hold => {
                            t1 = ct;
                            self.entries[i].hold = Some(s);
                            out.holds_changed = true;
                            break;
                        }
                        Action::Remove => {
                            remove.push(i);
                            out.left += 1;
                            t1 = f64::NAN;
                            break;
                        }
                    }
                }
            }
            if t1.is_nan() {
                // Taken: it no longer blocks the ones behind.
                continue;
            }
            // The leader at the end: offer it to the block beyond.
            if leader && t1 >= len - EPS && self.entries[i].hold.is_none() {
                t1 = len;
                if self.retry == 0 {
                    if world.deliver(&self.entries[i].item) {
                        remove.push(i);
                        out.left += 1;
                        // At most one leaves per tick; the next waits at
                        // the end if it gets there.
                        ahead = Some(len + SPACING);
                        leader = false;
                        continue;
                    }
                    self.retry = RETRY_TICKS;
                } else {
                    self.retry -= 1;
                }
            }
            leader = false;
            self.entries[i].x = from_t(t1);
            ahead = Some(t1);
        }
        remove.sort_unstable();
        for i in remove.into_iter().rev() {
            self.entries.remove(i);
        }
        // Keep the order (positions never cross, but be safe after removals).
        self.entries.sort_by(|a, b| a.x.total_cmp(&b.x));
        out
    }

    /// The world point of a position along the line: the centre of the belt
    /// surface, in block coordinates (x, y, z), for drawing.
    pub fn point(&self, x: f64) -> [f64; 3] {
        let i = self.block_at(x);
        let f = (x - i as f64).clamp(0.0, 1.0);
        let b = self.blocks[i];
        let [hx, hy, hz] = self.heading.step();
        let base = [b.x as f64 + 0.5, b.y as f64, b.z as f64 + 0.5];
        if self.vertical {
            return [base[0], b.y as f64 + f, base[2]];
        }
        let rise = match self.slopes[i] {
            Slope::Flat => 0.0,
            Slope::Rising => f,
            Slope::Falling => 1.0 - f,
        };
        let surface = BELT_TOP;
        [
            base[0] + hx as f64 * (f - 0.5),
            base[1] + hy as f64 * (f - 0.5) + rise + surface,
            base[2] + hz as f64 * (f - 0.5),
        ]
    }

    /// The block in front of the leading end: where items go (5.1).
    pub fn end_target(&self, forward: bool) -> Pos {
        if self.vertical {
            // The head's outlet is the game's (it has a facing).
            return self.blocks[self.blocks.len() - 1];
        }
        if forward {
            let last = self.blocks[self.blocks.len() - 1];
            last.offset(self.heading)
        } else {
            self.blocks[0].offset(self.heading.opposite())
        }
    }

    /// Entries inside block `index` as (item, offset within the block 0..1),
    /// for saving with that block (10.2).
    pub fn entries_in(&self, index: usize) -> Vec<(T, f64)> {
        let lo = index as f64;
        let last = index + 1 == self.blocks.len();
        self.entries
            .iter()
            .filter(|e| {
                e.x >= lo - EPS && (e.x < lo + 1.0 - EPS || (last && e.x <= lo + 1.0 + EPS))
            })
            .map(|e| (e.item.clone(), (e.x - lo).clamp(0.0, 1.0)))
            .collect()
    }

    /// Put back entries saved by one block (offsets within it). Entries
    /// that would overlap are pushed back along the line where there is room,
    /// else dropped and returned.
    pub fn restore(&mut self, index: usize, saved: Vec<(T, f64)>) -> Vec<T> {
        let mut lost = Vec::new();
        for (item, off) in saved {
            let x = index as f64 + off.clamp(0.0, 1.0);
            let x = x.min(self.length());
            match self.free_spot(x, 1.0) {
                Some(at) => {
                    let _ = self.insert(at, item);
                }
                None => lost.push(item),
            }
        }
        lost
    }

    /// What clients need to extrapolate: (position, held) per entry.
    pub fn snapshot(&self) -> Vec<(f64, bool)> {
        self.entries
            .iter()
            .map(|e| (e.x, e.hold.is_some()))
            .collect()
    }
}

/// Height of the belt surface above its block's floor (6 texels, 3.1).
pub const BELT_TOP: f64 = 6.0 / 16.0;

/// Station blocks whose centre lies in (t0, t1] in travel coordinates, in
/// travel order.
fn stations_between(stations: &[usize], t0: f64, t1: f64, forward: bool, len: f64) -> Vec<usize> {
    let mut out: Vec<(f64, usize)> = stations
        .iter()
        .filter_map(|&s| {
            let c = s as f64 + 0.5;
            let ct = if forward { c } else { len - c };
            (ct > t0 + EPS && ct <= t1 + EPS).then_some((ct, s))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, s)| s).collect()
}

/// The client's prediction (10.3): `ticks` ticks of free movement at `v`
/// from a snapshot of (position, held), with nothing getting on or off:
/// free entries stop at the spacing behind the one ahead and at the end.
/// The same arithmetic as `Line::step` with no stations and a blocked end.
pub fn extrapolate(entries: &[(f64, bool)], v: f64, len: f64, ticks: u32) -> Vec<f64> {
    let mut xs: Vec<f64> = entries.iter().map(|e| e.0).collect();
    if v.abs() < 1e-12 {
        return xs;
    }
    let forward = v > 0.0;
    let step = v.abs();
    let n = xs.len();
    for _ in 0..ticks {
        let mut ahead: Option<f64> = None;
        let order: Vec<usize> = if forward {
            (0..n).rev().collect()
        } else {
            (0..n).collect()
        };
        for i in order {
            let t0 = if forward { xs[i] } else { len - xs[i] };
            let limit = ahead.map_or(len, |a| a - SPACING);
            let mut t1 = if entries[i].1 {
                t0
            } else {
                (t0 + step).min(limit)
            };
            if t1 < t0 {
                t1 = t0;
            }
            xs[i] = if forward { t1 } else { len - t1 };
            ahead = Some(t1);
        }
    }
    xs
}

/// Extrapolate by a fraction of a tick too (drawing between ticks): the
/// position after `ticks` whole ticks, then a part of the next.
pub fn extrapolate_partial(
    entries: &[(f64, bool)],
    v: f64,
    len: f64,
    ticks: u32,
    partial: f64,
) -> Vec<f64> {
    let a = extrapolate(entries, v, len, ticks);
    if partial <= 0.0 {
        return a;
    }
    let snap: Vec<(f64, bool)> = a.iter().zip(entries).map(|(x, e)| (*x, e.1)).collect();
    let b = extrapolate(&snap, v, len, 1);
    a.iter()
        .zip(b)
        .map(|(x, y)| x + (y - x) * partial)
        .collect()
}

// ---- Tracing lines from the world ------------------------------------------------

/// A belt block as tracing sees it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BeltBlock {
    pub heading: Dir,
}

/// The chain of belts a block belongs to, start first: neighbours along the
/// heading at the same height or one up or down, with the same heading,
/// join (3.1). A belt that falls into this block can't rise out of it.
pub fn chain(start: Pos, get: &dyn Fn(Pos) -> Option<BeltBlock>) -> Vec<Pos> {
    let Some(b) = get(start) else {
        return Vec::new();
    };
    let h = b.heading;
    let next = |p: Pos| -> Option<Pos> {
        let ahead = p.offset(h);
        [ahead, ahead.offset(Dir::Up), ahead.offset(Dir::Down)]
            .into_iter()
            .find(|q| get(*q).is_some_and(|b| b.heading == h))
    };
    let prev = |p: Pos| -> Option<Pos> {
        let back = p.offset(h.opposite());
        [back, back.offset(Dir::Up), back.offset(Dir::Down)]
            .into_iter()
            .find(|q| get(*q).is_some_and(|b| b.heading == h))
    };
    // Walk back to the first block (bounded: a loop can't happen in a
    // straight line, but stay safe).
    let mut first = start;
    let mut guard = 0;
    while let Some(p) = prev(first) {
        if next(p) != Some(first) {
            break;
        }
        first = p;
        guard += 1;
        if guard > 4096 {
            break;
        }
    }
    let mut out = vec![first];
    let mut at = first;
    while let Some(q) = next(at) {
        if prev(q) != Some(at) || out.len() > 4096 {
            break;
        }
        // A V (down then up) doesn't join: a falling block can't rise.
        if q.y > at.y && out.len() >= 2 && out[out.len() - 2].y > at.y {
            break;
        }
        out.push(q);
        at = q;
    }
    out
}

/// Split a chain into lines of at most `MAX_LEN` blocks (3.1, 5.2), with
/// each block's slope.
pub fn split(chain: &[Pos]) -> Vec<(Vec<Pos>, Vec<Slope>)> {
    let slopes = slopes(chain);
    chain
        .chunks(MAX_LEN)
        .zip(slopes.chunks(MAX_LEN))
        .map(|(b, s)| (b.to_vec(), s.to_vec()))
        .collect()
}

/// Each block's slope from its neighbours' heights (3.1).
pub fn slopes(chain: &[Pos]) -> Vec<Slope> {
    (0..chain.len())
        .map(|i| {
            let y = chain[i].y;
            if i + 1 < chain.len() && chain[i + 1].y == y + 1 {
                Slope::Rising
            } else if i > 0 && chain[i - 1].y == y + 1 {
                Slope::Falling
            } else {
                Slope::Flat
            }
        })
        .collect()
}

/// A bucket elevator column's check (8.1): its height, or what's wrong.
pub fn elevator_ok(height: usize) -> Result<(), &'static str> {
    if height < ELEVATOR_MIN {
        Err("too_short")
    } else if height > ELEVATOR_MAX {
        Err("too_tall")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
