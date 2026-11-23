//! The part registry and its networks: building systems from the parts
//! (ratios, loops, spans), integrating them, damage, belts and pausing
//! (rotational network spec 3 to 7 and 10). One `Mechanics` per level.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use super::damage::{self, Jam};
use super::flow;
use super::part::*;
use super::solve::{self, ACCEL, DT, Terms};
use crate::sim::geom::{Chunk, Dir, Pos};

/// A source's push for this tick: stall torque, top speed and direction (±1)
/// about the part's own positive axis.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Drive {
    pub stall: f64,
    pub top: f64,
    pub dir: f64,
}

/// What a part saves with its block and gets back on load.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Saved {
    pub omega: f64,
    pub jam: Option<Jam>,
    pub over_t: f64,
    pub shear_t: f64,
}

/// The world as the network needs it.
pub trait View {
    /// Whether the chunk holding `pos` is loaded (so a part there would be known).
    fn loaded(&self, pos: Pos) -> bool;
    /// Whether block entities in `chunk` are simulated (tick).
    fn simulated(&self, chunk: Chunk) -> bool;
}

/// Something the game should show or do.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Jammed(Pos, Jam),
    Slipped(Pos, Pos),
    Gripped(Pos, Pos),
}

/// A part in the registry.
#[derive(Clone, Debug)]
pub struct PartState {
    pub spec: Spec,
    /// The part's own speed variable (RPM), kept up to date when the system
    /// dissolves and when it is read.
    pub omega: f64,
    pub jam: Option<Jam>,
    /// Overspeed warning timer, 0 to 1.
    pub over_t: f64,
    /// Overload (shear) warning timer, 0 to 1.
    pub shear_t: f64,
    /// False once its chunk unloaded.
    pub loaded: bool,
    pub drive: Drive,
    pub load: f64,
    /// Sagging (part of a span that is too long).
    pub sag: bool,
    /// The span this part is in, if any: its length and limit.
    pub span: Option<(usize, usize)>,
    /// Torque carried (local Nm), from the latest flow.
    pub torque: f64,
    sys: Option<usize>,
    member: usize,
    /// What clients last got.
    pub(crate) sent: Sent,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Sent {
    pub rpm: f64,
    pub warn: u8,
    pub sag: bool,
    pub jam: Option<Jam>,
    pub fresh: bool,
}

impl PartState {
    fn new(spec: Spec, saved: Saved) -> PartState {
        PartState {
            spec,
            omega: saved.omega,
            jam: saved.jam,
            over_t: saved.over_t,
            shear_t: saved.shear_t,
            loaded: true,
            drive: Drive::default(),
            load: 0.0,
            sag: false,
            span: None,
            torque: 0.0,
            sys: None,
            member: 0,
            sent: Sent {
                rpm: saved.omega,
                jam: saved.jam,
                ..Sent::default()
            },
        }
    }

    /// The faces it connects through now: none while jammed.
    fn ports(&self) -> Vec<(Dir, f64)> {
        if self.jam.is_some() {
            Vec::new()
        } else {
            self.spec.ports()
        }
    }

    pub fn saved(&self) -> Saved {
        Saved {
            omega: self.omega,
            jam: self.jam,
            over_t: self.over_t,
            shear_t: self.shear_t,
        }
    }

    /// Its torque limit now (halved while sagging).
    pub fn torque_limit(&self) -> Option<f64> {
        self.spec
            .torque_limit()
            .map(|l| if self.sag { l / 2.0 } else { l })
    }

    /// The warning level clients show (0 to 3).
    pub fn warning(&self) -> u8 {
        damage::warning_level(self.over_t.max(self.shear_t))
    }
}

#[derive(Clone, Debug)]
struct Member {
    pos: Pos,
    r: f64,
    inertia: f64,
    c: f64,
}

/// A belt as the build sees it.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Belt {
    a: Pos,
    b: Pos,
    /// ω_b = k · ω_a (local speeds about the shared positive axis).
    k: f64,
    da: f64,
    db: f64,
}

type BeltKey = (Pos, Pos);

fn belt_key(a: Pos, b: Pos) -> BeltKey {
    if a <= b { (a, b) } else { (b, a) }
}

#[derive(Clone, Debug)]
struct System {
    members: Vec<Member>,
    edges: Vec<(usize, usize)>,
    /// Gripping belts inside the system: edge index and the belt.
    belts: Vec<(usize, Belt)>,
    omega: f64,
    inertia: f64,
    b_static: f64,
    c_static: f64,
    sources: Vec<usize>,
    machines: Vec<usize>,
    limited: Vec<usize>,
    segments: Vec<usize>,
    red_ref: f64,
    cap_ref: f64,
    net: usize,
    flow_due: bool,
    last_flow: u64,
    flow_inputs: f64,
    flow_omega: f64,
    alpha: f64,
    /// Terms of the last tick, without belt slip.
    terms: Terms,
    warn_active: bool,
    shear_active: bool,
    sent_omega: f64,
}

#[derive(Clone, Debug)]
struct Network {
    systems: Vec<usize>,
    chunks: BTreeSet<Chunk>,
    frontier: Vec<Pos>,
    slips: Vec<Belt>,
    paused: bool,
}

/// What the tachometer and the wrench read about a part's network, in that
/// part's local units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub rpm: f64,
    pub paused: bool,
    /// The lowest max speed on the network, at this part.
    pub red_line: Option<f64>,
    /// The highest minimum among working machines, at this part.
    pub min_working: Option<f64>,
    /// Torque drawn (loads and friction) and what the sources give now.
    pub drawn: f64,
    pub give: f64,
    /// What the sources give at standstill.
    pub stall: f64,
    pub parts: usize,
}

