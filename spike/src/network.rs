//! THROWAWAY SPIKE CODE. The shaft network, free of the game: which parts
//! are connected, the push/pull speed model, and the rule that a network with
//! any part in an unloaded chunk pauses. It exists to prove the shape of the
//! problem on Horizon, not to be the real solver (that gets its own spec).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

pub type Pos = [i32; 3];

/// An axis a shaft runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn from_name(name: &str) -> Option<Axis> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    /// The unit vector of the axis.
    pub fn vector(self) -> [i32; 3] {
        match self {
            Axis::X => [1, 0, 0],
            Axis::Y => [0, 1, 0],
            Axis::Z => [0, 0, 1],
        }
    }
}

/// What a network part is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Connects to its two neighbours along its axis.
    Shaft(Axis),
    /// A source: connects on every side to a shaft pointing at it.
    Crank,
    /// A consumer: connects on every side to a shaft pointing at it.
    Grinder,
}

/// What the world holds at a position, as the network sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    /// The chunk isn't loaded: the network can't be complete.
    Unloaded,
    /// Loaded, and nothing of ours.
    Empty,
    Part(Kind),
}

/// The world, as the network reads it.
pub trait World {
    fn lookup(&self, pos: Pos) -> Lookup;
}

const SIDES: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

fn add(a: Pos, b: [i32; 3]) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn neg(a: [i32; 3]) -> [i32; 3] {
    [-a[0], -a[1], -a[2]]
}

/// The directions a part reaches out in.
fn reaches(kind: Kind) -> Vec<[i32; 3]> {
    match kind {
        Kind::Shaft(axis) => vec![axis.vector(), neg(axis.vector())],
        Kind::Crank | Kind::Grinder => SIDES.to_vec(),
    }
}

/// Whether `a` and `b`, with `b` at `a + dir`, are mechanically connected:
/// both must reach toward each other.
fn connected(a: Kind, b: Kind, dir: [i32; 3]) -> bool {
    let a_reaches = reaches(a).contains(&dir);
    let b_reaches = reaches(b).contains(&neg(dir));
    // Two non-shaft parts side by side don't connect: power goes through shafts.
    let some_shaft = matches!(a, Kind::Shaft(_)) || matches!(b, Kind::Shaft(_));
    a_reaches && b_reaches && some_shaft
}

/// One connected network, found from a seed.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub members: BTreeMap<Pos, Kind>,
    /// A member had a neighbour in an unloaded chunk.
    pub incomplete: bool,
}

/// Walks the network from `seed`. `None` when `seed` holds no part.
pub fn discover(world: &dyn World, seed: Pos) -> Option<Found> {
    let Lookup::Part(kind) = world.lookup(seed) else {
        return None;
    };
    let mut members = BTreeMap::from([(seed, kind)]);
    let mut queue = VecDeque::from([(seed, kind)]);
    let mut incomplete = false;
    while let Some((pos, kind)) = queue.pop_front() {
        for dir in reaches(kind) {
            let next = add(pos, dir);
            if members.contains_key(&next) {
                continue;
            }
            match world.lookup(next) {
                Lookup::Unloaded => incomplete = true,
                Lookup::Empty => {}
                Lookup::Part(other) => {
                    if connected(kind, other, dir) {
                        members.insert(next, other);
                        queue.push_back((next, other));
                    }
                }
            }
        }
    }
    Some(Found {
        members,
        incomplete,
    })
}

/// The push/pull model's constants. *(tune)*: placeholders, not balance.
pub mod tuning {
    /// A crank's stall torque while a player turns it.
    pub const CRANK_TORQUE: f32 = 8.0;
    /// The speed at which a crank's torque reaches zero.
    pub const CRANK_TOP_RPM: f32 = 32.0;
    /// Ticks a right-click keeps the crank pushing.
    pub const CRANK_TICKS: u32 = 20;
    /// A working grinder's load.
    pub const GRINDER_LOAD: f32 = 3.0;
    /// Friction per part, proportional to speed.
    pub const FRICTION_PER_PART: f32 = 0.02;
    /// Inertia per part (a flywheel would add a lot).
    pub const INERTIA_PER_PART: f32 = 0.5;
}

