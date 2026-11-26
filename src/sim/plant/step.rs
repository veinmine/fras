//! The steam step (steam and heat spec 14.1), every 4 ticks after the
//! rotational solve: rebuild what changed, check which plants pause, heat
//! the boilers, feed them water, route and deliver steam, update pressures,
//! timers and failures, then hand each engine its stall torque and top speed
//! and each pump its load.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::build::Dsu;
use super::*;
use crate::sim::fluid::{DRAIN, cool, exchange, pump, route, valve};
use crate::sim::geom::Dir;
use crate::sim::rot::View;
use crate::sim::rust::LEAK_RATE;
use crate::sim::steam::boiler::Boiler;
use crate::sim::steam::{
    self, AMBIENT, DT, HOT_WATER, VENT_RATE, bellows, damper, engine, fail, govern,
};

/// What a creative drain takes at most, mB/s: more than any pipe carries.
const VOID: f64 = 1.0e6;

/// The warning timer kinds of pipe parts on a steam network.
pub(crate) fn timer_kind(kind: Kind) -> Option<u8> {
    Some(match kind {
        Kind::Pipe(PipeMat::Copper) => 0,
        Kind::Pipe(PipeMat::Iron) => 1,
        Kind::Valve { .. } | Kind::Governor => 2,
        Kind::Vent { .. } => 3,
        _ => return None,
    })
}

/// A steam consumer this step.
#[derive(Copy, Clone, Debug)]
enum Taker {
    Cylinder(Pos),
    Vent(Pos),
    /// A burst pipe part, by position.
    Burst(Pos),
    /// A leaking part (rust spec 6), by position.
    Leak(Pos),
}

/// A consumer of water or hot water this step.
#[derive(Copy, Clone, Debug)]
enum Liquid {
    Leak(Pos),
    Feed(usize),
    Cool(Pos),
    Ex(Pos),
    Tank(usize),
    Drain(Pos),
    Void(Pos),
}

impl Works {
    /// Run one steam step.
    pub fn step(&mut self, view: &dyn View) {
        self.rebuild(view);
        self.steps += 1;
        self.refresh_pause(view);
        self.update_governors();
        self.update_dampers();
        self.heat_boilers();
        self.gave.clear();
        self.voids.clear();
        self.prepare_cooling();
        self.feed_water();
        self.route_steam();
        // After the steam is drawn, so its networks end each step at its
        // pressure (as gauges and engines read them).
        self.creative_steam();
        self.route_exhaust();
        self.finish_cooling();
        self.check_failures();
        self.set_drives();
        self.set_bellows_loads();
        self.sync_shells();
    }

    // ---- Regulation (regulation spec 3 and 4) ------------------------------------

    fn update_governors(&mut self) {
        let cap = (govern::HISTORY_SECONDS / DT).round() as usize;
        let govs = self.sorted_kind(Kind::Governor);
        for q in govs {
            if self.plant_of(q).is_some_and(|p| self.plant_paused(p)) {
                continue;
            }
            let part = &self.parts[&q];
            let g = govern::throttle(
                part.saved.ball,
                part.spec.target,
                govern::band(part.spec.sens),
            );
            let st = self.govs.entry(q).or_default();
            st.g = g;
            let s = st.speed;
            st.history.push(s, cap);
        }
    }

    /// The governors' throttle on the route to `node` of network `ni`.
    pub(crate) fn throttle_at(&self, ni: usize, node: usize) -> f64 {
        self.nets[ni].govs.get(node).map_or(1.0, |gs| {
            gs.iter()
                .map(|q| self.govs.get(q).map_or(1.0, |g| g.g))
                .product()
        })
    }

    /// The throttle on a consumer attached at `nodes` (its first reached one).
    fn throttle_on(&self, ni: usize, nodes: &[usize]) -> f64 {
        let t = &self.nets[ni].tree;
        nodes
            .iter()
            .find(|&&u| u < t.reached.len() && t.reached[u])
            .map_or(1.0, |&u| self.throttle_at(ni, u))
    }

