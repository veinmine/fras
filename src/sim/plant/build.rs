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
        self.bellows_of.clear();
        self.ex_nets.clear();
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
        // Dampers on working boilers (regulation spec 4.1).
        for d in self.sorted_parts(|s| s.kind == Kind::Damper) {
            let on = d.offset(self.parts[&d].spec.dir.opposite());
            if let Some(&si) = self.shell_of.get(&on)
                && self.shells[si].is_boiler()
                && self.shells[si].works()
            {
                self.shells[si].dampers.push(d);
            }
        }
        // Each firebox's bellows: the first by position, not on its mouth.
        for b in self.sorted_parts(|s| s.kind == Kind::Bellows) {
            let nozzle = self.parts[&b].spec.dir;
            let fire = b.offset(nozzle);
            if let Some(f) = self.parts.get(&fire)
                && f.spec.kind == Kind::Firebox
                && f.spec.dir != nozzle.opposite()
            {
                self.bellows_of.entry(fire).or_insert(b);
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
            dampers: Vec::new(),
            opening: 1.0,
            feed_temp: crate::sim::steam::AMBIENT,
            leaked: 0.0,
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
                throttle: vec![1.0; cylinders.len()],
                drawn: vec![0.0; cylinders.len()],
                vented: vec![0.0; cylinders.len()],
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
            Kind::Governor => (face.axis() == s.axis()).then_some(Role::Pipe),
            Kind::Vent { .. } => (face == s.dir).then_some(Role::Fit(End::Vent(pos))),
            Kind::Cylinder if face == Dir::Up && s.dir.axis() != crate::sim::geom::Axis::Y => {
                Some(Role::Fit(End::Exhaust(pos)))
            }
            Kind::Cylinder => (face == s.dir.opposite()).then_some(Role::Fit(End::Cylinder(pos))),
            Kind::Exchanger => {
                if face == s.dir {
                    Some(Role::Fit(End::ExOut(pos)))
                } else if face == s.dir.opposite() {
                    Some(Role::Fit(End::ExIn(pos)))
                } else {
                    None
                }
            }
            Kind::Drain => (face == s.dir).then_some(Role::Fit(End::Drain(pos))),
            Kind::Grinder => {
                if face == s.dir.opposite() {
                    Some(Role::Fit(End::CoolIn(pos)))
                } else if face.axis() != s.dir.axis() && face.axis() != crate::sim::geom::Axis::Y {
                    Some(Role::Fit(End::CoolOut(pos)))
                } else {
                    None
                }
            }
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
            Kind::Firebox
            | Kind::Flywheel
            | Kind::Damper
            | Kind::Bellows
            | Kind::Radiator { .. } => None,
        }
    }

    /// The faces through which a fitting takes part in pipe networks.
    pub(crate) fn fit_faces(&self, pos: Pos) -> Vec<Dir> {
        Dir::ALL
            .into_iter()
            .filter(|d| matches!(self.role(pos, *d), Some(Role::Fit(_))))
            .collect()
    }

    /// The content a vessel end gives, with a tank's own.
    fn end_content(&self, e: End) -> Option<Content> {
        match e {
            End::Tank(si) => self.shells[si].content,
            e => e.content(),
        }
    }

    /// Whether two ends may share a network: their contents agree, or one
    /// takes what the other brings.
    fn agree(&self, a: End, b: End) -> bool {
        match (self.end_content(a), self.end_content(b)) {
            (Some(x), Some(y)) => x == y,
            (Some(c), None) => b.accepts(c),
            (None, Some(c)) => a.accepts(c),
            (None, None) => true,
        }
    }

    /// Whether two roles facing each other connect.
    fn links(&self, a: Role, b: Role) -> bool {
        match (a, b) {
            (Role::Pipe, _) | (_, Role::Pipe) => true,
            (Role::Fit(f), Role::Vessel(v)) | (Role::Vessel(v), Role::Fit(f)) => self.agree(f, v),
            (Role::Fit(f), Role::Fit(g)) => self.agree(f, g),
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
        // Fittings straight on a vessel or on another fitting, with no pipe
        // between (a pair of fittings once, from the lower position).
        let fittings = self.sorted_parts(|s| {
            matches!(
                s.kind,
                Kind::Vent { .. }
                    | Kind::Cylinder
                    | Kind::Pump
                    | Kind::Exchanger
                    | Kind::Drain
                    | Kind::Grinder
            )
        });
        for f in fittings {
            for face in self.fit_faces(f) {
                let Some(Role::Fit(e)) = self.role(f, face) else {
                    continue;
                };
                match self.neighbour(f, face) {
                    Some((_, Role::Vessel(v))) => {
                        comps.push((Vec::new(), vec![(v, f), (e, f)], Vec::new()));
                    }
                    Some((g, Role::Fit(o))) if f < g => {
                        comps.push((Vec::new(), vec![(e, f), (o, g)], Vec::new()));
                    }
                    _ => {}
                }
            }
        }
        for (pipes, ends, frontier) in comps {
            self.add_net(pipes, ends, frontier);
        }
        self.classify_nets();
    }

    /// A pipe network's parts, links and endpoints; its content and routes
    /// come once every network is known (`classify_nets`).
    fn add_net(&mut self, pipes: Vec<Pos>, ends: Vec<(End, Pos)>, frontier: Vec<Pos>) {
        let index = self.nets.len();
        let mut net = Net {
            pipes: pipes.clone(),
            frontier,
            ends,
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
        for (e, _) in &net.ends {
            match e {
                End::Vent(v) | End::Cylinder(v) | End::Pump(v) => {
                    self.net_of.insert(*v, index);
                }
                End::ExIn(v) => self.ex_nets.entry(*v).or_default().0 = Some(index),
                End::ExOut(v) => self.ex_nets.entry(*v).or_default().1 = Some(index),
                _ => {}
            }
        }
        self.nets.push(net);
    }

    /// Each network's content (spec 3, 3.2; cooling spec 2): from the ends
    /// that set one, else from an exchanger outlet's inlet; mixed if two
    /// contents meet or an end can't take it. Then its sources, consumers
    /// and routes.
    fn classify_nets(&mut self) {
        let n = self.nets.len();
        let mut content: Vec<Option<Content>> = vec![None; n];
        let mut mixed = vec![false; n];
        // Setters fix a network's content; an empty tank takes the content of
        // the first network it meets and passes it on; outlets give what their
        // inlets turn into (chains of exchangers too).
        for _ in 0..8 {
            let mut changed = false;
            for ni in 0..n {
                if content[ni].is_some() || mixed[ni] {
                    continue;
                }
                let mut found: BTreeSet<Content> = BTreeSet::new();
                for (e, _) in &self.nets[ni].ends {
                    if let Some(c) = self.end_content(*e) {
                        found.insert(c);
                    }
                    if let End::ExOut(x) = e
                        && let Some(c) = self
                            .ex_nets
                            .get(x)
                            .and_then(|(i, _)| *i)
                            .and_then(|i| content[i])
                            .and_then(crate::sim::fluid::exchange::converts)
                    {
                        found.insert(c);
                    }
                }
                if found.len() > 1 {
                    mixed[ni] = true;
                    changed = true;
                } else if let Some(c) = found.into_iter().next() {
                    content[ni] = Some(c);
                    changed = true;
                    let tanks: Vec<usize> = self.nets[ni]
                        .ends
                        .iter()
                        .filter_map(|(e, _)| match e {
                            End::Tank(si) => Some(*si),
                            _ => None,
                        })
                        .collect();
                    for si in tanks {
                        if self.shells[si].content.is_none() {
                            self.shells[si].content = Some(c);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for ni in 0..n {
            if let Some(c) = content[ni]
                && self.nets[ni].ends.iter().any(|(e, _)| !e.accepts(c))
            {
                mixed[ni] = true;
            }
            if mixed[ni] {
                content[ni] = None;
            }
        }
        for ni in 0..n {
            self.finish_net(ni, content[ni], mixed[ni]);
        }
    }

    fn finish_net(&mut self, index: usize, content: Option<Content>, mixed: bool) {
        let mut net = std::mem::take(&mut self.nets[index]);
        net.content = content;
        net.mixed = mixed;
        let pipes = net.pipes.clone();
        let direct = pipes.is_empty();
        // Group the endpoints: each one's attachment nodes.
        let mut attach: BTreeMap<End, Vec<Pos>> = BTreeMap::new();
        let mut domes: BTreeMap<usize, BTreeSet<Pos>> = BTreeMap::new();
        for (e, at) in &net.ends {
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
        }
        // An empty tank takes the content of the first network it meets.
        if let Some(c) = net.content {
            for e in attach.keys() {
                if let End::Tank(si) = e
                    && self.shells[*si].content.is_none()
                {
                    self.shells[*si].content = Some(c);
                }
            }
        }
        // Source nodes and consumers.
        let mut plain_sources = Vec::new();
        let mut tank_sources = Vec::new();
        let liquid = net.content.is_some_and(Content::is_liquid);
        for (e, ats) in &attach {
            let source_cap = match (*e, net.content) {
                (End::Domes(si), Some(Content::Steam)) => Some(
                    crate::sim::fluid::DOME * domes.get(&si).map_or(1, |d| d.len().max(1)) as f64,
                ),
                (End::Tank(_), Some(_)) => Some(UNLIMITED),
                (End::Pump(_), Some(Content::Water)) => Some(crate::sim::fluid::pump::source_cap(
                    crate::sim::fluid::pump::MAX_SOURCES,
                )),
                (End::ExOut(_), Some(c)) if c.is_liquid() => {
                    Some(crate::sim::fluid::exchange::THROUGHPUT)
                }
                (End::CoolOut(_), Some(Content::HotWater)) => Some(UNLIMITED),
                (End::Exhaust(_), Some(Content::Exhaust)) => Some(UNLIMITED),
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
                    End::Tank(_) if liquid => tank_sources.push(node),
                    _ => plain_sources.push(node),
                }
            }
            let consumer = match (e, net.content) {
                (End::Vent(_) | End::Cylinder(_), Some(Content::Steam)) => true,
                (End::Feed(_) | End::Tank(_) | End::Drain(_), Some(c)) => c.is_liquid(),
                (End::CoolIn(_), Some(Content::Water)) => true,
                (End::ExIn(_), Some(Content::HotWater | Content::Exhaust)) => true,
                _ => false,
            };
            if consumer {
                net.consumers.push((*e, nodes));
            }
        }
        // A direct link: the consumer hangs on the other end's source node.
        if direct {
            let src: Vec<usize> = net.sources.iter().map(|(_, n)| *n).collect();
            for (_, nodes) in net.consumers.iter_mut() {
                nodes.extend(src.iter().copied());
            }
        }
        // Cooled machines route first, exchangers last: a machine that can't
        // get its hot water out loses cooling, an exchanger only lets
        // exhaust vent.
        let rank = |node: usize| match net.sources.iter().find(|(_, n)| *n == node) {
            Some((End::CoolOut(_), _)) => 0,
            Some((End::ExOut(_), _)) => 2,
            _ => 1,
        };
        plain_sources.sort_by_key(|&n| (rank(n), net.graph.key[n]));
        match net.content {
            Some(c) if liquid => {
                let _ = c;
                net.tree = route::tree(&net.graph, &plain_sources);
                net.tank_tree = route::tree(&net.graph, &tank_sources);
                net.source_trees = plain_sources
                    .iter()
                    .map(|&s| route::tree(&net.graph, &[s]))
                    .collect();
            }
            Some(c) => {
                let all: Vec<usize> = net.sources.iter().map(|(_, n)| *n).collect();
                net.tree = route::tree(&net.graph, &all);
                if c == Content::Exhaust {
                    net.source_trees = all.iter().map(|&s| route::tree(&net.graph, &[s])).collect();
                }
            }
            None => {}
        }
        // The governors on the route to each node (steam only).
        net.govs = vec![Vec::new(); net.graph.len()];
        if net.content == Some(Content::Steam) {
            for &u in &net.tree.order.clone() {
                let mut g = net.tree.parent[u]
                    .map(|p| net.govs[p].clone())
                    .unwrap_or_default();
                let key = net.graph.key[u];
                if net.node_of.get(&key) == Some(&u)
                    && self
                        .parts
                        .get(&key)
                        .is_some_and(|p| p.spec.kind == Kind::Governor)
                {
                    g.push(key);
                }
                net.govs[u] = g;
            }
        }
        net.flow = vec![0.0; net.graph.len()];
        self.nets[index] = net;
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
        for (si, shell) in self.shells.iter().enumerate() {
            if steam_shell(shell) {
                groups
                    .entry(dsu.find(shell_item(si)))
                    .or_default()
                    .0
                    .push(si);
            }
        }
        for (ni, net) in self.nets.iter().enumerate() {
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
                    | Kind::Damper
                    | Kind::Bellows
                    | Kind::Exchanger
                    | Kind::Drain
                    | Kind::Grinder
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
                    e => {
                        if let Some(it) = e.pos().and_then(|q| loose_item.get(&q)) {
                            dsu.union(net_item(ni), *it);
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
            for d in s.domes.iter().chain(&s.dampers) {
                if let Some(&it) = loose_item.get(d) {
                    dsu.union(it, si);
                }
            }
        }
        for (fire, b) in &self.bellows_of {
            if let (Some(&a), Some(&c)) = (loose_item.get(fire), loose_item.get(b)) {
                dsu.union(a, c);
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
                if self.parts[q].spec.kind == Kind::Governor {
                    plant.rot_parts.push(*q);
                }
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
            if self.parts[&q].spec.kind.is_rot() {
                plant.rot_parts.push(q);
            }
        }
    }
}
