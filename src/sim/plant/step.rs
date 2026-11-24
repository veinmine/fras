//! The steam step (steam and heat spec 14.1), every 4 ticks after the
//! rotational solve: rebuild what changed, check which plants pause, heat
//! the boilers, feed them water, route and deliver steam, update pressures,
//! timers and failures, then hand each engine its stall torque and top speed
//! and each pump its load.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::build::Dsu;
use super::*;
use crate::sim::fluid::{pump, route, valve};
use crate::sim::rot::View;
use crate::sim::steam::boiler::Boiler;
use crate::sim::steam::{self, AMBIENT, DT, VENT_RATE, engine, fail};

/// The warning timer kinds of pipe parts on a steam network.
pub(crate) fn timer_kind(kind: Kind) -> Option<u8> {
    Some(match kind {
        Kind::Pipe(PipeMat::Copper) => 0,
        Kind::Pipe(PipeMat::Iron) => 1,
        Kind::Valve { .. } => 2,
        Kind::Vent { .. } => 3,
        _ => return None,
    })
}

/// Water takers: shell and the nodes it attaches at.
type Takers = Vec<(usize, Vec<usize>)>;

/// A steam consumer this step.
#[derive(Copy, Clone, Debug)]
enum Taker {
    Cylinder(Pos),
    Vent(Pos),
    /// A burst pipe part, by position.
    Burst(Pos),
}

impl Works {
    /// Run one steam step.
    pub fn step(&mut self, view: &dyn View) {
        self.rebuild(view);
        self.steps += 1;
        self.refresh_pause(view);
        self.heat_boilers();
        self.feed_water();
        self.route_steam();
        self.check_failures();
        self.set_drives();
        self.sync_shells();
    }

    // ---- Pausing (spec 2, 14.2) ------------------------------------------------

    /// Work out which plants pause: any part not simulated or not loaded, or
    /// a rotational network linked to it paused by its own rules; plants
    /// sharing a rotational network pause together.
    pub fn refresh_pause(&mut self, view: &dyn View) {
        let n = self.plants.len();
        let mut own: Vec<bool> = self
            .plants
            .iter()
            .map(|p| {
                p.partial
                    || p.chunks.iter().any(|c| !view.simulated(*c))
                    || p.frontier.iter().any(|q| !view.loaded(*q))
            })
            .collect();
        // Plants joined through rotational networks.
        let mut dsu = Dsu::new(n);
        let mut first: BTreeMap<usize, usize> = BTreeMap::new();
        for (pi, p) in self.plants.iter().enumerate() {
            for q in &p.rot_parts {
                let Some(r) = self.rot.get(q) else { continue };
                if r.paused {
                    own[pi] = true;
                }
                if let Some(net) = r.net {
                    let f = *first.entry(net).or_insert(pi);
                    dsu.union(f, pi);
                }
            }
        }
        let mut group_paused: HashMap<usize, bool> = HashMap::new();
        for (pi, &o) in own.iter().enumerate() {
            let r = dsu.find(pi);
            *group_paused.entry(r).or_insert(false) |= o;
        }
        self.held.clear();
        for pi in 0..n {
            let paused = group_paused[&dsu.find(pi)];
            self.plants[pi].paused = paused;
            if paused {
                self.held.extend(self.plants[pi].rot_parts.iter().copied());
            }
        }
    }

    fn plant_paused(&self, plant: usize) -> bool {
        self.plants.get(plant).is_none_or(|p| p.paused)
    }

    /// The local speed of a flywheel or pump, RPM.
    fn omega(&self, pos: Pos) -> f64 {
        self.rot.get(&pos).map_or(0.0, |r| r.omega)
    }

    // ---- Heat (spec 2, 4.1 steps 1 to 4) ----------------------------------------