/// One step of the speed model: `push` and `pull` are torques at the current
/// speed, `inertia` the network's. Returns the new speed (RPM).
pub fn step_speed(rpm: f32, push: f32, pull: f32, inertia: f32) -> f32 {
    let accel = (push - pull) / inertia.max(0.001);
    let next = rpm + accel;
    // Load and friction only ever slow the network down: they can't reverse it.
    if push == 0.0 && next.signum() != rpm.signum() {
        0.0
    } else {
        next
    }
}

/// A crank's push at `rpm` (falls to zero at its top speed).
pub fn crank_torque(rpm: f32) -> f32 {
    use tuning::*;
    (CRANK_TORQUE * (1.0 - rpm.abs() / CRANK_TOP_RPM)).max(0.0)
}

/// A live network.
#[derive(Clone, Debug, PartialEq)]
pub struct Network {
    pub members: BTreeMap<Pos, Kind>,
    pub rpm: f32,
    pub paused: bool,
}

impl Network {
    pub fn inertia(&self) -> f32 {
        self.members.len() as f32 * tuning::INERTIA_PER_PART
    }

    /// One tick: `cranking` are the cranks being turned, `working` the
    /// grinders with something to grind.
    pub fn tick(&mut self, cranking: &BTreeSet<Pos>, working: &BTreeSet<Pos>) {
        if self.paused {
            return;
        }
        let mut push = 0.0;
        let mut pull = self.members.len() as f32 * tuning::FRICTION_PER_PART * self.rpm.abs();
        for (pos, kind) in &self.members {
            match kind {
                Kind::Crank if cranking.contains(pos) => push += crank_torque(self.rpm),
                Kind::Grinder if working.contains(pos) && self.rpm.abs() > 0.5 => {
                    pull += tuning::GRINDER_LOAD
                }
                _ => {}
            }
        }
        self.rpm = step_speed(self.rpm, push, pull, self.inertia());
        if self.rpm.abs() < 0.01 {
            self.rpm = 0.0;
        }
    }
}

/// Every network of one level, rebuilt only when something changed.
#[derive(Default, Debug)]
pub struct Networks {
    nets: Vec<Network>,
    by_pos: HashMap<Pos, usize>,
    /// Positions whose network must be found again.
    dirty: BTreeSet<Pos>,
    /// Cranks a player turned, with the ticks of push they have left.
    cranks: BTreeMap<Pos, u32>,
}

impl Networks {
    /// Something changed at `pos`: rebuild what touches it on the next tick.
    pub fn mark(&mut self, pos: Pos) {
        self.dirty.insert(pos);
        for dir in SIDES {
            self.dirty.insert(add(pos, dir));
        }
    }

    /// A chunk loaded or unloaded: every network with a member in or next to
    /// it is rebuilt (an unloaded neighbour pauses it).
    pub fn mark_chunk(&mut self, chunk: [i32; 2]) {
        let touching: Vec<Pos> = self
            .by_pos
            .keys()
            .filter(|p| {
                let [cx, cz] = [p[0].div_euclid(16), p[2].div_euclid(16)];
                (cx - chunk[0]).abs() <= 1 && (cz - chunk[1]).abs() <= 1
            })
            .copied()
            .collect();
        self.dirty.extend(touching);
    }

    pub fn crank(&mut self, pos: Pos) {
        self.cranks.insert(pos, tuning::CRANK_TICKS);
    }

    /// The network at `pos`, if any.
    pub fn at(&self, pos: Pos) -> Option<&Network> {
        self.by_pos.get(&pos).map(|&i| &self.nets[i])
    }

    /// Members of any network that aren't in `alive`.
    pub fn members_not_in(&self, alive: &BTreeSet<Pos>) -> Vec<Pos> {
        self.by_pos
            .keys()
            .filter(|p| !alive.contains(*p))
            .copied()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.nets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nets.is_empty()
    }