/// One level's parts and networks.
#[derive(Default)]
pub struct Mechanics {
    parts: HashMap<Pos, PartState>,
    systems: Vec<Option<System>>,
    nets: Vec<Option<Network>>,
    slipping: HashSet<BeltKey>,
    dirty: BTreeSet<Pos>,
    causes: BTreeSet<Pos>,
    events: Vec<Event>,
    ticks: u64,
    /// Speeds of systems dissolved since the last build, by member.
    old: HashMap<Pos, (usize, f64)>,
    old_ids: usize,
    /// Rebuilds done (for tests and the profiler).
    pub rebuilds: u64,
}

/// Belts and the flows' recompute rules.
const FLOW_EVERY: u64 = 4;
const FLOW_CHANGE: f64 = 0.05;
const REGRIP_RPM: f64 = 0.5;
/// Extra friction on each pulley of a slipping belt.
const SLIP_FRICTION: f64 = 0.04;

impl Mechanics {
    pub fn new() -> Mechanics {
        Mechanics::default()
    }

    // ---- What the game reports -------------------------------------------

    /// A part placed (`cause`: by a player, so joining may shock it) or loaded
    /// with its saved state.
    pub fn insert(&mut self, pos: Pos, spec: Spec, saved: Option<Saved>, cause: bool) {
        if let Some(p) = self.parts.get_mut(&pos)
            && p.spec == spec
            && !cause
        {
            // Its chunk came back: the registry's state is the newer.
            p.loaded = true;
            return;
        }
        self.dissolve_at(pos);
        let state = PartState::new(spec, saved.unwrap_or_default());
        self.parts.insert(pos, state);
        self.dirty.insert(pos);
        if cause {
            self.causes.insert(pos);
        }
    }

    /// A part's block changed what it is (wrench, clutch, belt). No-op if equal.
    pub fn update(&mut self, pos: Pos, spec: Spec, cause: bool) {
        let Some(p) = self.parts.get(&pos) else {
            return;
        };
        if p.spec == spec {
            return;
        }
        let partner = p.spec.belt.map(|b| b.partner);
        self.dissolve_at(pos);
        if let Some(q) = partner {
            self.dirty.insert(q);
        }
        if let Some(p) = self.parts.get_mut(&pos) {
            p.spec = spec;
        }
        self.dirty.insert(pos);
        if cause {
            self.causes.insert(pos);
        }
    }

    /// A part broken.
    pub fn remove(&mut self, pos: Pos) {
        if !self.parts.contains_key(&pos) {
            return;
        }
        self.dissolve_at(pos);
        if let Some(p) = self.parts.remove(&pos)
            && let Some(b) = p.spec.belt
        {
            self.dirty.insert(b.partner);
            self.slipping.remove(&belt_key(pos, b.partner));
        }
        self.dirty.insert(pos);
    }

    /// Its chunk unloaded: the network pauses; once every part of a network
    /// is unloaded it leaves the registry (it comes back from saved data).
    pub fn unload(&mut self, pos: Pos) {
        let Some(p) = self.parts.get_mut(&pos) else {
            return;
        };
        p.loaded = false;
        let Some(sys) = p.sys else {
            if self.dirty.contains(&pos) {
                return;
            }
            self.parts.remove(&pos);
            return;
        };
        let net = self.sys(sys).net;
        if let Some(n) = self.nets[net].as_mut() {
            n.paused = true;
        }
        let all_gone = self
            .net_members(net)
            .all(|q| self.parts.get(&q).is_none_or(|p| !p.loaded));
        if all_gone {
            let members: Vec<Pos> = self.net_members(net).collect();
            self.drop_net(net);
            for q in members {
                self.parts.remove(&q);
                self.dirty.remove(&q);
            }
        }
    }

    pub fn set_drive(&mut self, pos: Pos, drive: Drive) {
        if let Some(p) = self.parts.get_mut(&pos) {
            p.drive = drive;
        }
    }

    pub fn set_load(&mut self, pos: Pos, load: f64) {
        if let Some(p) = self.parts.get_mut(&pos) {
            p.load = load;
        }
    }

    /// Repair a jammed part: it rejoins at once (and may shock).
    pub fn repair(&mut self, pos: Pos) -> bool {
        let Some(p) = self.parts.get_mut(&pos) else {
            return false;
        };
        if p.jam.is_none() {
            return false;
        }
        p.jam = None;
        p.over_t = 0.0;
        p.shear_t = 0.0;
        self.dissolve_at(pos);
        self.dirty.insert(pos);
        self.causes.insert(pos);
        true
    }

    // ---- Reads -----------------------------------------------------------

    pub fn part(&self, pos: Pos) -> Option<&PartState> {
        self.parts.get(&pos)
    }

