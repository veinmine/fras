//! Building the plant from its parts: multiblocks (spec 4), pipe networks
//! and their contents (spec 3), steam networks (spec 2, 5) and plants
//! (spec 2, 14.2). Everything is rebuilt when a part is placed, broken,
//! loaded, unloaded or changes how it connects; nothing is rebuilt on a timer.
//! Before a rebuild the built state goes back into the parts' shares, so a
//! rebuild that finds the same structures changes nothing.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::*;
use crate::sim::fluid::route;
use crate::sim::geom::Dir;
use crate::sim::multi::{boiler, engine, shape};
use crate::sim::rot::View;
use crate::sim::steam::PIPE_STEAM;

/// Effectively unlimited throughput (a tank's faces).
pub const UNLIMITED: f64 = 1e9;

/// A pipe network found: its pipe parts, endpoints (and where) and frontier.
type Comp = (Vec<Pos>, Vec<(End, Pos)>, Vec<Pos>);

/// How a part takes part in a pipe network through one face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Role {
    /// A pipe part: carries flow on.
    Pipe,
    /// A fitting with one pipe face: vent, cylinder inlet, pump outlet.
    Fit(End),
    /// A vessel: a boiler's dome or feed, a tank.
    Vessel(End),
}

/// A small union-find.
pub(crate) struct Dsu(Vec<usize>);

impl Dsu {
    pub fn new(n: usize) -> Dsu {
        Dsu((0..n).collect())
    }

    pub fn find(&mut self, a: usize) -> usize {
        let mut r = a;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut a = a;
        while self.0[a] != r {
            let next = self.0[a];
            self.0[a] = r;
            a = next;
        }
        r
    }

    pub fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // The lower index stays the root, so results don't depend on order.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.0[hi] = lo;
        }
    }
}

impl Works {
    /// Put the built state back into the parts and forget it (before the
    /// parts change). Does nothing while a rebuild is pending.
    pub(crate) fn dissolve(&mut self) {
        if self.dirty {
            return;
        }
        self.write_shares();
        self.shells.clear();
        self.shell_of.clear();
        self.refused.clear();
        self.nets.clear();
        self.net_of.clear();
        self.steam.clear();
        self.engines.clear();
        self.engine_of.clear();
        self.plants.clear();
        self.part_plant.clear();
        self.heats.clear();
    }

    /// Write each multiblock's water, temperature and timer and each steam
    /// network's steam back into its blocks' shares (spec 2, 14.3).
    pub(crate) fn write_shares(&mut self) {
        for s in &self.shells {
            if s.partial {
                continue;
            }
            let n = s.blocks.len().max(1) as f64;
            for b in &s.blocks {
                if let Some(p) = self.parts.get_mut(b) {
                    p.saved.water = s.water / n;
                    p.saved.temp = s.temp;
                    p.saved.timer = s.timer;
                    p.saved.content = s.content;
                }
            }
        }
        for net in &self.steam {
            let pool = net.pool;
            for &si in &net.shells {
                let s = &self.shells[si];
                let cap = s.block_steam_capacity();
                for b in &s.blocks {
                    if let Some(p) = self.parts.get_mut(b) {
                        p.saved.steam = pool.share(cap);
                    }
                }
            }
            for &ni in &net.nets {
                let vents: Vec<Pos> = self.nets[ni]
                    .consumers
                    .iter()
                    .filter_map(|(e, _)| match e {
                        End::Vent(q) => Some(*q),
                        _ => None,
                    })
                    .collect();
                for q in self.nets[ni].pipes.iter().chain(&vents) {
                    if let Some(p) = self.parts.get_mut(q) {
                        if p.spec.kind.is_pipe() {
                            p.saved.steam = pool.share(PIPE_STEAM);
                        }
                        // Pipe timers survive a rebuild.
                        p.saved.timer = super::step::timer_kind(p.spec.kind)
                            .and_then(|k| net.timers.get(&k))
                            .copied()
                            .unwrap_or(0.0);
                    }
                }
            }
        }
    }