    fn heat_boilers(&mut self) {
        let mut heat_in: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
        for (spot, boilers) in &self.heats {
            let kw = self.heat.get(spot).copied().unwrap_or(0.0);
            if kw <= 0.0 || boilers.is_empty() {
                continue;
            }
            let share = kw / boilers.len() as f64;
            for &b in boilers {
                let e = heat_in.entry(b).or_default();
                e.0 += share;
                e.1 += 1;
            }
        }
        for si in 0..self.shells.len() {
            let s = &self.shells[si];
            if !s.is_boiler() || !s.works() || s.partial || self.plant_paused(s.plant) {
                continue;
            }
            let (kw, fires) = heat_in.get(&si).copied().unwrap_or_default();
            let mut b = Boiler {
                blocks: s.n(),
                water: s.water,
                temp: s.temp,
            };
            let boiled = b.heat(kw, DT);
            let pool = s.pool;
            let s = &mut self.shells[si];
            s.water = b.water;
            s.temp = b.temp;
            s.heat = kw;
            s.fireboxes = fires;
            s.boiled = boiled / DT;
            s.fed = 0.0;
            if let Some(p) = pool {
                self.steam[p].pool.steam += boiled;
                self.steam[p].steam_in = boiled / DT;
            }
        }
    }

    // ---- Water (fluids spec 5.2) --------------------------------------------------