    pub fn contains(&self, pos: Pos) -> bool {
        self.parts.contains_key(&pos)
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The part's local speed now, and whether its network is paused.
    pub fn speed(&self, pos: Pos) -> Option<(f64, bool)> {
        let p = self.parts.get(&pos)?;
        match p.sys {
            Some(s) => {
                let sys = self.sys(s);
                let paused = self.nets[sys.net].as_ref().is_none_or(|n| n.paused);
                Some((sys.members[p.member].r * sys.omega, paused))
            }
            None => Some((p.omega, true)),
        }
    }

    /// Whether the part is in a network that is running.
    pub fn running(&self, pos: Pos) -> bool {
        self.speed(pos).is_some_and(|(_, paused)| !paused)
    }

    /// What the tachometer and the wrench overlay show for a part's network.
    pub fn reading(&self, pos: Pos) -> Option<Reading> {
        let p = self.parts.get(&pos)?;
        let Some(s) = p.sys else {
            return Some(Reading {
                rpm: p.omega,
                paused: true,
                ..Reading::default()
            });
        };
        let sys = self.sys(s);
        let net = self.nets[sys.net].as_ref()?;
        let r = sys.members[p.member].r.abs().max(1e-9);
        let w = sys.omega;
        let mut give = 0.0;
        let mut stall = 0.0;
        for &i in &sys.sources {
            let m = &sys.members[i];
            let d = self.parts[&m.pos].drive;
            let local = m.r * w;
            let (a, b) = solve::source_line(d.stall, d.top, d.dir, local);
            give += m.r * (a - b * local);
            stall += d.stall * m.r.abs();
        }
        if w < 0.0 {
            give = -give;
        }
        let mut drawn = sys.c_static + (sys.b_static * w).abs();
        let mut min_ref: Option<f64> = None;
        for &i in &sys.machines {
            let m = &sys.members[i];
            let part = &self.parts[&m.pos];
            drawn += part.load * m.r.abs();
            if part.load > 0.0 {
                let need = GRINDER_MIN_RPM / m.r.abs().max(1e-9);
                min_ref = Some(min_ref.map_or(need, |v: f64| v.max(need)));
            }
        }
        let parts = net.systems.iter().map(|s| self.sys(*s).members.len()).sum();
        Some(Reading {
            rpm: sys.members[p.member].r * w,
            paused: net.paused,
            red_line: sys.red_ref.is_finite().then_some(sys.red_ref * r),
            min_working: min_ref.map(|v| v * r),
            drawn: drawn / r,
            give: give.max(0.0) / r,
            stall: stall / r,
            parts,
        })
    }

    /// Parts whose warning timers are filling: position, whether it is
    /// overspeed (else overload), and the warning level (1 to 3).
    pub fn warned(&self) -> Vec<(Pos, bool, u8)> {
        self.parts
            .iter()
            .filter(|(_, p)| p.loaded && p.jam.is_none())
            .filter_map(|(pos, p)| {
                let over = p.over_t >= p.shear_t;
                let level = p.warning();
                (level > 0).then_some((*pos, over, level))
            })
            .collect()
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Parts whose client-visible state changed enough to send (spec 10.6):
    /// speed by more than max(0.5 RPM, 3%), or warning, sag or jam. Marks
    /// them sent. Paused networks keep what clients last got.
    pub fn take_sync(&mut self) -> Vec<(Pos, Sent)> {
        let mut out = Vec::new();
        for s in 0..self.systems.len() {
            let Some(sys) = self.systems[s].as_ref() else {
                continue;
            };
            let paused = self.nets[sys.net].as_ref().is_none_or(|n| n.paused);
            let moved = !paused && changed_enough(sys.sent_omega, sys.omega);
            let w = sys.omega;
            let check_warn = sys.warn_active || sys.shear_active;
            let members: Vec<(Pos, f64)> = sys.members.iter().map(|m| (m.pos, m.r)).collect();
            if moved {
                self.systems[s].as_mut().unwrap().sent_omega = w;
            }
            for (pos, r) in members {
                let p = self.parts.get_mut(&pos).unwrap();
                let rpm = if moved { r * w } else { p.sent.rpm };
                let next = Sent {
                    rpm,
                    warn: if check_warn || p.sent.warn > 0 {
                        p.warning()
                    } else {
                        0
                    },
                    sag: p.sag,
                    jam: p.jam,
                    fresh: false,
                };
                let old = Sent {
                    fresh: false,
                    ..p.sent
                };
                if next != old || p.sent.fresh {
                    p.sent = next;
                    p.omega = r * w;
                    out.push((pos, next));
                }
            }
        }
        // Jammed parts sit in no system: they stop.
        for (pos, p) in self.parts.iter_mut() {
            if p.sys.is_none() && p.jam.is_some() && (p.sent.jam != p.jam || p.sent.rpm != 0.0) {
                p.omega = 0.0;
                p.sent = Sent {
                    rpm: 0.0,
                    warn: 0,
                    sag: p.sag,
                    jam: p.jam,
                    fresh: false,
                };
                out.push((*pos, p.sent));
            }
        }
        out
    }

    // ---- The tick --------------------------------------------------------

    /// Rebuild what changed, then run every running network one tick.
    pub fn tick(&mut self, view: &dyn View) {
        self.ticks += 1;
        self.rebuild(view);
        if self.ticks.is_multiple_of(10) {
            self.refresh_pause(view);
        }
        for n in 0..self.nets.len() {
            if self.nets[n].as_ref().is_some_and(|n| !n.paused) {
                self.tick_net(n);
            }
        }
    }

    /// Check every network against the simulated area (spec 10.4).
    pub fn refresh_pause(&mut self, view: &dyn View) {
        for n in 0..self.nets.len() {
            let Some(net) = self.nets[n].as_ref() else {
                continue;
            };
            let frontier: Vec<Pos> = net
                .frontier
                .iter()
                .copied()
                .filter(|q| !view.loaded(*q) || self.parts.contains_key(q))
                .collect();
            let unloaded_member = self.net_members(n).any(|q| !self.parts[&q].loaded);
            let outside = net.chunks.iter().any(|c| !view.simulated(*c));
            let paused = unloaded_member || outside || frontier.iter().any(|q| !view.loaded(*q));
            let net = self.nets[n].as_mut().unwrap();
            net.frontier = frontier;
            net.paused = paused;
        }
    }

    fn tick_net(&mut self, n: usize) {
        let net = self.nets[n].as_ref().unwrap();
        let systems = net.systems.clone();
        let slips = net.slips.clone();
        // Slipping belts pass their limit from the faster side to the slower.
        let mut extra: HashMap<usize, f64> = HashMap::new();
        for belt in &slips {
            let (wa, wb) = (self.local(belt.a), self.local(belt.b));
            let s = wb - belt.k * wa;
            if s == 0.0 {
                continue;
            }
            let force = BELT_LIMIT / belt.da.min(belt.db);
            let cross = belt.k.signum();
            let tb = -s.signum() * force * belt.db;
            let ta = s.signum() * cross * force * belt.da;
            for (pos, tau) in [(belt.a, ta), (belt.b, tb)] {
                let p = &self.parts[&pos];
                if let Some(si) = p.sys {
                    let r = self.sys(si).members[p.member].r;
                    *extra.entry(si).or_default() += r * tau;
                }
            }
        }
        let mut jams: Vec<(Pos, Jam)> = Vec::new();
        let mut slip_now: Vec<BeltKey> = Vec::new();
        for &s in &systems {
            let mut terms = Terms {
                a: 0.0,
                b: self.sys(s).b_static,
                c: self.sys(s).c_static,
            };
            let mut inputs = 0.0;
            {
                let sys = self.sys(s);
                for &i in &sys.sources {
                    let m = &sys.members[i];
                    let d = self.parts[&m.pos].drive;
                    let (a, b) = solve::source_line(d.stall, d.top, d.dir, m.r * sys.omega);
                    terms.a += m.r * a;
                    terms.b += m.r * m.r * b;
                    inputs += d.stall * d.dir;
                }
                for &i in &sys.machines {
                    let m = &sys.members[i];
                    let load = self.parts[&m.pos].load;
                    terms.c += m.r.abs() * load;
                    inputs += load;
                }
            }
            let with_slip = Terms {
                a: terms.a + extra.get(&s).copied().unwrap_or(0.0),
                ..terms
            };
            let ticks = self.ticks;
            let sys = self.systems[s].as_mut().unwrap();
            let next = solve::step(sys.omega, sys.inertia, with_slip, DT);
            sys.alpha = (next - sys.omega) / DT;
            sys.omega = next;
            sys.terms = terms;
            // Torques follow the inputs, the speed and the acceleration.
            let changed = (inputs - sys.flow_inputs).abs()
                > FLOW_CHANGE * sys.flow_inputs.abs().max(1.0)
                || (next - sys.flow_omega).abs() > FLOW_CHANGE * sys.flow_omega.abs().max(1.0);
            let due = sys.flow_due || (changed && ticks - sys.last_flow >= FLOW_EVERY);
            if due {
                sys.flow_due = false;
                sys.last_flow = ticks;
                sys.flow_inputs = inputs;
                sys.flow_omega = next;
                self.compute_flows(s, &mut slip_now);
            }
            self.tick_damage(s, &mut jams);
        }
        // Belts that slipped become separate systems; slipping belts that
        // matched speeds grip again.
        for key in slip_now {
            if self.slipping.insert(key) {
                self.dirty.insert(key.0);
                self.dirty.insert(key.1);
                self.events.push(Event::Slipped(key.0, key.1));
            }
        }
        for belt in &slips {
            if self.regrip_ok(belt) {
                let key = belt_key(belt.a, belt.b);
                if self.slipping.remove(&key) {
                    self.dirty.insert(belt.a);
                    self.dirty.insert(belt.b);
                    self.events.push(Event::Gripped(belt.a, belt.b));
                }
            }
        }
        for (pos, jam) in jams {
            self.jam(pos, jam);
        }
    }

    /// The local speed of a part in a built system.
    fn local(&self, pos: Pos) -> f64 {
        let p = &self.parts[&pos];
        match p.sys {
            Some(s) => self.sys(s).members[p.member].r * self.sys(s).omega,
            None => p.omega,
        }
    }

    fn tick_damage(&mut self, s: usize, jams: &mut Vec<(Pos, Jam)>) {
        let sys = self.systems[s].as_ref().unwrap();
        let w = sys.omega.abs();
        if w > sys.cap_ref {
            for m in &sys.members {
                if (m.r * sys.omega).abs() > HARD_CAP_RPM {
                    jams.push((m.pos, Jam::Overspeed));
                }
            }
        }
        if w > sys.red_ref || sys.warn_active {
            let mut active = false;
            for &i in &sys.limited {
                let m = &sys.members[i];
                let p = self.parts.get_mut(&m.pos).unwrap();
                let max = p.spec.max_rpm().unwrap_or(f64::INFINITY);
                p.over_t = damage::tick_timer(p.over_t, m.r * sys.omega, max, DT);
                if p.over_t >= 1.0 {
                    jams.push((m.pos, Jam::Overspeed));
                }
                active |= p.over_t > 0.0;
            }
            self.systems[s].as_mut().unwrap().warn_active = active;
        }
        let sys = self.systems[s].as_ref().unwrap();
        if sys.shear_active {
            let mut active = false;
            for &i in &sys.segments {
                let m = &sys.members[i];
                let p = self.parts.get_mut(&m.pos).unwrap();
                let limit = p.torque_limit().unwrap_or(f64::INFINITY);
                p.shear_t = damage::tick_timer(p.shear_t, p.torque, limit, DT);
                if p.shear_t >= 1.0 {
                    jams.push((m.pos, Jam::Shear));
                }
                active |= p.shear_t > 0.0 || p.torque > limit;
            }
            self.systems[s].as_mut().unwrap().shear_active = active;
        }
    }

    /// Torque flow through every joint of a system (spec 6.3).
    fn compute_flows(&mut self, s: usize, slip_now: &mut Vec<BeltKey>) {
        let sys = self.systems[s].as_ref().unwrap();
        let w = sys.omega;
        let n = sys.members.len();
        let mut t = vec![0.0; n];
        let sign = if w > 0.0 {
            1.0
        } else if w < 0.0 {
            -1.0
        } else {
            0.0
        };
        for (i, m) in sys.members.iter().enumerate() {
            let p = &self.parts[&m.pos];
            let local = m.r * w;
            let mut tau =
                -m.c * local - CONST_FRICTION * local.signum() * (local != 0.0) as u8 as f64;
            if p.spec.is_source() {
                let (a, b) = solve::source_line(p.drive.stall, p.drive.top, p.drive.dir, local);
                tau += a - b * local;
            }
            if p.spec.is_machine() {
                tau -= p.load * if local != 0.0 { local.signum() } else { 0.0 };
            }
            // Referred to the reference, minus what accelerates the part.
            t[i] = m.r * tau - m.inertia * m.r * m.r * sys.alpha / ACCEL;
        }
        // At standstill the loads hold against the push: they take what the
        // push gives, up to their size (stiction).
        if sign == 0.0 {
            let push: f64 = t.iter().sum();
            if push.abs() > 0.0 {
                let mut holders: Vec<usize> = sys.machines.clone();
                if holders.is_empty() {
                    holders = (0..n).collect();
                }
                let share = push / holders.len() as f64;
                for i in holders {
                    t[i] -= share;
                }
            }
        }
        let f = flow::flows(n, &sys.edges, &t);
        let mut carried = vec![0.0f64; n];
        for (e, &(a, b)) in sys.edges.iter().enumerate() {
            carried[a] = carried[a].max(f[e].abs());
            carried[b] = carried[b].max(f[e].abs());
        }
        let mut shear = false;
        let members: Vec<(Pos, f64)> = sys.members.iter().map(|m| (m.pos, m.r)).collect();
        for (i, (pos, r)) in members.iter().enumerate() {
            let p = self.parts.get_mut(pos).unwrap();
            p.torque = carried[i] / r.abs().max(1e-9);
            if let Some(limit) = p.torque_limit() {
                shear |= p.torque > limit || p.shear_t > 0.0;
            }
        }
        let sys = self.systems[s].as_mut().unwrap();
        sys.shear_active = shear;
        for &(e, belt) in &sys.belts {
            let pa = &self.parts[&belt.a];
            let pb = &self.parts[&belt.b];
            let ra = sys.members[pa.member].r.abs().max(1e-9);
            let rb = sys.members[pb.member].r.abs().max(1e-9);
            // Torque at each pulley; the limit is at the smaller one.
            let small = if belt.da <= belt.db {
                f[e].abs() / ra
            } else {
                f[e].abs() / rb
            };
            if small > BELT_LIMIT {
                slip_now.push(belt_key(belt.a, belt.b));
            }
        }
    }

    /// A slipping belt grips again when its two sides match speed and what it
    /// would carry is under its limit.
    fn regrip_ok(&self, belt: &Belt) -> bool {
        let (wa, wb) = (self.local(belt.a), self.local(belt.b));
        if (wb - belt.k * wa).abs() >= REGRIP_RPM {
            return false;
        }
        let (Some(sa), Some(sb)) = (self.parts[&belt.a].sys, self.parts[&belt.b].sys) else {
            return false;
        };
        if sa == sb {
            return true;
        }
        let (a, b) = (self.sys(sa), self.sys(sb));
        let ra = a.members[self.parts[&belt.a].member].r;
        let rb = b.members[self.parts[&belt.b].member].r;
        // Joined, in pulley b's local units x: ω_sb = x / rb, ω_sa = x / (k·ra).
        let ka = belt.k * ra;
        let inertia = b.inertia / (rb * rb) + a.inertia / (ka * ka);
        let torque = b.terms.torque(b.omega) / rb + a.terms.torque(a.omega) / ka;
        let alpha_x = ACCEL * torque / inertia;
        // What b's system needs from the belt, at pulley b.
        let need_ref = b.inertia * (alpha_x / rb) / ACCEL - b.terms.torque(b.omega);
        let at_b = (need_ref / rb).abs();
        let at_small = if belt.db <= belt.da {
            at_b
        } else {
            at_b * belt.da / belt.db
        };
        at_small < BELT_LIMIT
    }

    fn jam(&mut self, pos: Pos, jam: Jam) {
        let Some(p) = self.parts.get_mut(&pos) else {
            return;
        };
        if p.jam.is_some() {
            return;
        }
        p.jam = Some(jam);
        p.over_t = 0.0;
        p.shear_t = 0.0;
        self.events.push(Event::Jammed(pos, jam));
        self.dissolve_at(pos);
        self.dirty.insert(pos);
    }

    // ---- Building --------------------------------------------------------

    fn sys(&self, s: usize) -> &System {
        self.systems[s].as_ref().expect("live system")
    }

    fn net_members(&self, n: usize) -> impl Iterator<Item = Pos> + '_ {
        self.nets[n]
            .iter()
            .flat_map(|net| net.systems.iter())
            .flat_map(|s| self.sys(*s).members.iter().map(|m| m.pos))
    }

    /// Dissolve the network holding `pos`, remembering every member's speed.
    fn dissolve_at(&mut self, pos: Pos) {
        let Some(s) = self.parts.get(&pos).and_then(|p| p.sys) else {
            return;
        };
        let n = self.sys(s).net;
        self.dissolve_net(n);
    }

    fn dissolve_net(&mut self, n: usize) {
        let Some(net) = self.nets[n].take() else {
            return;
        };
        for s in net.systems {
            let sys = self.systems[s].take().unwrap();
            self.old_ids += 1;
            let id = self.old_ids;
            for m in sys.members {
                let w = m.r * sys.omega;
                if let Some(p) = self.parts.get_mut(&m.pos) {
                    p.omega = w;
                    p.sys = None;
                }
                self.old.insert(m.pos, (id, w));
                self.dirty.insert(m.pos);
            }
        }
    }

    fn drop_net(&mut self, n: usize) {
        if let Some(net) = self.nets[n].take() {
            for s in net.systems {
                if let Some(sys) = self.systems[s].take() {
                    for m in sys.members {
                        if let Some(p) = self.parts.get_mut(&m.pos) {
                            p.sys = None;
                        }
                    }
                }
            }
        }
    }

    /// Rebuild the networks around everything that changed (spec 4.2).
    pub fn rebuild(&mut self, view: &dyn View) {
        if self.dirty.is_empty() {
            return;
        }
        self.rebuilds += 1;
        // Neighbours of a change may have joined or split from it.
        let dirty = std::mem::take(&mut self.dirty);
        let mut seeds: BTreeSet<Pos> = BTreeSet::new();
        for &pos in &dirty {
            seeds.insert(pos);
            for d in Dir::ALL {
                seeds.insert(pos.offset(d));
            }
            if let Some(b) = self.parts.get(&pos).and_then(|p| p.spec.belt) {
                seeds.insert(b.partner);
            }
        }
        for &pos in &seeds {
            self.dissolve_at(pos);
        }
        // dissolve_at may have marked more members dirty.
        let mut place: BTreeSet<Pos> = std::mem::take(&mut self.dirty);
        place.extend(seeds.into_iter().filter(|p| self.parts.contains_key(p)));
        place.retain(|p| self.parts.contains_key(p));
        while let Some(start) = place.pop_first() {
            if self.parts[&start].sys.is_some() {
                continue;
            }
            if self.parts[&start].jam.is_some() {
                self.parts.get_mut(&start).unwrap().omega = 0.0;
                continue;
            }
            let members = self.build_network(start, view);
            for m in members {
                place.remove(&m);
            }
        }
        self.old.clear();
        self.causes.clear();
    }

    fn belt_of(&self, pos: Pos) -> Option<Belt> {
        let p = self.parts.get(&pos)?;
        if p.jam.is_some() {
            return None;
        }
        let end = p.spec.belt?;
        let q = self.parts.get(&end.partner)?;
        if q.jam.is_some() {
            return None;
        }
        let back = q.spec.belt?;
        if back.partner != pos || back.crossed != end.crossed {
            return None;
        }
        let da = p.spec.pulley_diameter()?;
        let db = q.spec.pulley_diameter()?;
        let cross = if end.crossed { -1.0 } else { 1.0 };
        Some(Belt {
            a: pos,
            b: end.partner,
            k: cross * da / db,
            da,
            db,
        })
    }

    /// The rigid links of a part: (neighbour, ratio ω_n / ω_self, belt).
    fn links(&self, pos: Pos) -> Vec<(Pos, f64, Option<Belt>)> {
        let mut out = Vec::new();
        let p = &self.parts[&pos];
        for (face, gain) in p.ports() {
            let q = pos.offset(face);
            if let Some(other) = self.parts.get(&q)
                && let Some((_, g2)) = other
                    .ports()
                    .into_iter()
                    .find(|(f, _)| *f == face.opposite())
            {
                out.push((q, gain / g2, None));
            }
        }
        if let Some(belt) = self.belt_of(pos) {
            out.push((belt.b, belt.k, Some(belt)));
        }
        out
    }

    /// Build the network that holds `start`; returns its members.
    fn build_network(&mut self, start: Pos, view: &dyn View) -> Vec<Pos> {
        loop {
            if self.parts[&start].jam.is_some() {
                self.parts.get_mut(&start).unwrap().omega = 0.0;
                return vec![start];
            }
            // Every part reachable through ports and belts.
            let mut all: BTreeSet<Pos> = BTreeSet::new();
            let mut queue = VecDeque::from([start]);
            all.insert(start);
            while let Some(pos) = queue.pop_front() {
                for (q, _, _) in self.links(pos) {
                    if self.parts[&q].sys.is_some() {
                        let n = self.sys(self.parts[&q].sys.unwrap()).net;
                        self.dissolve_net(n);
                    }
                    if all.insert(q) {
                        queue.push_back(q);
                    }
                }
            }
            // Split into systems: belts that slip don't link rigidly.
            let mut systems: Vec<Vec<Pos>> = Vec::new();
            let mut assigned: HashSet<Pos> = HashSet::new();
            for &pos in &all {
                if assigned.contains(&pos) {
                    continue;
                }
                let mut comp = vec![pos];
                assigned.insert(pos);
                let mut i = 0;
                while i < comp.len() {
                    let at = comp[i];
                    i += 1;
                    for (q, _, belt) in self.links(at) {
                        if let Some(b) = belt
                            && self.slipping.contains(&belt_key(b.a, b.b))
                        {
                            continue;
                        }
                        if assigned.insert(q) {
                            comp.push(q);
                        }
                    }
                }
                comp.sort();
                systems.push(comp);
            }
            // Ratios; an inconsistent loop jams a part and starts again.
            let mut built = Vec::new();
            let mut conflict = None;
            for comp in &systems {
                match self.ratios(comp) {
                    Ok(r) => built.push(r),
                    Err(culprit) => {
                        conflict = Some(culprit);
                        break;
                    }
                }
            }
            if let Some(culprit) = conflict {
                self.force_jam(culprit, Jam::Conflict);
                continue;
            }
            // Momentum: the inertia-weighted average of the parts' groups.
            let mut shock = None;
            let mut speeds = Vec::new();
            for (comp, (ratios, _, _)) in systems.iter().zip(&built) {
                let (w, shocked) = self.joined_speed(comp, ratios);
                if shocked.is_some() {
                    shock = shocked;
                    break;
                }
                speeds.push(w);
            }
            if let Some(culprit) = shock {
                self.force_jam(culprit, Jam::Shock);
                continue;
            }
            let members: Vec<Pos> = all.iter().copied().collect();
            self.install(systems, built, speeds, view);
            return members;
        }
    }

    /// Jam a part during a build (it has no system yet).
    fn force_jam(&mut self, pos: Pos, jam: Jam) {
        if let Some(p) = self.parts.get_mut(&pos) {
            p.jam = Some(jam);
            p.over_t = 0.0;
            p.shear_t = 0.0;
            self.events.push(Event::Jammed(pos, jam));
        }
    }

    /// Ratios relative to the lowest position, the edges and gripping belts;
    /// or the part to jam for an inconsistent loop.
    #[allow(clippy::type_complexity)]
    fn ratios(
        &self,
        comp: &[Pos],
    ) -> Result<(Vec<f64>, Vec<(usize, usize)>, Vec<(usize, Belt)>), Pos> {
        let index: HashMap<Pos, usize> = comp.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let mut r: Vec<Option<f64>> = vec![None; comp.len()];
        let mut parent: Vec<Option<usize>> = vec![None; comp.len()];
        let mut edges = Vec::new();
        let mut belts = Vec::new();
        let mut seen_edges: HashSet<(usize, usize)> = HashSet::new();
        r[0] = Some(1.0);
        let mut queue = VecDeque::from([0usize]);
        while let Some(i) = queue.pop_front() {
            let ri = r[i].unwrap();
            for (q, k, belt) in self.links(comp[i]) {
                if let Some(b) = belt
                    && self.slipping.contains(&belt_key(b.a, b.b))
                {
                    continue;
                }
                let Some(&j) = index.get(&q) else {
                    continue;
                };
                let key = (i.min(j), i.max(j));
                if seen_edges.insert(key) {
                    edges.push((i, j));
                    if let Some(b) = belt {
                        belts.push((edges.len() - 1, b));
                    }
                }
                let want = k * ri;
                match r[j] {
                    None => {
                        r[j] = Some(want);
                        parent[j] = Some(i);
                        queue.push_back(j);
                    }
                    Some(have) => {
                        if (have - want).abs() > 1e-6 * have.abs().max(want.abs()).max(1.0) {
                            return Err(self.loop_culprit(comp, &parent, i, j));
                        }
                    }
                }
            }
        }
        Ok((
            r.into_iter().map(|v| v.unwrap_or(1.0)).collect(),
            edges,
            belts,
        ))
    }

    /// The part that closed the loop: a part changed by a player if one is
    /// in the loop, else the one with the lowest speed limit (then position).
    fn loop_culprit(&self, comp: &[Pos], parent: &[Option<usize>], i: usize, j: usize) -> Pos {
        let path = |mut v: usize| {
            let mut out = vec![v];
            while let Some(p) = parent[v] {
                out.push(p);
                v = p;
            }
            out
        };
        let (pi, pj) = (path(i), path(j));
        let on_j: HashSet<usize> = pj.iter().copied().collect();
        let mut cycle: Vec<usize> = Vec::new();
        for &v in &pi {
            cycle.push(v);
            if on_j.contains(&v) {
                let lca = v;
                for &w in &pj {
                    if w == lca {
                        break;
                    }
                    cycle.push(w);
                }
                break;
            }
        }
        let positions: Vec<Pos> = cycle.iter().map(|v| comp[*v]).collect();
        if let Some(c) = positions.iter().filter(|p| self.causes.contains(p)).min() {
            return *c;
        }
        *positions
            .iter()
            .min_by(|a, b| {
                let ra = self.parts[a].spec.loop_rank();
                let rb = self.parts[b].spec.loop_rank();
                ra.partial_cmp(&rb).unwrap().then(a.cmp(b))
            })
            .unwrap()
    }

    /// The speed a new system starts at, and a part to jam if two moving
    /// systems joined turning opposite ways (spec 6.5).
    fn joined_speed(&self, comp: &[Pos], ratios: &[f64]) -> (f64, Option<Pos>) {
        // Group parts by the system they came from; parts that had none
        // (placed or loaded) are groups of their own.
        let mut groups: BTreeMap<usize, (f64, f64)> = BTreeMap::new(); // id -> (Σ I r², Σ I r² ω_ref)
        let mut loose_i = 0.0;
        let mut loose_m = 0.0;
        let mut cause_here = None;
        for (pos, &r) in comp.iter().zip(ratios) {
            let p = &self.parts[pos];
            let weight = p.spec.inertia() * r * r;
            if self.causes.contains(pos) && cause_here.is_none() {
                cause_here = Some(*pos);
            }
            match self.old.get(pos) {
                Some(&(id, w)) => {
                    let g = groups.entry(id).or_default();
                    g.0 += weight;
                    g.1 += weight * (w / r);
                }
                None => {
                    // Placed parts stand still; loaded ones bring their speed.
                    loose_i += weight;
                    if !self.causes.contains(pos) {
                        loose_m += weight * (p.omega / r);
                    }
                }
            }
        }
        if let Some(cause) = cause_here {
            let moving: Vec<f64> = groups
                .values()
                .map(|(i, m)| m / i)
                .filter(|w| w.abs() > SHOCK_RPM)
                .collect();
            if moving.iter().any(|w| *w > 0.0) && moving.iter().any(|w| *w < 0.0) {
                return (0.0, Some(cause));
            }
        }
        let total_i: f64 = groups.values().map(|g| g.0).sum::<f64>() + loose_i;
        let total_m: f64 = groups.values().map(|g| g.1).sum::<f64>() + loose_m;
        if total_i <= 0.0 {
            return (0.0, None);
        }
        (total_m / total_i, None)
    }

    #[allow(clippy::type_complexity)]
    fn install(
        &mut self,
        systems: Vec<Vec<Pos>>,
        built: Vec<(Vec<f64>, Vec<(usize, usize)>, Vec<(usize, Belt)>)>,
        speeds: Vec<f64>,
        view: &dyn View,
    ) {
        let n = self
            .nets
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.nets.push(None);
                self.nets.len() - 1
            });
        let mut net = Network {
            systems: Vec::new(),
            chunks: BTreeSet::new(),
            frontier: Vec::new(),
            slips: Vec::new(),
            paused: false,
        };
        // Spans first: sag changes friction.
        let all: Vec<Pos> = systems.iter().flatten().copied().collect();
        self.compute_spans(&all);
        let slipping_pulleys: HashSet<Pos> = all
            .iter()
            .filter_map(|p| self.belt_of(*p))
            .filter(|b| self.slipping.contains(&belt_key(b.a, b.b)))
            .flat_map(|b| [b.a, b.b])
            .collect();
        for ((comp, (ratios, edges, belts)), omega) in systems.into_iter().zip(built).zip(speeds) {
            let s = self
                .systems
                .iter()
                .position(Option::is_none)
                .unwrap_or_else(|| {
                    self.systems.push(None);
                    self.systems.len() - 1
                });
            let mut sys = System {
                members: Vec::with_capacity(comp.len()),
                edges,
                belts,
                omega,
                inertia: 0.0,
                b_static: 0.0,
                c_static: 0.0,
                sources: Vec::new(),
                machines: Vec::new(),
                limited: Vec::new(),
                segments: Vec::new(),
                red_ref: f64::INFINITY,
                cap_ref: f64::INFINITY,
                net: n,
                flow_due: true,
                last_flow: 0,
                flow_inputs: 0.0,
                flow_omega: 0.0,
                alpha: 0.0,
                terms: Terms::default(),
                warn_active: false,
                shear_active: false,
                sent_omega: f64::NAN,
            };
            let mut max_r: f64 = 0.0;
            for (i, (pos, r)) in comp.iter().zip(ratios).enumerate() {
                let p = self.parts.get_mut(pos).unwrap();
                p.sys = Some(s);
                p.member = i;
                let mut c = p.spec.friction();
                if p.sag {
                    c *= 2.0;
                }
                if slipping_pulleys.contains(pos) {
                    c += SLIP_FRICTION;
                }
                let inertia = p.spec.inertia();
                sys.inertia += inertia * r * r;
                sys.b_static += c * r * r;
                sys.c_static += CONST_FRICTION * r.abs();
                if p.spec.is_source() {
                    sys.sources.push(i);
                }
                if p.spec.is_machine() {
                    sys.machines.push(i);
                }
                if let Some(max) = p.spec.max_rpm() {
                    sys.limited.push(i);
                    sys.red_ref = sys.red_ref.min(max / r.abs());
                }
                if p.spec.torque_limit().is_some() {
                    sys.segments.push(i);
                }
                sys.warn_active |= p.over_t > 0.0;
                sys.shear_active |= p.shear_t > 0.0;
                max_r = max_r.max(r.abs());
                p.sent.fresh |= p.sent.jam != p.jam;
                net.chunks.insert(pos.chunk());
                sys.members.push(Member {
                    pos: *pos,
                    r,
                    inertia,
                    c,
                });
            }
            sys.cap_ref = HARD_CAP_RPM / max_r.max(1e-9);
            sys.sent_omega = f64::NAN;
            self.systems[s] = Some(sys);
            net.systems.push(s);
        }
        // Slipping belts between the systems; the frontier into unloaded chunks.
        let mut slips: BTreeMap<BeltKey, Belt> = BTreeMap::new();
        for &pos in &all {
            if let Some(b) = self.belt_of(pos) {
                let key = belt_key(b.a, b.b);
                if self.slipping.contains(&key) {
                    slips.entry(key).or_insert(b);
                }
            }
            if let Some(end) = self.parts[&pos].spec.belt
                && !self.parts.contains_key(&end.partner)
                && !view.loaded(end.partner)
            {
                net.frontier.push(end.partner);
                net.chunks.insert(end.partner.chunk());
            }
            for (face, _) in self.parts[&pos].ports() {
                let q = pos.offset(face);
                if !self.parts.contains_key(&q) && !view.loaded(q) {
                    net.frontier.push(q);
                    net.chunks.insert(q.chunk());
                }
            }
        }
        net.slips = slips.into_values().collect();
        let unloaded = all.iter().any(|p| !self.parts[p].loaded);
        net.paused =
            unloaded || !net.frontier.is_empty() || net.chunks.iter().any(|c| !view.simulated(*c));
        self.nets[n] = Some(net);
    }

    /// Mark sagging spans (spec 5).
    fn compute_spans(&mut self, members: &[Pos]) {
        let set: HashSet<Pos> = members.iter().copied().collect();
        let in_span = |m: &Mechanics, p: Pos| {
            m.parts
                .get(&p)
                .is_some_and(|s| s.spec.in_span() && s.jam.is_none())
        };
        let mut done: HashSet<Pos> = HashSet::new();
        for &pos in members {
            self.parts.get_mut(&pos).unwrap().sag = false;
            self.parts.get_mut(&pos).unwrap().span = None;
        }
        for &pos in members {
            if done.contains(&pos) || !in_span(self, pos) {
                continue;
            }
            let Some(axis) = self.parts[&pos].spec.axis() else {
                continue;
            };
            // Walk back to the start of the run, then forward over it.
            let linked = |m: &Mechanics, a: Pos, b: Pos| {
                set.contains(&b)
                    && in_span(m, b)
                    && m.parts[&b].spec.axis() == Some(axis)
                    && m.links(a)
                        .iter()
                        .any(|(q, _, belt)| *q == b && belt.is_none())
            };
            let mut first = pos;
            while linked(self, first, first.offset(axis.negative())) {
                first = first.offset(axis.negative());
            }
            let mut run = vec![first];
            let mut at = first;
            while linked(self, at, at.offset(axis.positive())) {
                at = at.offset(axis.positive());
                run.push(at);
            }
            let limit = run
                .iter()
                .filter_map(|p| self.parts[p].spec.span_limit())
                .min()
                .unwrap_or(16);
            let sag = run.len() > limit;
            for p in &run {
                done.insert(*p);
                let st = self.parts.get_mut(p).unwrap();
                st.sag = sag;
                st.span = Some((run.len(), limit));
            }
        }
    }
}

fn changed_enough(sent: f64, now: f64) -> bool {
    if sent.is_nan() {
        return true;
    }
    if (sent == 0.0) != (now == 0.0) {
        return true;
    }
    (now - sent).abs() > (0.03 * sent.abs()).max(0.5)
}