    fn update_dampers(&mut self) {
        for si in 0..self.shells.len() {
            let s = &self.shells[si];
            if s.dampers.is_empty() || s.partial || self.plant_paused(s.plant) {
                continue;
            }
            let p = s.pool.map_or(0.0, |i| self.steam[i].pool.pressure());
            let mut least: f64 = 1.0;
            for d in s.dampers.clone() {
                let part = self.parts.get_mut(&d).unwrap();
                if !part.spec.leaking() {
                    let target = damper::target(p, part.spec.set, part.spec.powered);
                    part.saved.opening = damper::travel(part.saved.opening, target, DT);
                }
                least = least.min(part.saved.opening);
            }
            self.shells[si].opening = least;
        }
    }

    fn sorted_kind(&self, kind: Kind) -> Vec<Pos> {
        let mut out: Vec<Pos> = self
            .parts
            .iter()
            .filter(|(_, p)| p.spec.kind == kind)
            .map(|(q, _)| *q)
            .collect();
        out.sort();
        out
    }

    fn set_bellows_loads(&mut self) {
        self.bellows_loads.clear();
        for b in self.sorted_kind(Kind::Bellows) {
            if self.plant_of(b).is_some_and(|p| self.plant_paused(p)) {
                continue;
            }
            let blowing = self
                .bellows_of
                .iter()
                .find(|(_, x)| **x == b)
                .and_then(|(f, _)| self.fires.get(f))
                .is_some_and(|f| f.kw > 0.0 && f.damper > 0.0 && f.boost > 0.0);
            self.bellows_loads
                .insert(b, if blowing { bellows::LOAD } else { 0.0 });
        }
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
        // Each firebox: its fuel's rate, the bellows' boost, its boilers' dampers.
        let mut fires: BTreeMap<Pos, FireState> = BTreeMap::new();
        for f in self.sorted_kind(Kind::Firebox) {
            let kw = self.heat.get(&f).copied().unwrap_or(0.0);
            let boost = self.bellows_of.get(&f).map_or(0.0, |b| {
                let r = self.rot.get(b).copied().unwrap_or_default();
                if r.jammed || r.paused {
                    0.0
                } else {
                    bellows::boost(r.omega)
                }
            });
            let openings: Vec<f64> = self
                .heats
                .get(&f)
                .map(|bs| bs.iter().map(|&b| self.shells[b].opening).collect())
                .unwrap_or_default();
            fires.insert(
                f,
                FireState {
                    kw,
                    boost,
                    damper: damper::burn_factor(&openings),
                },
            );
        }
        let mut heat_in: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
        for (spot, boilers) in &self.heats {
            let Some(fire) = fires.get(spot) else {
                continue;
            };
            if fire.kw <= 0.0 || boilers.is_empty() {
                continue;
            }
            let share = fire.kw * (1.0 + fire.boost) / boilers.len() as f64;
            for &b in boilers {
                let e = heat_in.entry(b).or_default();
                e.0 += share * self.shells[b].opening;
                e.1 += 1;
            }
        }
        self.fires = fires;
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

    // ---- Creative sources ------------------------------------------------------

    /// Creative steam sources top their networks up to their pressure, at
    /// most at their rate.
    fn creative_steam(&mut self) {
        for pi in 0..self.steam.len() {
            if self.plant_paused(self.steam[pi].plant) {
                continue;
            }
            let mut sources: Vec<Pos> = Vec::new();
            for &ni in &self.steam[pi].nets {
                for (e, _) in &self.nets[ni].sources {
                    if let End::Source(q, Content::Steam) = e
                        && !sources.contains(q)
                    {
                        sources.push(*q);
                    }
                }
            }
            sources.sort();
            for q in sources {
                let Some(part) = self.parts.get(&q) else {
                    continue;
                };
                let (bar, rate) = (part.spec.set.max(0.0), part.spec.amount.max(0.0));
                let pool = &mut self.steam[pi].pool;
                let room = (bar * pool.capacity - pool.steam).max(0.0);
                let add = (rate * DT).min(room);
                pool.steam += add;
                self.steam[pi].steam_in += add / DT;
                *self.gave.entry(q).or_default() += add / DT;
            }
        }
    }

    // ---- Cooling (cooling spec 5, 6 and 9) ---------------------------------------

    /// Each exchanger's inlet, power and intake for this step, and each
    /// grinder's cooling demand.
    fn prepare_cooling(&mut self) {
        let mut ex: HashMap<Pos, ExState> = HashMap::new();
        for x in self.sorted_kind(Kind::Exchanger) {
            let spec = self.parts[&x].spec;
            let (inn, out) = self.ex_nets.get(&x).copied().unwrap_or_default();
            let live = |ni: Option<usize>| {
                ni.filter(|&i| !self.nets[i].mixed && !self.plant_paused(self.nets[i].plant))
            };
            let inlet = live(inn).and_then(|i| self.nets[i].content);
            // Radiators on its four side faces.
            let mut rads = Vec::new();
            let mut count = (0, 0);
            for d in Dir::ALL {
                if d.axis() == spec.dir.axis() {
                    continue;
                }
                let q = x.offset(d);
                if let Some(r) = self.parts.get(&q)
                    && let Kind::Radiator { copper } = r.spec.kind
                    && r.spec.dir == d
                {
                    if copper {
                        rads.push(exchange::Radiator::Copper);
                        count.0 += 1;
                    } else if r.spec.leaking() {
                        rads.push(exchange::Radiator::Leaking);
                        count.1 += 1;
                    } else {
                        rads.push(exchange::Radiator::Iron);
                        count.1 += 1;
                    }
                }
            }
            let power = exchange::power(spec.wet, &rads);
            let capacity = inlet.map_or(0.0, |c| exchange::capacity(c, power));
            let wants = inlet.and_then(exchange::converts);
            let blocked = match (live(out), wants) {
                (_, None) => None,
                (None, Some(_)) => Some(ExBlock::NoOutlet),
                (Some(o), Some(w)) => match self.nets[o].content {
                    Some(c) if c != w => Some(ExBlock::OutletCarries(c)),
                    _ if self.parts[&x].saved.buffer >= exchange::BUFFER - 1e-6 => {
                        Some(ExBlock::OutletFull)
                    }
                    _ => None,
                },
            };
            ex.insert(
                x,
                ExState {
                    inlet,
                    power,
                    radiators: count,
                    capacity,
                    took: 0.0,
                    blocked,
                },
            );
        }
        self.exchangers = ex;
        let mut cooling = HashMap::new();
        for g in self.sorted_kind(Kind::Grinder) {
            let demand = self.cool_demand.get(&g).copied().unwrap_or(0.0);
            cooling.insert(
                g,
                CoolState {
                    demand,
                    received: 0.0,
                    cooled: false,
                },
            );
        }
        self.cooling = cooling;
    }

    /// What an exchanger can take in now, mB/s.
    fn ex_intake(&self, x: Pos) -> f64 {
        let Some(e) = self.exchangers.get(&x) else {
            return 0.0;
        };
        if e.blocked.is_some() || e.inlet.is_none() {
            return 0.0;
        }
        let room = (exchange::BUFFER - self.parts[&x].saved.buffer).max(0.0);
        (e.capacity - e.took).max(0.0).min(room / DT)
    }

    /// An exchanger took `mb_s` of its inlet: its outlet buffer gets the
    /// converted content (less a leak's drip).
    fn ex_take(&mut self, x: Pos, mb_s: f64) {
        let leaking = self.parts.get(&x).is_some_and(|p| p.spec.leaking());
        if let Some(e) = self.exchangers.get_mut(&x) {
            e.took += mb_s;
        }
        let mut mb = mb_s * DT;
        if leaking {
            let lost = (LEAK_RATE * DT).min(mb);
            mb -= lost;
            let c = self
                .exchangers
                .get(&x)
                .and_then(|e| e.inlet)
                .unwrap_or(Content::Water);
            let e = self.leaks.entry(x).or_insert((c, 0.0));
            e.1 += lost / DT;
        }
        if let Some(p) = self.parts.get_mut(&x) {
            p.saved.buffer = (p.saved.buffer + mb).min(exchange::BUFFER);
        }
    }

    /// What a grinder can take in now, mB/s.
    fn cool_intake(&self, g: Pos) -> f64 {
        let Some(c) = self.cooling.get(&g) else {
            return 0.0;
        };
        let room = (cool::BUFFER - self.parts[&g].saved.buffer).max(0.0);
        (c.demand - c.received).max(0.0).min(room / DT)
    }

    fn finish_cooling(&mut self) {
        // A grinder whose cooling inlet has water to draw on counts as cooled
        // while idle; one with no supply never does.
        let mut supplied: BTreeSet<Pos> = BTreeSet::new();
        for net in &self.nets {
            if net.content != Some(Content::Water) || net.mixed {
                continue;
            }
            for (e, nodes) in &net.consumers {
                if let End::CoolIn(g) = e {
                    let reached = |t: &route::Tree| {
                        nodes.iter().any(|&u| u < t.reached.len() && t.reached[u])
                    };
                    if reached(&net.tree) || reached(&net.tank_tree) {
                        supplied.insert(*g);
                    }
                }
            }
        }
        let grinders: Vec<Pos> = self.cooling.keys().copied().collect();
        for g in grinders {
            let buffer = self.parts[&g].saved.buffer;
            let c = self.cooling.get_mut(&g).unwrap();
            c.cooled = supplied.contains(&g) && cool::cooled(c.demand, c.received, buffer);
        }
    }

    // ---- Water and hot water (fluids spec 5.2, cooling spec 7.4) -------------

    fn feed_water(&mut self) {
        self.pump_flow.clear();
        self.drains.clear();
        self.leaks.clear();
        // Hot water networks first, so boilers fed both ways prefer hot feed
        // whatever order the networks were built in (cooling spec 7.4).
        let mut order: Vec<usize> = (0..self.nets.len()).collect();
        order.sort_by_key(|&ni| self.nets[ni].content != Some(Content::HotWater));
        for ni in order {
            let net = &self.nets[ni];
            let Some(c) = net.content.filter(|c| c.is_liquid()) else {
                continue;
            };
            if net.mixed || self.plant_paused(net.plant) {
                continue;
            }
            let mut cap = net.graph.cap.clone();
            // What each source can give this step.
            for (e, node) in &net.sources {
                cap[*node] = match e {
                    End::Pump(q) if c == Content::Water => self.pump_give(*q),
                    End::Tank(si) => self.shells[*si].water / DT,
                    End::ExOut(x) => {
                        let ok = self
                            .exchangers
                            .get(x)
                            .and_then(|s| s.inlet)
                            .and_then(exchange::converts)
                            == Some(c);
                        if ok {
                            self.parts[x].saved.buffer / DT
                        } else {
                            0.0
                        }
                    }
                    End::CoolOut(g) if c == Content::HotWater => self.parts[g].saved.buffer / DT,
                    End::Source(q, sc) if *sc == c => self.parts[q].spec.amount.max(0.0),
                    _ => 0.0,
                };
            }
            let mut groups: [Vec<(Liquid, Vec<usize>)>; 5] = Default::default();
            for q in &net.pipes {
                let part = &self.parts[q];
                if part.spec.leaking() && !part.spec.damaged {
                    groups[0].push((Liquid::Leak(*q), vec![net.node_of[q]]));
                }
            }
            for (e, nodes) in &net.consumers {
                let (i, t) = match e {
                    End::Feed(si) => (1, Liquid::Feed(*si)),
                    End::CoolIn(g) => (2, Liquid::Cool(*g)),
                    End::ExIn(x) => (2, Liquid::Ex(*x)),
                    End::Tank(si) => (3, Liquid::Tank(*si)),
                    End::Drain(q) => (4, Liquid::Drain(*q)),
                    End::Void(q) => (4, Liquid::Void(*q)),
                    _ => continue,
                };
                groups[i].push((t, nodes.clone()));
            }
            let trees = net.source_trees.clone();
            let tank_tree = net.tank_tree.clone();
            let mut total_flow = vec![0.0; cap.len()];
            // Leaks take what passes, then boilers (hot feed first: hot water
            // networks go first), cooled machines and exchangers, tanks, and
            // drains only what is left (cooling spec 7.4). Each source routes
            // on its own tree, one after another.
            let mut passes: Vec<(&route::Tree, usize)> = Vec::new();
            for g in 0..5 {
                passes.extend(trees.iter().map(|t| (t, g)));
                if g == 1 || g == 2 {
                    passes.push((&tank_tree, g));
                }
            }
            for (tree, g) in passes {
                let takers = &groups[g];
                if takers.is_empty() {
                    continue;
                }
                let demand: Vec<f64> = takers.iter().map(|(t, _)| self.liquid_room(*t)).collect();
                let pairs: Vec<(usize, Vec<usize>)> = takers
                    .iter()
                    .enumerate()
                    .map(|(i, (_, n))| (i, n.clone()))
                    .collect();
                let (got, flow) = route_spread(tree, &cap, &pairs, &demand);
                for (u, f) in flow.iter().enumerate() {
                    cap[u] = (cap[u] - f).max(0.0);
                    total_flow[u] += f;
                }
                for ((t, _), g) in takers.iter().zip(&got) {
                    if *g > 0.0 {
                        self.liquid_take(*t, c, *g);
                    }
                }
            }
            // What each source gave.
            let net = &self.nets[ni];
            let mut gave: Vec<(End, f64)> = Vec::new();
            for (e, node) in &net.sources {
                gave.push((*e, total_flow[*node]));
            }
            for (e, f) in gave {
                match e {
                    End::Pump(q) => *self.pump_flow.entry(q).or_default() += f,
                    End::Tank(si) => {
                        let s = &mut self.shells[si];
                        s.water = (s.water - f * DT).max(0.0);
                    }
                    End::ExOut(q) | End::CoolOut(q) => {
                        if let Some(p) = self.parts.get_mut(&q) {
                            p.saved.buffer = (p.saved.buffer - f * DT).max(0.0);
                        }
                    }
                    End::Source(q, _) => *self.gave.entry(q).or_default() += f,
                    _ => {}
                }
            }
            self.nets[ni].flow = total_flow;
        }
        // Leaking water tank blocks drip.
        for si in 0..self.shells.len() {
            let s = &self.shells[si];
            let Some(c) = s.content.filter(|c| c.is_liquid()) else {
                continue;
            };
            if !s.works() || s.partial || self.plant_paused(s.plant) {
                continue;
            }
            let leaking: Vec<Pos> = s
                .blocks
                .iter()
                .copied()
                .filter(|b| self.parts[b].spec.leaking())
                .collect();
            let mut left = s.water;
            for b in &leaking {
                let lost = (LEAK_RATE * DT).min(left);
                left -= lost;
                self.leaks.insert(*b, (c, lost / DT));
            }
            let s = &mut self.shells[si];
            s.leaked = (s.water - left) / DT;
            s.water = left;
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

    /// What a water or hot water consumer takes at most now, mB/s.
    fn liquid_room(&self, t: Liquid) -> f64 {
        let room = |si: usize| {
            let s = &self.shells[si];
            if s.works() && !s.partial {
                (s.water_capacity() - s.water).max(0.0) / DT
            } else {
                0.0
            }
        };
        match t {
            Liquid::Leak(_) => LEAK_RATE,
            Liquid::Feed(si) | Liquid::Tank(si) => room(si),
            Liquid::Cool(g) => self.cool_intake(g),
            Liquid::Ex(x) => self.ex_intake(x),
            Liquid::Drain(q) => {
                if self.parts.get(&q).is_some_and(|p| p.spec.damaged) {
                    0.0
                } else {
                    DRAIN
                }
            }
            Liquid::Void(_) => VOID,
        }
    }

    fn liquid_take(&mut self, t: Liquid, c: Content, got: f64) {
        match t {
            Liquid::Leak(q) => {
                let e = self.leaks.entry(q).or_insert((c, 0.0));
                e.1 += got;
            }
            Liquid::Feed(si) => {
                let t_feed = if c == Content::HotWater {
                    HOT_WATER
                } else {
                    AMBIENT
                };
                self.fill_shell(si, got * DT, t_feed);
            }
            Liquid::Tank(si) => self.fill_shell(si, got * DT, AMBIENT),
            Liquid::Cool(g) => {
                if let Some(s) = self.cooling.get_mut(&g) {
                    s.received += got;
                }
                if let Some(p) = self.parts.get_mut(&g) {
                    p.saved.buffer = (p.saved.buffer + got * DT).min(cool::BUFFER);
                }
            }
            Liquid::Ex(x) => self.ex_take(x, got),
            Liquid::Drain(q) => {
                let e = self.drains.entry(q).or_insert((c, 0.0));
                e.1 += got;
            }
            Liquid::Void(q) => {
                let e = self.voids.entry(q).or_insert((c, 0.0));
                e.1 += got;
            }
        }
    }

    // ---- Exhaust (cooling spec 4) -------------------------------------------------

    /// Each cylinder's exhaust to its exchangers; what they can't take vents
    /// at the cylinder.
    fn route_exhaust(&mut self) {
        let mut piped: HashMap<Pos, f64> = HashMap::new();
        for ni in 0..self.nets.len() {
            let net = &self.nets[ni];
            if net.content != Some(Content::Exhaust) || net.mixed || self.plant_paused(net.plant) {
                continue;
            }
            let mut cap = net.graph.cap.clone();
            for (e, node) in &net.sources {
                cap[*node] = match e {
                    End::Exhaust(q) => self.cylinder_drawn(*q),
                    _ => 0.0,
                };
            }
            let mut leaks: Vec<(Liquid, Vec<usize>)> = Vec::new();
            for q in &net.pipes {
                let part = &self.parts[q];
                if part.spec.leaking() && !part.spec.damaged {
                    leaks.push((Liquid::Leak(*q), vec![net.node_of[q]]));
                }
            }
            let exs: Vec<(Liquid, Vec<usize>)> = net
                .consumers
                .iter()
                .filter_map(|(e, n)| match e {
                    End::ExIn(x) => Some((Liquid::Ex(*x), n.clone())),
                    End::Void(q) => Some((Liquid::Void(*q), n.clone())),
                    _ => None,
                })
                .collect();
            let trees = net.source_trees.clone();
            let mut total_flow = vec![0.0; cap.len()];
            for (tree, takers) in trees.iter().flat_map(|t| [(t, &leaks), (t, &exs)]) {
                if takers.is_empty() {
                    continue;
                }
                let demand: Vec<f64> = takers.iter().map(|(t, _)| self.liquid_room(*t)).collect();
                let pairs: Vec<(usize, Vec<usize>)> = takers
                    .iter()
                    .enumerate()
                    .map(|(i, (_, n))| (i, n.clone()))
                    .collect();
                let (got, flow) = route_spread(tree, &cap, &pairs, &demand);
                for (u, f) in flow.iter().enumerate() {
                    cap[u] = (cap[u] - f).max(0.0);
                    total_flow[u] += f;
                }
                for ((t, _), g) in takers.iter().zip(&got) {
                    if *g > 0.0 {
                        self.liquid_take(*t, Content::Exhaust, *g);
                    }
                }
            }
            let net = &self.nets[ni];
            for (e, node) in &net.sources {
                if let End::Exhaust(q) = e {
                    *piped.entry(*q).or_default() += total_flow[*node];
                }
            }
            self.nets[ni].flow = total_flow;
        }
        for e in &mut self.engines {
            for (i, c) in e.cylinders.iter().enumerate() {
                let out = piped.get(c).copied().unwrap_or(0.0);
                e.vented[i] = (e.drawn[i] - out).max(0.0);
            }
        }
    }

    /// The steam a cylinder drew last step, mB/s.
    fn cylinder_drawn(&self, q: Pos) -> f64 {
        let Some(&ei) = self.engine_of.get(&q) else {
            return 0.0;
        };
        let e = &self.engines[ei];
        e.cylinders
            .iter()
            .position(|c| *c == q)
            .map_or(0.0, |i| e.drawn[i])
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
    pub(crate) fn fill(&mut self, si: usize, mb: f64) {
        self.fill_shell(si, mb, AMBIENT);
    }

    fn fill_shell(&mut self, si: usize, mb: f64, t_feed: f64) {
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
            let flashed = b.feed(mb, t_feed);
            s.water = b.water;
            s.temp = b.temp;
            s.fed += mb / DT;
            s.feed_temp = t_feed;
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
            e.drawn.iter_mut().for_each(|f| *f = 0.0);
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
                    let g = self.throttle_on(ni, at);
                    let (t, d) = match e {
                        End::Cylinder(q) => (Taker::Cylinder(*q), self.cylinder_demand(*q, p) * g),
                        End::Vent(q) => (Taker::Vent(*q), self.vent_demand(*q, p, &mut vents) * g),
                        _ => continue,
                    };
                    takers.push(t);
                    demands.push(d);
                    nodes.push(at.clone());
                    // A leaking cylinder or vent leaks beside what it draws.
                    if let End::Cylinder(q) | End::Vent(q) = e
                        && self.parts[q].spec.leaking()
                        && !self.parts[q].spec.damaged
                    {
                        takers.push(Taker::Leak(*q));
                        demands.push(LEAK_RATE);
                        nodes.push(at.clone());
                    }
                }
                for q in &net.pipes {
                    let spec = &self.parts[q].spec;
                    if spec.damaged {
                        takers.push(Taker::Burst(*q));
                        demands.push(spec.throughput_now());
                        nodes.push(vec![net.node_of[q]]);
                    } else if spec.leaking() {
                        takers.push(Taker::Leak(*q));
                        demands.push(LEAK_RATE);
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
            // Leaking shell blocks vent straight from the pool.
            let mut shell_leaks: Vec<Pos> = Vec::new();
            for &si in &self.steam[pi].shells {
                let sh = &self.shells[si];
                if !sh.works() {
                    continue;
                }
                for b in &sh.blocks {
                    if self.parts.get(b).is_some_and(|x| x.spec.leaking()) {
                        shell_leaks.push(*b);
                    }
                }
            }
            let want = want + shell_leaks.len() as f64 * LEAK_RATE * DT;
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
                                    e.drawn[i] = got;
                                }
                            }
                        }
                        Taker::Vent(q) => {
                            let v = vents.entry(*q).or_default();
                            v.rate = got;
                        }
                        Taker::Leak(q) => {
                            self.leaks.insert(*q, (Content::Steam, got));
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
            for b in shell_leaks {
                let got = LEAK_RATE * scale;
                out += got;
                self.leaks.insert(b, (Content::Steam, got));
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
        for ei in 0..self.engines.len() {
            if self.plant_paused(self.engines[ei].plant) {
                continue;
            }
            let t: Vec<f64> = self.engines[ei]
                .cylinders
                .iter()
                .map(|c| self.cylinder_throttle(*c))
                .collect();
            self.engines[ei].throttle = t;
        }
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
                        d.stall += engine::stall(p, e.fill[i]) * e.throttle[i];
                        d.top = d.top.max(engine::top(p));
                    }
                }
                self.engines[ei].drive = d;
            }
            drives.insert(q, d);
        }
        self.drives = drives;
    }

    /// The governors' throttle on a cylinder (1 with none).
    pub fn cylinder_throttle(&self, q: Pos) -> f64 {
        let Some(&ni) = self.net_of.get(&q) else {
            return 1.0;
        };
        self.nets[ni]
            .consumers
            .iter()
            .find(|(e, _)| *e == End::Cylinder(q))
            .map_or(1.0, |(_, at)| self.throttle_on(ni, at))
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