    fn feed_water(&mut self) {
        self.pump_flow.clear();
        for ni in 0..self.nets.len() {
            let net = &self.nets[ni];
            if net.content != Some(Content::Water) || net.mixed || self.plant_paused(net.plant) {
                continue;
            }
            let mut cap = net.graph.cap.clone();
            // Each pump gives its flow this step.
            for (e, node) in &net.sources {
                cap[*node] = match e {
                    End::Pump(q) => self.pump_give(*q),
                    End::Tank(si) => self.shells[*si].water / DT,
                    _ => 0.0,
                };
            }
            let feeds: Vec<(usize, Vec<usize>)> = net
                .consumers
                .iter()
                .filter_map(|(e, nodes)| match e {
                    End::Feed(si) => Some((*si, nodes.clone())),
                    _ => None,
                })
                .collect();
            let tanks: Vec<(usize, Vec<usize>)> = net
                .consumers
                .iter()
                .filter_map(|(e, nodes)| match e {
                    End::Tank(si) => Some((*si, nodes.clone())),
                    _ => None,
                })
                .collect();
            let tree = net.tree.clone();
            let tank_tree = net.tank_tree.clone();
            let mut total_flow = vec![0.0; cap.len()];
            // Pumps to boilers, tanks to boilers, pumps to tanks.
            let passes: [(&route::Tree, &Takers); 3] =
                [(&tree, &feeds), (&tank_tree, &feeds), (&tree, &tanks)];
            for (tree, takers) in passes {
                let room: Vec<f64> = takers
                    .iter()
                    .map(|(si, _)| {
                        let s = &self.shells[*si];
                        if s.works() && !s.partial {
                            (s.water_capacity() - s.water).max(0.0) / DT
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let (got, flow) = route_spread(tree, &cap, takers, &room);
                for (u, f) in flow.iter().enumerate() {
                    cap[u] = (cap[u] - f).max(0.0);
                    total_flow[u] += f;
                }
                for ((si, _), g) in takers.iter().zip(&got) {
                    if *g > 0.0 {
                        self.fill_shell(*si, g * DT);
                    }
                }
            }
            // What each source gave.
            let net = &self.nets[ni];
            let mut drained: Vec<(usize, f64)> = Vec::new();
            for (e, node) in &net.sources {
                let gave = total_flow[*node];
                match e {
                    End::Pump(q) => {
                        *self.pump_flow.entry(*q).or_default() += gave;
                    }
                    End::Tank(si) => drained.push((*si, gave * DT)),
                    _ => {}
                }
            }
            for (si, mb) in drained {
                let s = &mut self.shells[si];
                s.water = (s.water - mb).max(0.0);
            }
            self.nets[ni].flow = total_flow;
        }
        // Loads: a pump pulls its load whenever it delivers anything.
        self.pump_loads.clear();
        for q in self.rot_parts() {
            if self.parts[&q].spec.kind != Kind::Pump {
                continue;
            }
            if self.plant_of(q).is_some_and(|p| self.plant_paused(p)) {
                continue;
            }
            let delivering = self.pump_flow.get(&q).is_some_and(|f| *f > 1e-6);
            self.pump_loads
                .insert(q, if delivering { pump::LOAD } else { 0.0 });
        }
    }

    /// What a pump can push this step, mB/s.
    fn pump_give(&self, q: Pos) -> f64 {
        let p = &self.parts[&q];
        let r = self.rot.get(&q).copied().unwrap_or_default();
        if p.spec.damaged || r.jammed {
            return 0.0;
        }
        pump::flow(r.omega, self.pump_sources.get(&q).copied().unwrap_or(0))
    }

    /// Water into a boiler (mixing its temperature, flashing on a hot dry
    /// shell) or a water tank.
    fn fill_shell(&mut self, si: usize, mb: f64) {
        let s = &mut self.shells[si];
        let mb = mb.min((s.water_capacity() - s.water).max(0.0));
        if mb <= 0.0 {
            return;
        }
        if s.is_boiler() {
            let mut b = Boiler {
                blocks: s.n(),
                water: s.water,
                temp: s.temp,
            };
            let flashed = b.feed(mb, AMBIENT);
            s.water = b.water;
            s.temp = b.temp;
            s.fed += mb / DT;
            if let Some(p) = s.pool {
                self.steam[p].pool.steam += flashed;
                self.steam[p].steam_in += flashed / DT;
            }
        } else {
            s.water += mb;
        }
    }

    // ---- Steam (spec 2, 5; fluids spec 5.1) --------------------------------------

    fn route_steam(&mut self) {
        let mut vents: BTreeMap<Pos, VentState> = BTreeMap::new();
        for e in &mut self.engines {
            e.demand.iter_mut().for_each(|d| *d = 0.0);
            e.fill.iter_mut().for_each(|f| *f = 1.0);
        }
        for pi in 0..self.steam.len() {
            if self.plant_paused(self.steam[pi].plant) {
                // Paused vents keep their look.
                for &ni in &self.steam[pi].nets {
                    for (e, _) in &self.nets[ni].consumers {
                        if let End::Vent(q) = e
                            && let Some(v) = self.vents.get(q)
                        {
                            vents.insert(*q, *v);
                        }
                    }
                }
                continue;
            }
            let p = self.steam[pi].pool.pressure();
            // Every network's demands and their routes.
            let mut plan: Vec<(usize, Vec<Taker>, Vec<f64>)> = Vec::new();
            for &ni in &self.steam[pi].nets.clone() {
                let net = &self.nets[ni];
                let mut takers = Vec::new();
                let mut demands = Vec::new();
                let mut nodes = Vec::new();
                for (e, at) in &net.consumers {
                    let (t, d) = match e {
                        End::Cylinder(q) => (Taker::Cylinder(*q), self.cylinder_demand(*q, p)),
                        End::Vent(q) => (Taker::Vent(*q), self.vent_demand(*q, p, &mut vents)),
                        _ => continue,
                    };
                    takers.push(t);
                    demands.push(d);
                    nodes.push(at.clone());
                }
                for q in &net.pipes {
                    if self.parts[q].spec.damaged {
                        takers.push(Taker::Burst(*q));
                        demands.push(self.parts[q].spec.throughput_now());
                        nodes.push(vec![net.node_of[q]]);
                    }
                }
                let cap: Vec<f64> = (0..net.graph.len())
                    .map(|u| {
                        let key = net.graph.key[u];
                        match self.parts.get(&key) {
                            Some(part) if net.node_of.get(&key) == Some(&u) => {
                                part.spec.throughput_now()
                            }
                            _ => net.graph.cap[u],
                        }
                    })
                    .collect();
                let pairs: Vec<(usize, Vec<usize>)> =
                    (0..takers.len()).map(|i| (i, nodes[i].clone())).collect();
                let (got, flow) = route_spread(&net.tree, &cap, &pairs, &demands);
                self.nets[ni].flow = flow;
                plan.push((
                    ni,
                    takers,
                    got.into_iter()
                        .zip(demands)
                        .flat_map(|(g, d)| [g, d])
                        .collect(),
                ));
            }
            // Never more than the network holds this step.
            let want: f64 = plan
                .iter()
                .flat_map(|(_, _, gd)| gd.chunks(2).map(|c| c[0]))
                .sum::<f64>()
                * DT;
            let held = self.steam[pi].pool.steam.max(0.0);
            let scale = if want > held && want > 0.0 {
                held / want
            } else {
                1.0
            };
            let mut out = 0.0;
            for (_, takers, gd) in &plan {
                for (t, c) in takers.iter().zip(gd.chunks(2)) {
                    let got = c[0] * scale;
                    let demand = c[1];
                    out += got;
                    match t {
                        Taker::Cylinder(q) => {
                            if let Some(&ei) = self.engine_of.get(q) {
                                let e = &mut self.engines[ei];
                                if let Some(i) = e.cylinders.iter().position(|c| c == q) {
                                    e.demand[i] = demand;
                                    e.fill[i] = engine::fill(got, demand);
                                }
                            }
                        }
                        Taker::Vent(q) => {
                            let v = vents.entry(*q).or_default();
                            v.rate = got;
                        }
                        Taker::Burst(q) => {
                            vents.insert(
                                *q,
                                VentState {
                                    open: true,
                                    simmer: false,
                                    rate: got,
                                },
                            );
                        }
                    }
                }
            }
            let sn = &mut self.steam[pi];
            sn.pool.take(out * DT);
            sn.steam_out = out;
        }
        // Emergency valves that popped or reseated.
        for (q, v) in &vents {
            let was = self.vents.get(q).is_some_and(|o| o.open);
            if self
                .parts
                .get(q)
                .is_some_and(|p| p.spec.kind == Kind::Vent { emergency: true })
                && v.open != was
            {
                self.events.push(if v.open {
                    Event::ValveOpen(*q)
                } else {
                    Event::ValveShut(*q)
                });
            }
        }
        self.vents = vents;
    }

    /// A cylinder's demand at pressure `p`: nothing if jammed or not part of
    /// an engine.
    fn cylinder_demand(&self, q: Pos, p: f64) -> f64 {
        if self.parts[&q].spec.damaged {
            return 0.0;
        }
        match self.engine_of.get(&q) {
            Some(&ei) => {
                let fly = self.engines[ei].flywheel;
                if self.rot.get(&fly).is_some_and(|r| r.jammed) {
                    return 0.0;
                }
                engine::demand(self.omega(fly), p)
            }
            None => 0.0,
        }
    }

    /// A vent's demand at pressure `p`, recording whether it is open.
    fn vent_demand(&self, q: Pos, p: f64, vents: &mut BTreeMap<Pos, VentState>) -> f64 {
        let spec = &self.parts[&q].spec;
        let (open, simmer) = if spec.damaged {
            (true, false)
        } else {
            match spec.kind {
                Kind::Vent { emergency: true } => {
                    let was = self.vents.get(&q).is_some_and(|v| v.open);
                    let open = valve::emergency(was, p, spec.set);
                    (open, !open && valve::simmering(p, spec.set))
                }
                _ => (spec.opening > 0.5, false),
            }
        };
        vents.insert(
            q,
            VentState {
                open,
                simmer,
                rate: 0.0,
            },
        );
        if !open {
            0.0
        } else if spec.damaged {
            spec.kind.throughput()
        } else {
            VENT_RATE
        }
    }

    // ---- Failures (spec 2, 7) -----------------------------------------------------

    fn check_failures(&mut self) {
        for pi in 0..self.steam.len() {
            if self.plant_paused(self.steam[pi].plant) {
                continue;
            }
            let p = self.steam[pi].pool.pressure();
            // Boilers and steam tanks.
            let mut ruptured: Option<Event> = None;
            for &si in &self.steam[pi].shells.clone() {
                let s = &mut self.shells[si];
                if !s.works() || s.partial {
                    continue;
                }
                s.timer = fail::tick_timer(s.timer, p, s.kind.rating(), DT);
                let dry = s.is_boiler() && s.temp >= s.kind.dry_limit();
                if ruptured.is_none() && (fail::failed(s.timer) || dry) {
                    let n = s.n();
                    let power = if dry && !fail::failed(s.timer) {
                        fail::dry_rupture_power(p, n)
                    } else {
                        fail::rupture_power(p, n)
                    };
                    s.ruptured = true;
                    s.water = 0.0;
                    s.timer = 0.0;
                    let c = s.cuboid;
                    let m = Cuboid::max(&c);
                    ruptured = Some(Event::Rupture {
                        kind: s.kind,
                        blocks: s.cuboid.positions(),
                        center: [
                            (c.min.x + m.x + 1) as f64 / 2.0,
                            (c.min.y + m.y + 1) as f64 / 2.0,
                            (c.min.z + m.z + 1) as f64 / 2.0,
                        ],
                        power,
                        cloud: fail::cloud_radius(n),
                        dry,
                    });
                }
            }
            if let Some(ev) = ruptured {
                // All its network's steam goes at once.
                self.steam[pi].pool.steam = 0.0;
                self.events.push(ev);
                continue;
            }
            // Pipe parts: one timer per kind; one part bursts when it fills.
            let mut by_kind: BTreeMap<u8, (f64, Vec<Pos>)> = BTreeMap::new();
            for &ni in &self.steam[pi].nets {
                let net = &self.nets[ni];
                let vents = net.consumers.iter().filter_map(|(e, _)| match e {
                    End::Vent(q) => Some(*q),
                    _ => None,
                });
                for q in net.pipes.iter().copied().chain(vents) {
                    let spec = &self.parts[&q].spec;
                    if let (Some(k), Some(r)) = (timer_kind(spec.kind), spec.kind.rating()) {
                        let e = by_kind.entry(k).or_insert((r, Vec::new()));
                        if !spec.damaged {
                            e.1.push(q);
                        }
                    }
                }
            }
            let mut timers = std::mem::take(&mut self.steam[pi].timers);
            for (k, (rating, mut intact)) in by_kind {
                let t = timers.entry(k).or_insert(0.0);
                *t = fail::tick_timer(*t, p, rating, DT);
                if fail::failed(*t) {
                    *t = 0.0;
                    intact.sort();
                    if let Some(&q) = intact.first() {
                        self.events.push(Event::Burst(q));
                        if let Some(part) = self.parts.get_mut(&q) {
                            part.spec.damaged = true;
                        }
                    }
                }
            }
            timers.retain(|_, t| *t > 0.0);
            self.steam[pi].timers = timers;
            // Cylinders jam over their rating, each on its own timer.
            for &ni in &self.steam[pi].nets {
                for (e, _) in &self.nets[ni].consumers {
                    let End::Cylinder(q) = e else { continue };
                    let Some(part) = self.parts.get_mut(q) else {
                        continue;
                    };
                    if part.spec.damaged {
                        continue;
                    }
                    part.saved.timer = fail::tick_timer(part.saved.timer, p, fail::CYLINDER, DT);
                    if fail::failed(part.saved.timer) {
                        part.saved.timer = 0.0;
                        part.spec.damaged = true;
                        self.events.push(Event::CylinderJam(*q));
                    }
                }
            }
        }
    }

    // ---- Engines and sync -------------------------------------------------------

    fn set_drives(&mut self) {
        let mut drives = BTreeMap::new();
        for q in self.rot_parts() {
            let spec = self.parts[&q].spec;
            if spec.kind != Kind::Flywheel {
                continue;
            }
            let dir = if spec.reverse { -1.0 } else { 1.0 };
            let mut d = Drive {
                stall: 0.0,
                top: 0.0,
                dir,
            };
            if let Some(&ei) = self.engine_of.get(&q) {
                let e = &self.engines[ei];
                if self.plant_paused(e.plant) {
                    // Held: keep what it had.
                    d = self.drives.get(&q).copied().unwrap_or(d);
                    d.dir = dir;
                } else {
                    for (i, c) in e.cylinders.iter().enumerate() {
                        if self.parts[c].spec.damaged {
                            continue;
                        }
                        let Some(p) = self.cylinder_pressure(*c) else {
                            continue;
                        };
                        d.stall += engine::stall(p, e.fill[i]);
                        d.top = d.top.max(engine::top(p));
                    }
                }
                self.engines[ei].drive = d;
            }
            drives.insert(q, d);
        }
        self.drives = drives;
    }

    /// The pressure at a cylinder's inlet (None off any steam network).
    pub fn cylinder_pressure(&self, q: Pos) -> Option<f64> {
        let ni = *self.net_of.get(&q)?;
        let pi = self.nets[ni].pool?;
        Some(self.steam[pi].pool.pressure())
    }

    /// Shell blocks whose look changed: warning level and glow.
    fn sync_shells(&mut self) {
        for si in 0..self.shells.len() {
            let s = &self.shells[si];
            let p = s.pool.map_or(0.0, |i| self.steam[i].pool.pressure());
            let level = if s.works() && s.pool.is_some() {
                fail::warning_level(p, s.kind.rating(), s.timer)
            } else {
                0
            };
            let glow = s.is_boiler() && s.temp >= fail::GLOW_TEMP;
            for b in s.blocks.clone() {
                if let Some(part) = self.parts.get_mut(&b)
                    && part.sent != (level, glow)
                {
                    part.sent = (level, glow);
                    self.sync.push(b);
                }
            }
        }
    }

    /// Shell blocks to send to clients (warning level, glow).
    pub fn take_sync(&mut self) -> Vec<Pos> {
        std::mem::take(&mut self.sync)
    }

    /// Positions the plant holds paused on the rotational side.
    pub fn held_parts(&self) -> &BTreeSet<Pos> {
        &self.held
    }

    /// Steps run (for tests and the profiler).
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The step length, seconds.
    pub fn dt() -> f64 {
        steam::DT
    }
}

/// Route `demands` to takers attached at several nodes each: a taker's
/// demand is split evenly over the reached nodes it attaches at. Returns
/// each taker's delivery and every node's flow.
fn route_spread(
    tree: &route::Tree,
    cap: &[f64],
    takers: &[(usize, Vec<usize>)],
    demands: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let mut pairs = Vec::new();
    let mut owner = Vec::new();
    for (i, (_, nodes)) in takers.iter().enumerate() {
        let reached: Vec<usize> = nodes
            .iter()
            .copied()
            .filter(|&u| u < tree.reached.len() && tree.reached[u])
            .collect();
        if reached.is_empty() {
            continue;
        }
        let share = demands[i] / reached.len() as f64;
        for u in reached {
            pairs.push((u, share));
            owner.push(i);
        }
    }
    let (got, flow) = if pairs.is_empty() {
        (Vec::new(), vec![0.0; cap.len()])
    } else {
        route::route(tree, cap, &pairs)
    };
    let mut out = vec![0.0; takers.len()];
    for (i, g) in owner.iter().zip(got) {
        out[*i] += g;
    }
    (out, flow)
}