    /// Rebuilds the dirty networks. `saved_rpm` gives the speed a part saved,
    /// for a network that comes back after being unloaded.
    pub fn rebuild(&mut self, world: &dyn World, saved_rpm: &dyn Fn(Pos) -> Option<f32>) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty = std::mem::take(&mut self.dirty);
        // Old networks touched by a dirty position give up their members (and
        // their speed, which carries over to whatever they become).
        let mut carried: HashMap<Pos, f32> = HashMap::new();
        let touched: BTreeSet<usize> = dirty
            .iter()
            .filter_map(|p| self.by_pos.get(p).copied())
            .collect();
        for &i in &touched {
            for pos in self.nets[i].members.keys() {
                carried.insert(*pos, self.nets[i].rpm);
            }
        }
        let mut seeds: BTreeSet<Pos> = dirty;
        seeds.extend(carried.keys().copied());
        let mut kept: Vec<Network> = self
            .nets
            .drain(..)
            .enumerate()
            .filter(|(i, _)| !touched.contains(i))
            .map(|(_, n)| n)
            .collect();
        let mut claimed: BTreeSet<Pos> = BTreeSet::new();
        for seed in seeds {
            if claimed.contains(&seed) || kept.iter().any(|n| n.members.contains_key(&seed)) {
                continue;
            }
            let Some(found) = discover(world, seed) else {
                continue;
            };
            claimed.extend(found.members.keys().copied());
            // Speed: what the old networks had (the fastest piece wins when
            // two join), else what the parts saved, else rest.
            let from_old = found
                .members
                .keys()
                .filter_map(|p| carried.get(p).copied())
                .fold(None, |acc: Option<f32>, r| {
                    Some(acc.map_or(r, |a| if r.abs() > a.abs() { r } else { a }))
                });
            let rpm = from_old
                .or_else(|| found.members.keys().find_map(|p| saved_rpm(*p)))
                .unwrap_or(0.0);
            kept.push(Network {
                members: found.members,
                rpm,
                paused: found.incomplete,
            });
        }
        self.nets = kept;
        self.by_pos = self
            .nets
            .iter()
            .enumerate()
            .flat_map(|(i, n)| n.members.keys().map(move |p| (*p, i)))
            .collect();
    }

    /// One tick of every network.
    pub fn tick(&mut self, working: &BTreeSet<Pos>) {
        let cranking: BTreeSet<Pos> = self.cranks.keys().copied().collect();
        for net in &mut self.nets {
            net.tick(&cranking, working);
        }
        self.cranks.retain(|_, left| {
            *left -= 1;
            *left > 0
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test world: parts at positions, and chunks that are unloaded.
    #[derive(Default)]
    struct Grid {
        parts: HashMap<Pos, Kind>,
        unloaded: BTreeSet<[i32; 2]>,
    }

    impl World for Grid {
        fn lookup(&self, pos: Pos) -> Lookup {
            if self
                .unloaded
                .contains(&[pos[0].div_euclid(16), pos[2].div_euclid(16)])
            {
                return Lookup::Unloaded;
            }
            self.parts
                .get(&pos)
                .map_or(Lookup::Empty, |k| Lookup::Part(*k))
        }
    }

    /// A crank at x=0, shafts along x up to `len`, a grinder after them.
    fn line(len: i32) -> Grid {
        let mut g = Grid::default();
        g.parts.insert([0, 64, 0], Kind::Crank);
        for x in 1..=len {
            g.parts.insert([x, 64, 0], Kind::Shaft(Axis::X));
        }
        g.parts.insert([len + 1, 64, 0], Kind::Grinder);
        g
    }

    fn none(_: Pos) -> Option<f32> {
        None
    }

    #[test]
    fn discovers_a_line_through_shafts() {
        let g = line(5);
        let found = discover(&g, [3, 64, 0]).unwrap();
        assert_eq!(found.members.len(), 7);
        assert!(!found.incomplete);
    }

    #[test]
    fn a_shaft_across_its_axis_does_not_connect() {
        let mut g = line(3);
        g.parts.insert([2, 64, 1], Kind::Shaft(Axis::X));
        let found = discover(&g, [2, 64, 0]).unwrap();
        assert!(!found.members.contains_key(&[2, 64, 1]));
    }

    #[test]
    fn two_machines_side_by_side_do_not_connect() {
        let mut g = Grid::default();
        g.parts.insert([0, 64, 0], Kind::Crank);
        g.parts.insert([1, 64, 0], Kind::Grinder);
        assert_eq!(discover(&g, [0, 64, 0]).unwrap().members.len(), 1);
    }

    #[test]
    fn cranking_speeds_up_and_load_sags() {
        let g = line(3);
        let mut nets = Networks::default();
        nets.mark([0, 64, 0]);
        nets.rebuild(&g, &none);
        let idle = BTreeSet::new();
        for _ in 0..200 {
            nets.crank([0, 64, 0]);
            nets.tick(&idle);
        }
        let free = nets.at([0, 64, 0]).unwrap().rpm;
        assert!(free > 20.0 && free < tuning::CRANK_TOP_RPM, "{free}");
        let working = BTreeSet::from([[4, 64, 0]]);
        for _ in 0..200 {
            nets.crank([0, 64, 0]);
            nets.tick(&working);
        }
        let loaded = nets.at([0, 64, 0]).unwrap().rpm;
        assert!(loaded < free, "{loaded} < {free}");
    }

    #[test]
    fn stops_when_nobody_cranks() {
        let g = line(3);
        let mut nets = Networks::default();
        nets.mark([0, 64, 0]);
        nets.rebuild(&g, &none);
        for _ in 0..100 {
            nets.crank([0, 64, 0]);
            nets.tick(&BTreeSet::new());
        }
        for _ in 0..2000 {
            nets.tick(&BTreeSet::new());
        }
        assert_eq!(nets.at([0, 64, 0]).unwrap().rpm, 0.0);
    }

    #[test]
    fn a_network_across_an_unloaded_chunk_pauses_and_keeps_its_speed() {
        // A line from x=0 to x=21 crosses the chunk border at x=16.
        let mut g = line(20);
        let mut nets = Networks::default();
        nets.mark([0, 64, 0]);
        nets.rebuild(&g, &none);
        for _ in 0..100 {
            nets.crank([0, 64, 0]);
            nets.tick(&BTreeSet::new());
        }
        let before = nets.at([0, 64, 0]).unwrap().rpm;
        assert!(before > 0.0);
        // Chunk x=1 unloads: the rest is paused, with its speed.
        g.unloaded.insert([1, 0]);
        nets.mark_chunk([1, 0]);
        nets.rebuild(&g, &none);
        let net = nets.at([0, 64, 0]).unwrap();
        assert!(net.paused);
        assert_eq!(net.rpm, before);
        for _ in 0..100 {
            nets.tick(&BTreeSet::new());
        }
        assert_eq!(nets.at([0, 64, 0]).unwrap().rpm, before);
        // It loads again: one network, running, at the same speed.
        g.unloaded.clear();
        nets.mark_chunk([1, 0]);
        nets.rebuild(&g, &none);
        let net = nets.at([0, 64, 0]).unwrap();
        assert!(!net.paused);
        assert_eq!(net.members.len(), 22);
        assert_eq!(net.rpm, before);
        assert_eq!(nets.len(), 1);
    }

    #[test]
    fn a_network_reloaded_from_disk_takes_its_saved_speed() {
        let g = line(3);
        let mut nets = Networks::default();
        nets.mark([2, 64, 0]);
        nets.rebuild(&g, &|p| (p == [2, 64, 0]).then_some(17.5));
        assert_eq!(nets.at([0, 64, 0]).unwrap().rpm, 17.5);
    }

    #[test]
    fn breaking_a_shaft_splits_the_network_and_both_halves_keep_turning() {
        let mut g = line(6);
        let mut nets = Networks::default();
        nets.mark([0, 64, 0]);
        nets.rebuild(&g, &none);
        for _ in 0..50 {
            nets.crank([0, 64, 0]);
            nets.tick(&BTreeSet::new());
        }
        let rpm = nets.at([0, 64, 0]).unwrap().rpm;
        g.parts.remove(&[3, 64, 0]);
        nets.mark([3, 64, 0]);
        nets.rebuild(&g, &none);
        assert_eq!(nets.len(), 2);
        assert_eq!(nets.at([0, 64, 0]).unwrap().rpm, rpm);
        assert_eq!(nets.at([7, 64, 0]).unwrap().rpm, rpm);
        assert!(nets.at([3, 64, 0]).is_none());
    }

    #[test]
    fn rebuild_is_skipped_when_nothing_changed() {
        let g = line(3);
        let mut nets = Networks::default();
        nets.mark([0, 64, 0]);
        nets.rebuild(&g, &none);
        let empty = Grid::default();
        // Nothing marked: the cached network survives even a world that lies.
        nets.rebuild(&empty, &none);
        assert_eq!(nets.len(), 1);
    }
}