    /// Rebuild everything if a part changed since the last build.
    pub fn rebuild(&mut self, view: &dyn View) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.rebuilds += 1;
        self.build_shells(view);
        self.build_engines();
        self.build_nets(view);
        self.build_steam();
        self.build_plants();
    }

    fn sorted_parts(&self, pred: impl Fn(&Spec) -> bool) -> Vec<Pos> {
        let mut out: Vec<Pos> = self
            .parts
            .iter()
            .filter(|(_, p)| pred(&p.spec))
            .map(|(q, _)| *q)
            .collect();
        out.sort();
        out
    }

    // ---- Multiblocks ---------------------------------------------------------

    fn build_shells(&mut self, view: &dyn View) {
        let mut done: BTreeSet<Pos> = BTreeSet::new();
        let mut in_shell: BTreeSet<Pos> = BTreeSet::new();
        for kind in ShellKind::ALL {
            let starts = self.sorted_parts(|s| s.kind == Kind::Shell(kind));
            for start in starts {
                if done.contains(&start) {
                    continue;
                }
                let member = |q: Pos| {
                    self.parts
                        .get(&q)
                        .is_some_and(|p| p.spec.kind == Kind::Shell(kind))
                };
                // The whole group, for marking; capped to stay cheap.
                let whole = shape::group(start, &member, 4096);
                done.extend(whole.blocks.iter().copied());
                let saved = whole
                    .blocks
                    .iter()
                    .find_map(|q| self.parts[q].saved.formation);
                // Part of its saved multiblock is unloaded: keep it as saved.
                if let Some(f) = saved
                    && f.positions().iter().any(|q| !view.loaded(*q))
                {
                    let blocks: Vec<Pos> = whole
                        .blocks
                        .iter()
                        .copied()
                        .filter(|q| f.contains(*q))
                        .collect();
                    in_shell.extend(blocks.iter().copied());
                    self.add_shell(kind, f, blocks, true);
                    continue;
                }
                let capped = shape::Group {
                    too_big: whole.blocks.len() > kind.limit(),
                    blocks: whole.blocks.clone(),
                };
                match shape::check(&capped, kind.limit()) {
                    Ok(c) => {
                        in_shell.extend(whole.blocks.iter().copied());
                        self.add_shell(kind, c, whole.blocks, false);
                    }
                    Err(r) => {
                        for q in &whole.blocks {
                            self.refused.insert(*q, r.clone());
                        }
                    }
                }
            }
        }
        // Blocks no longer in a multiblock: inert, their steam goes.
        let lone = self.sorted_parts(|s| matches!(s.kind, Kind::Shell(_)));
        for q in lone {
            if in_shell.contains(&q) {
                continue;
            }
            let p = self.parts.get_mut(&q).unwrap();
            if p.saved.steam > 1.0 {
                self.events.push(Event::Puff(q));
            }
            p.saved.steam = 0.0;
            if p.saved.formation.is_some() {
                p.saved.formation = None;
                self.formations.push((q, None));
            }
        }
        // Domes on working boilers.
        for d in self.sorted_parts(|s| s.kind == Kind::Dome) {
            if let Some(&si) = self.shell_of.get(&d.offset(Dir::Down))
                && self.shells[si].is_boiler()
                && self.shells[si].works()
            {
                self.shells[si].domes.push(d);
            }
        }
        // The fireboxes that heat each boiler.
        for si in 0..self.shells.len() {
            let s = &self.shells[si];
            if !s.is_boiler() || !s.works() {
                continue;
            }
            for spot in boiler::firebox_spots(&s.cuboid) {
                if self
                    .parts
                    .get(&spot)
                    .is_some_and(|p| p.spec.kind == Kind::Firebox)
                {
                    self.heats.entry(spot).or_default().push(si);
                }
            }
        }
    }

    fn add_shell(&mut self, kind: ShellKind, c: Cuboid, blocks: Vec<Pos>, partial: bool) {
        let shares: Vec<boiler::Share> = blocks
            .iter()
            .map(|q| {
                let s = &self.parts[q].saved;
                boiler::Share {
                    water: s.water,
                    temp: s.temp,
                }
            })
            .collect();
        let merged = boiler::merge(&shares);
        let timer = blocks
            .iter()
            .map(|q| self.parts[q].saved.timer)
            .fold(0.0, f64::max);
        let content = blocks.iter().find_map(|q| self.parts[q].saved.content);
        let damaged = blocks.iter().any(|q| self.parts[q].spec.damaged);
        let index = self.shells.len();
        for q in &blocks {
            self.shell_of.insert(*q, index);
            let p = self.parts.get_mut(q).unwrap();
            if p.saved.formation != Some(c) && !partial {
                p.saved.formation = Some(c);
                self.formations.push((*q, Some(Formation { cuboid: c })));
            }
        }
        self.shells.push(Shell {
            kind,
            cuboid: c,
            blocks,
            damaged,
            partial,
            ruptured: false,
            water: merged.water,
            temp: merged.temp,
            timer,
            content: if kind.is_boiler() { None } else { content },
            domes: Vec::new(),
            pool: None,
            plant: 0,
            heat: 0.0,
            fireboxes: 0,
            boiled: 0.0,
            fed: 0.0,
        });
    }

    // ---- Engines ---------------------------------------------------------------

    fn build_engines(&mut self) {
        for hub in self.sorted_parts(|s| s.kind == Kind::Flywheel) {
            let fly = self.parts[&hub].spec;
            let mut cylinders = Vec::new();
            if !fly.damaged {
                for (spot, facing) in engine::cylinder_spots(hub, fly.axis()) {
                    if let Some(c) = self.parts.get(&spot)
                        && c.spec.kind == Kind::Cylinder
                        && c.spec.dir == facing
                    {
                        cylinders.push(spot);
                    }
                }
            }
            if cylinders.is_empty() {
                continue;
            }
            let index = self.engines.len();
            self.engine_of.insert(hub, index);
            for c in &cylinders {
                self.engine_of.insert(*c, index);
            }
            self.engines.push(Engine {
                flywheel: hub,
                demand: vec![0.0; cylinders.len()],
                fill: vec![1.0; cylinders.len()],
                cylinders,
                drive: Drive::default(),
                plant: 0,
            });
        }
        // Cylinders tell their blocks whether they are part of an engine.
        for c in self.sorted_parts(|s| s.kind == Kind::Cylinder) {
            let formed = self.engine_of.get(&c).map(|&e| Cuboid {
                min: self.engines[e].flywheel,
                size: [1, 1, 1],
            });
            let p = self.parts.get_mut(&c).unwrap();
            if p.saved.formation != formed {
                p.saved.formation = formed;
                self.formations
                    .push((c, formed.map(|cuboid| Formation { cuboid })));
            }
        }
    }

    // ---- Pipe networks -----------------------------------------------------------

    /// How the part at `pos` takes part in a pipe network through `face`.
    pub fn role(&self, pos: Pos, face: Dir) -> Option<Role> {
        let p = self.parts.get(&pos)?;
        let s = &p.spec;
        match s.kind {
            Kind::Pipe(_) => s.face(face).then_some(Role::Pipe),
            Kind::Valve { .. } => (face.axis() == s.axis() && s.open()).then_some(Role::Pipe),
            Kind::Vent { .. } => (face == s.dir).then_some(Role::Fit(End::Vent(pos))),
            Kind::Cylinder => (face == s.dir.opposite()).then_some(Role::Fit(End::Cylinder(pos))),
            Kind::Pump => (face == s.dir.opposite()).then_some(Role::Fit(End::Pump(pos))),
            Kind::Dome => {
                let &si = self.shell_of.get(&pos.offset(Dir::Down))?;
                let sh = &self.shells[si];
                (face != Dir::Down && sh.is_boiler() && sh.works())
                    .then_some(Role::Vessel(End::Domes(si)))
            }
            Kind::Shell(kind) => {
                let &si = self.shell_of.get(&pos)?;
                if !self.shells[si].works() {
                    return None;
                }
                if kind.is_boiler() {
                    (face != Dir::Up).then_some(Role::Vessel(End::Feed(si)))
                } else {
                    Some(Role::Vessel(End::Tank(si)))
                }
            }
            Kind::Firebox | Kind::Flywheel => None,
        }
    }

    /// The content a vessel end gives, with a tank's own.
    fn end_content(&self, e: End) -> Option<Content> {
        match e {
            End::Tank(si) => self.shells[si].content,
            e => e.content(),
        }
    }

    /// Whether two roles facing each other connect.
    fn links(&self, a: Role, b: Role) -> bool {
        match (a, b) {
            (Role::Pipe, _) | (_, Role::Pipe) => true,
            (Role::Fit(f), Role::Vessel(v)) | (Role::Vessel(v), Role::Fit(f)) => {
                match (f.content(), self.end_content(v), v) {
                    (_, _, End::Tank(_)) => {
                        self.end_content(v).is_none_or(|c| Some(c) == f.content())
                    }
                    (fc, vc, _) => fc == vc,
                }
            }
            _ => false,
        }
    }

    /// The role across `face` of the part at `pos`, if the two connect.
    pub fn neighbour(&self, pos: Pos, face: Dir) -> Option<(Pos, Role)> {
        let mine = self.role(pos, face)?;
        let q = pos.offset(face);
        let theirs = self.role(q, face.opposite())?;
        self.links(mine, theirs).then_some((q, theirs))
    }

    fn build_nets(&mut self, view: &dyn View) {
        let pipes = self.sorted_parts(|s| s.kind.is_pipe() && s.open());
        let mut seen: BTreeSet<Pos> = BTreeSet::new();
        let mut comps: Vec<Comp> = Vec::new();
        for start in pipes {
            if seen.contains(&start) {
                continue;
            }
            let mut members = vec![start];
            let mut ends = Vec::new();
            let mut frontier = Vec::new();
            seen.insert(start);
            let mut i = 0;
            while i < members.len() {
                let p = members[i];
                i += 1;
                for d in Dir::ALL {
                    if self.role(p, d) != Some(Role::Pipe) {
                        continue;
                    }
                    let q = p.offset(d);
                    if !view.loaded(q) {
                        frontier.push(q);
                        continue;
                    }
                    match self.neighbour(p, d) {
                        Some((q, Role::Pipe)) => {
                            if seen.insert(q) {
                                members.push(q);
                            }
                        }
                        Some((_, Role::Fit(e) | Role::Vessel(e))) => ends.push((e, p)),
                        None => {}
                    }
                }
            }
            members.sort();
            comps.push((members, ends, frontier));
        }
        // Fittings straight on a vessel, with no pipe between.
        for f in
            self.sorted_parts(|s| matches!(s.kind, Kind::Vent { .. } | Kind::Cylinder | Kind::Pump))
        {
            let face = self.parts[&f].spec.dir;
            let face = if matches!(self.parts[&f].spec.kind, Kind::Vent { .. }) {
                face
            } else {
                face.opposite()
            };
            if let Some((_, Role::Vessel(v))) = self.neighbour(f, face)
                && let Some(Role::Fit(e)) = self.role(f, face)
            {
                comps.push((Vec::new(), vec![(v, f), (e, f)], Vec::new()));
            }
        }
        for (pipes, ends, frontier) in comps {
            self.add_net(pipes, ends, frontier);
        }
    }

    fn add_net(&mut self, pipes: Vec<Pos>, ends: Vec<(End, Pos)>, frontier: Vec<Pos>) {
        let index = self.nets.len();
        let mut net = Net {
            pipes: pipes.clone(),
            frontier,
            ..Net::default()
        };
        for q in &pipes {
            let cap = self.parts[q].spec.kind.throughput();
            let node = net.graph.add(cap, *q);
            net.node_of.insert(*q, node);
            self.net_of.insert(*q, index);
        }
        for q in &pipes {
            for d in Dir::ALL {
                if let Some((r, Role::Pipe)) = self.neighbour(*q, d)
                    && let Some(&b) = net.node_of.get(&r)
                {
                    let a = net.node_of[q];
                    net.graph.link(a, b);
                }
            }
        }
        // Group the endpoints: each one's attachment nodes.
        let direct = pipes.is_empty();
        let mut attach: BTreeMap<End, Vec<Pos>> = BTreeMap::new();
        let mut domes: BTreeMap<usize, BTreeSet<Pos>> = BTreeMap::new();
        for (e, at) in &ends {
            attach.entry(*e).or_default().push(*at);
            if let End::Domes(si) = e {
                // Which dome: the vessel across from the pipe.
                for d in Dir::ALL {
                    let q = at.offset(d);
                    if self
                        .parts
                        .get(&q)
                        .is_some_and(|p| p.spec.kind == Kind::Dome)
                        && self.shell_of.get(&q.offset(Dir::Down)) == Some(si)
                        && (direct || self.neighbour(*at, d).is_some())
                    {
                        domes.entry(*si).or_default().insert(q);
                    }
                }
            }
            if let End::Vent(v) | End::Cylinder(v) | End::Pump(v) = e {
                self.net_of.insert(*v, index);
            }
        }
        // Its content, from its endpoints (spec 3, 3.2).
        let mut steam = false;
        let mut water = false;
        let mut empty_tanks = Vec::new();
        for e in attach.keys() {
            match self.end_content(*e) {
                Some(Content::Steam) => steam = true,
                Some(Content::Water) => water = true,
                None => empty_tanks.push(*e),
            }
        }
        net.mixed = steam && water;
        net.content = match (steam, water) {
            (true, false) => Some(Content::Steam),
            (false, true) => Some(Content::Water),
            _ => None,
        };
        // An empty tank takes the content of the first network it meets.
        if let Some(c) = net.content {
            for e in empty_tanks {
                if let End::Tank(si) = e {
                    self.shells[si].content = Some(c);
                }
            }
        }
        // Source nodes and consumers.
        let mut pump_sources = Vec::new();
        let mut tank_sources = Vec::new();
        for (e, ats) in &attach {
            let source_cap = match (*e, net.content) {
                (End::Domes(si), Some(Content::Steam)) => Some(
                    crate::sim::fluid::DOME * domes.get(&si).map_or(1, |d| d.len().max(1)) as f64,
                ),
                (End::Tank(_), Some(_)) => Some(UNLIMITED),
                (End::Pump(_), Some(Content::Water)) => Some(crate::sim::fluid::pump::source_cap(
                    crate::sim::fluid::pump::MAX_SOURCES,
                )),
                _ => None,
            };
            let nodes: Vec<usize> = if direct {
                Vec::new()
            } else {
                ats.iter().map(|q| net.node_of[q]).collect()
            };
            if let Some(cap) = source_cap {
                let key = ats[0];
                let node = net.graph.add(cap, key);
                for &n in &nodes {
                    net.graph.link(node, n);
                }
                net.sources.push((*e, node));
                match e {
                    End::Pump(_) => pump_sources.push(node),
                    End::Tank(_) if net.content == Some(Content::Water) => tank_sources.push(node),
                    _ => {}
                }
            }
            let consumer = match e {
                End::Vent(_) | End::Cylinder(_) => net.content == Some(Content::Steam),
                End::Feed(_) => net.content == Some(Content::Water),
                End::Tank(_) => net.content == Some(Content::Water),
                _ => false,
            };
            if consumer {
                net.consumers.push((*e, nodes));
            }
        }
        // A direct link: the consumer hangs on the vessel's source node.
        if direct {
            let src: Vec<usize> = net.sources.iter().map(|(_, n)| *n).collect();
            for (_, nodes) in net.consumers.iter_mut() {
                nodes.extend(src.iter().copied());
            }
        }
        match net.content {
            Some(Content::Steam) => {
                let all: Vec<usize> = net.sources.iter().map(|(_, n)| *n).collect();
                net.tree = route::tree(&net.graph, &all);
            }
            Some(Content::Water) => {
                net.tree = route::tree(&net.graph, &pump_sources);
                net.tank_tree = route::tree(&net.graph, &tank_sources);
            }
            None => {}
        }
        net.flow = vec![0.0; net.graph.len()];
        self.nets.push(net);
    }

    // ---- Steam networks ---------------------------------------------------------

    fn build_steam(&mut self) {
        let ns = self.shells.len();
        let nn = self.nets.len();
        let mut dsu = Dsu::new(ns + nn);
        let shell_item = |si: usize| si;
        let net_item = |ni: usize| ns + ni;
        let steam_shell =
            |s: &Shell| s.works() && (s.is_boiler() || s.content == Some(Content::Steam));
        for (ni, net) in self.nets.iter().enumerate() {
            if net.content != Some(Content::Steam) || net.mixed {
                continue;
            }
            for (e, _) in &net.sources {
                if let End::Domes(si) | End::Tank(si) = e {
                    dsu.union(net_item(ni), shell_item(*si));
                }
            }
        }
        let mut groups: BTreeMap<usize, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        for si in 0..ns {
            if steam_shell(&self.shells[si]) {
                groups
                    .entry(dsu.find(shell_item(si)))
                    .or_default()
                    .0
                    .push(si);
            }
        }
        for ni in 0..nn {
            let net = &self.nets[ni];
            if net.content == Some(Content::Steam) && !net.mixed {
                groups.entry(dsu.find(net_item(ni))).or_default().1.push(ni);
            }
        }
        // Steam held where no steam network is (water pipes, idle tanks) is gone.
        let in_steam: BTreeSet<Pos> = groups
            .values()
            .flat_map(|(ss, nn)| {
                ss.iter()
                    .flat_map(|&s| self.shells[s].blocks.clone())
                    .chain(nn.iter().flat_map(|&n| self.nets[n].pipes.clone()))
            })
            .collect();
        for (q, p) in self.parts.iter_mut() {
            if !in_steam.contains(q) {
                p.saved.steam = 0.0;
            }
        }
        for (shells, nets) in groups.into_values() {
            let index = self.steam.len();
            let mut steam = 0.0;
            let mut capacity = 0.0;
            let mut red = f64::INFINITY;
            let mut red_by = None;
            let mut timers: BTreeMap<u8, f64> = BTreeMap::new();
            let keep_timer = |p: &Part, timers: &mut BTreeMap<u8, f64>| {
                if let Some(k) = super::step::timer_kind(p.spec.kind)
                    && p.saved.timer > 0.0
                {
                    let t = timers.entry(k).or_insert(0.0);
                    *t = t.max(p.saved.timer);
                }
            };
            let rate = |kind: Kind, red: &mut f64, red_by: &mut Option<Kind>| {
                if let Some(r) = kind.rating()
                    && r < *red
                {
                    *red = r;
                    *red_by = Some(kind);
                }
            };
            for &si in &shells {
                let s = &self.shells[si];
                let cap = s.block_steam_capacity();
                for b in &s.blocks {
                    steam += self.parts[b].saved.steam;
                    capacity += cap;
                }
                rate(Kind::Shell(s.kind), &mut red, &mut red_by);
                self.shells[si].pool = Some(index);
            }
            for &ni in &nets {
                let net = &self.nets[ni];
                for q in &net.pipes {
                    let p = &self.parts[q];
                    steam += p.saved.steam;
                    capacity += PIPE_STEAM;
                    rate(p.spec.kind, &mut red, &mut red_by);
                    keep_timer(p, &mut timers);
                }
                for (e, _) in &net.consumers {
                    if let End::Vent(q) | End::Cylinder(q) = e {
                        let p = &self.parts[q];
                        rate(p.spec.kind, &mut red, &mut red_by);
                        if let End::Vent(_) = e {
                            keep_timer(p, &mut timers);
                        }
                    }
                }
                self.nets[ni].pool = Some(index);
            }
            self.steam.push(SteamNet {
                pool: Pool::new(steam, capacity),
                shells,
                nets,
                red: if red.is_finite() { red } else { 0.0 },
                red_by,
                timers,
                ..SteamNet::default()
            });
        }
    }

    // ---- Plants ------------------------------------------------------------------

    fn build_plants(&mut self) {
        let ns = self.shells.len();
        let nn = self.nets.len();
        let ne = self.engines.len();
        // Parts outside shells, nets and engines get items of their own.
        let loose = self.sorted_parts(|s| {
            matches!(
                s.kind,
                Kind::Firebox
                    | Kind::Flywheel
                    | Kind::Pump
                    | Kind::Dome
                    | Kind::Cylinder
                    | Kind::Vent { .. }
            )
        });
        let loose_item: HashMap<Pos, usize> = loose
            .iter()
            .enumerate()
            .map(|(i, q)| (*q, ns + nn + ne + i))
            .collect();
        let mut dsu = Dsu::new(ns + nn + ne + loose.len());
        let net_item = |ni: usize| ns + ni;
        let engine_item = |ei: usize| ns + nn + ei;
        for (ni, net) in self.nets.iter().enumerate() {
            for (e, _) in net
                .sources
                .iter()
                .map(|(e, n)| (e, vec![*n]))
                .chain(net.consumers.iter().map(|(e, n)| (e, n.clone())))
            {
                match e {
                    End::Domes(si) | End::Feed(si) | End::Tank(si) => dsu.union(net_item(ni), *si),
                    End::Vent(q) | End::Cylinder(q) | End::Pump(q) => {
                        if let Some(&it) = loose_item.get(q) {
                            dsu.union(net_item(ni), it);
                        }
                    }
                }
            }
        }
        for sn in &self.steam {
            let first = sn
                .shells
                .first()
                .copied()
                .or_else(|| sn.nets.first().map(|&n| net_item(n)));
            if let Some(first) = first {
                for &s in &sn.shells {
                    dsu.union(first, s);
                }
                for &n in &sn.nets {
                    dsu.union(first, net_item(n));
                }
            }
        }
        for (ei, e) in self.engines.iter().enumerate() {
            for q in std::iter::once(&e.flywheel).chain(&e.cylinders) {
                if let Some(&it) = loose_item.get(q) {
                    dsu.union(engine_item(ei), it);
                }
            }
        }
        for (spot, boilers) in &self.heats {
            if let Some(&it) = loose_item.get(spot) {
                for &b in boilers {
                    dsu.union(it, b);
                }
            }
        }
        for (si, s) in self.shells.iter().enumerate() {
            for d in &s.domes {
                if let Some(&it) = loose_item.get(d) {
                    dsu.union(it, si);
                }
            }
        }
        // Number the plants by their roots.
        let mut ids: BTreeMap<usize, usize> = BTreeMap::new();
        let total = ns + nn + ne + loose.len();
        let mut plant_of = vec![0; total];
        for (it, slot) in plant_of.iter_mut().enumerate() {
            let r = dsu.find(it);
            let next = ids.len();
            *slot = *ids.entry(r).or_insert(next);
        }
        self.plants = vec![Plant::default(); ids.len()];
        for (si, &p) in plant_of.iter().enumerate().take(ns) {
            self.shells[si].plant = p;
            let s = &self.shells[si];
            let plant = &mut self.plants[p];
            plant.partial |= s.partial;
            for q in s.cuboid.positions() {
                plant.chunks.insert(q.chunk());
            }
        }
        for ni in 0..nn {
            let p = plant_of[net_item(ni)];
            self.nets[ni].plant = p;
            let net = &self.nets[ni];
            let plant = &mut self.plants[p];
            plant.frontier.extend(net.frontier.iter().copied());
            for q in &net.pipes {
                plant.chunks.insert(q.chunk());
            }
        }
        for sn in self.steam.iter_mut() {
            sn.plant = sn
                .shells
                .first()
                .map(|&s| self.shells[s].plant)
                .or_else(|| sn.nets.first().map(|&n| self.nets[n].plant))
                .unwrap_or(0);
        }
        for ei in 0..ne {
            self.engines[ei].plant = plant_of[engine_item(ei)];
        }
        for q in loose {
            let p = plant_of[loose_item[&q]];
            self.part_plant.insert(q, p);
            let plant = &mut self.plants[p];
            plant.chunks.insert(q.chunk());
            if matches!(self.parts[&q].spec.kind, Kind::Flywheel | Kind::Pump) {
                plant.rot_parts.push(q);
            }
        }
    }
}
