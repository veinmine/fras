//! What the wrench overlay, the gauges and the blocks read from the plant
//! (steam and heat spec 10 and 11, fluids spec 9). The game words them.

use super::*;
use crate::sim::steam::{self, HOT_PIPE_BAR, engine};

/// A pressure gauge's reading.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Gauge {
    pub bar: f64,
    /// The network's red line (0 on a water network or nothing).
    pub red: f64,
}

impl Gauge {
    /// Comparator output, 0 to 15: linear from 0 bar to the red line.
    pub fn signal(&self) -> u8 {
        if self.red <= 0.0 || self.bar <= 0.0 {
            0
        } else {
            ((self.bar / self.red) * 15.0).floor().clamp(0.0, 15.0) as u8
        }
    }

    /// 0 normal, 1 yellow (from 90% of the red line), 2 red (over it).
    pub fn zone(&self) -> u8 {
        if self.red <= 0.0 {
            0
        } else if self.bar > self.red {
            2
        } else if self.bar >= self.red * crate::sim::steam::fail::SIMMER_SHARE {
            1
        } else {
            0
        }
    }
}

/// A steam network as the overlay shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct SteamInfo {
    pub bar: f64,
    pub red: f64,
    pub red_by: Option<Kind>,
    /// mB per bar.
    pub capacity: f64,
    pub steam_in: f64,
    pub steam_out: f64,
    pub paused: bool,
}

/// A boiler or tank as the overlay shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellInfo {
    pub kind: ShellKind,
    pub size: [i32; 3],
    pub blocks: u32,
    pub water: f64,
    pub water_capacity: f64,
    pub temp: f64,
    pub boiled: f64,
    pub heat: f64,
    pub fireboxes: usize,
    pub timer: f64,
    pub content: Option<Content>,
    pub domes: usize,
    pub damaged: bool,
    pub paused: bool,
}

/// An engine as the overlay shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineInfo {
    pub stall: f64,
    pub top: f64,
    pub demand: f64,
    /// The lowest fill among its cylinders.
    pub fill: f64,
    pub reverse: bool,
    pub cylinders: usize,
    pub paused: bool,
}

impl Works {
    /// The steam network a part belongs to (shell, dome, pipe part, fitting).
    pub fn steam_net_at(&self, pos: Pos) -> Option<usize> {
        if let Some(s) = self.shell_at(pos) {
            return s.pool;
        }
        if self
            .parts
            .get(&pos)
            .is_some_and(|p| p.spec.kind == Kind::Dome)
        {
            return self.shell_at(pos.offset(crate::sim::geom::Dir::Down))?.pool;
        }
        let &ni = self.net_of.get(&pos)?;
        self.nets[ni].pool
    }

    /// What a pressure gauge on `pos` reads.
    pub fn gauge(&self, pos: Pos) -> Gauge {
        match self.steam_net_at(pos) {
            Some(i) => Gauge {
                bar: self.steam[i].pool.pressure(),
                red: self.steam[i].red,
            },
            None => Gauge::default(),
        }
    }

    /// A sight glass on a boiler or tank block: how full (0 to 1) and, on
    /// a boiler, its warning (0 none, 1 low below 25%, 2 very low below 10%
    /// or dry and hot).
    pub fn sight(&self, pos: Pos) -> Option<(f64, u8)> {
        let s = self.shell_at(pos)?;
        if s.content == Some(Content::Steam) {
            return Some((0.0, 0));
        }
        let full = (s.water / s.water_capacity().max(1.0)).clamp(0.0, 1.0);
        let warn = if !s.is_boiler() {
            0
        } else if full < 0.10 || s.temp >= steam::fail::GLOW_TEMP {
            2
        } else if full < 0.25 {
            1
        } else {
            0
        };
        Some((full, warn))
    }

    /// Whether a pipe part burns on touch (on a steam network above 0.2 bar).
    pub fn hot(&self, pos: Pos) -> bool {
        self.parts.get(&pos).is_some_and(|p| p.spec.kind.is_pipe())
            && self
                .steam_net_at(pos)
                .is_some_and(|i| self.steam[i].pool.pressure() > HOT_PIPE_BAR)
    }

    /// The content of the network a part is on, or a vessel's own.
    pub fn content_at(&self, pos: Pos) -> Option<Content> {
        if let Some(s) = self.shell_at(pos) {
            return if s.is_boiler() { None } else { s.content };
        }
        let &ni = self.net_of.get(&pos)?;
        self.nets[ni].content
    }

    pub fn steam_info(&self, pos: Pos) -> Option<SteamInfo> {
        let i = self.steam_net_at(pos)?;
        let n = &self.steam[i];
        Some(SteamInfo {
            bar: n.pool.pressure(),
            red: n.red,
            red_by: n.red_by,
            capacity: n.pool.capacity,
            steam_in: n.steam_in,
            steam_out: n.steam_out,
            paused: self.plants.get(n.plant).is_none_or(|p| p.paused),
        })
    }

    pub fn shell_info(&self, pos: Pos) -> Option<ShellInfo> {
        let s = self.shell_at(pos)?;
        Some(ShellInfo {
            kind: s.kind,
            size: s.cuboid.size,
            blocks: s.n(),
            water: s.water,
            water_capacity: s.water_capacity(),
            temp: s.temp,
            boiled: s.boiled,
            heat: s.heat,
            fireboxes: s.fireboxes,
            timer: s.timer,
            content: s.content,
            domes: s.domes.len(),
            damaged: s.damaged,
            paused: self.plants.get(s.plant).is_none_or(|p| p.paused),
        })
    }

    /// Why a group of shell blocks doesn't form.
    pub fn refusal(&self, pos: Pos) -> Option<&Refusal> {
        self.refused.get(&pos)
    }

    pub fn engine_info(&self, pos: Pos) -> Option<EngineInfo> {
        let &ei = self.engine_of.get(&pos)?;
        let e = &self.engines[ei];
        Some(EngineInfo {
            stall: e.drive.stall,
            top: e.drive.top,
            demand: e.demand.iter().sum(),
            fill: e.fill.iter().copied().fold(1.0, f64::min),
            reverse: self.parts[&e.flywheel].spec.reverse,
            cylinders: e.cylinders.len(),
            paused: self.plants.get(e.plant).is_none_or(|p| p.paused),
        })
    }

    /// A cylinder's stall torque and top speed alone, for its own overlay.
    pub fn cylinder_curve(&self, pos: Pos) -> Option<(f64, f64)> {
        let p = self.cylinder_pressure(pos)?;
        let &ei = self.engine_of.get(&pos)?;
        let e = &self.engines[ei];
        let i = e.cylinders.iter().position(|c| *c == pos)?;
        Some((engine::stall(p, e.fill[i]), engine::top(p)))
    }

    /// A firebox's boilers.
    pub fn firebox_boilers(&self, pos: Pos) -> usize {
        self.heats.get(&pos).map_or(0, |b| b.len())
    }

    /// A pump's flow last step, mB/s.
    pub fn pump_flow(&self, pos: Pos) -> f64 {
        self.pump_flow.get(&pos).copied().unwrap_or(0.0)
    }

    /// An emergency valve's state, or a steam release's.
    pub fn vent_state(&self, pos: Pos) -> VentState {
        self.vents.get(&pos).copied().unwrap_or_default()
    }

    /// What a part's block should save now: its share of its multiblock and
    /// its network's steam (spec 2, 14.3).
    pub fn saved(&self, pos: Pos) -> Option<Saved> {
        let part = self.parts.get(&pos)?;
        let mut out = part.saved;
        if self.dirty {
            return Some(out);
        }
        if let Some(&si) = self.shell_of.get(&pos) {
            let s = &self.shells[si];
            if !s.partial {
                out.water = s.water / s.blocks.len().max(1) as f64;
                out.temp = s.temp;
                out.timer = s.timer;
                out.content = s.content;
                out.steam = s
                    .pool
                    .map_or(0.0, |i| self.steam[i].pool.share(s.block_steam_capacity()));
            }
        }
        Some(out)
    }

    /// A repaired multiblock block restarts cold and empty (spec 2, 12).
    pub fn reset(&mut self, pos: Pos) {
        self.dissolve();
        if let Some(p) = self.parts.get_mut(&pos) {
            let formation = p.saved.formation;
            p.saved = Saved {
                formation,
                ..Saved::default()
            };
        }
        self.dirty = true;
    }

    /// Pipe parts that burn on touch now, grouped by network.
    pub fn hot_pipes(&self) -> Vec<Vec<Pos>> {
        self.steam
            .iter()
            .filter(|n| {
                n.pool.pressure() > HOT_PIPE_BAR
                    && !self.plants.get(n.plant).is_none_or(|p| p.paused)
            })
            .map(|n| {
                n.nets
                    .iter()
                    .flat_map(|&ni| self.nets[ni].pipes.iter().copied())
                    .collect()
            })
            .filter(|v: &Vec<Pos>| !v.is_empty())
            .collect()
    }

    /// Cylinders exhausting now (their engine turning on steam).
    pub fn exhausts(&self) -> Vec<Pos> {
        let mut out = Vec::new();
        for e in &self.engines {
            if self.plants.get(e.plant).is_none_or(|p| p.paused) {
                continue;
            }
            for (i, c) in e.cylinders.iter().enumerate() {
                if e.vented[i] > 0.5 {
                    out.push(*c);
                }
            }
        }
        out
    }

    /// The engine a flywheel or cylinder belongs to: the flywheel and its cylinders.
    pub fn engine_parts(&self, pos: Pos) -> Option<(Pos, Vec<Pos>)> {
        let &ei = self.engine_of.get(&pos)?;
        let e = &self.engines[ei];
        Some((e.flywheel, e.cylinders.clone()))
    }

    /// Every loaded block of a formed multiblock.
    pub fn shell_positions(&self) -> Vec<Pos> {
        let mut out: Vec<Pos> = self.shell_of.keys().copied().collect();
        out.sort();
        out
    }

    /// Pour water into a boiler or water tank (a bucket): true if it had room.
    pub fn pour(&mut self, pos: Pos, mb: f64) -> bool {
        let Some(&si) = self.shell_of.get(&pos) else {
            return false;
        };
        let s = &self.shells[si];
        if !s.works() || s.partial || s.content == Some(Content::Steam) {
            return false;
        }
        if s.water_capacity() - s.water < mb - 1e-9 {
            return false;
        }
        let was_empty = !s.is_boiler() && s.content.is_none();
        if was_empty {
            self.shells[si].content = Some(Content::Water);
        }
        self.fill(si, mb);
        if was_empty {
            // Its networks take its content now.
            self.dissolve();
            self.dirty = true;
        }
        true
    }

    /// Take water from a water tank (an empty bucket): true if it held enough.
    pub fn draw_water(&mut self, pos: Pos, mb: f64) -> bool {
        let Some(&si) = self.shell_of.get(&pos) else {
            return false;
        };
        let s = &mut self.shells[si];
        if s.is_boiler()
            || !s.works()
            || s.partial
            || s.content != Some(Content::Water)
            || s.water < mb - 1e-9
        {
            return false;
        }
        s.water -= mb;
        true
    }

    /// The flow through a pipe part last step and its throughput now, mB/s.
    pub fn flow_at(&self, pos: Pos) -> Option<(f64, f64)> {
        let &ni = self.net_of.get(&pos)?;
        let net = &self.nets[ni];
        let &node = net.node_of.get(&pos)?;
        let part = self.parts.get(&pos)?;
        Some((
            net.flow.get(node).copied().unwrap_or(0.0),
            part.spec.throughput_now(),
        ))
    }

    /// Whether a part's plant is waiting on part of it outside the loaded area.
    pub fn partial(&self, pos: Pos) -> bool {
        self.plant_of(pos)
            .and_then(|p| self.plants.get(p))
            .is_some_and(|p| p.partial)
    }

    // ---- 0.3: regulation, cooling, rust ---------------------------------------

    /// A governor as the overlay shows it.
    pub fn governor_info(&self, pos: Pos) -> Option<GovernorInfo> {
        let part = self.parts.get(&pos)?;
        if part.spec.kind != Kind::Governor {
            return None;
        }
        let st = self.govs.get(&pos).cloned().unwrap_or_default();
        let (engines, behind) = match self.net_of.get(&pos) {
            Some(&ni) => {
                let net = &self.nets[ni];
                let mut engines = BTreeSet::new();
                let mut behind = false;
                for (e, at) in &net.consumers {
                    let under = at
                        .iter()
                        .any(|&u| net.govs.get(u).is_some_and(|g| g.contains(&pos)));
                    if !under {
                        continue;
                    }
                    match e {
                        End::Cylinder(q) => {
                            if let Some(&ei) = self.engine_of.get(q) {
                                engines.insert(ei);
                            }
                        }
                        End::Vent(_) => behind = true,
                        _ => {}
                    }
                }
                (engines.len(), behind)
            }
            None => (0, false),
        };
        Some(GovernorInfo {
            target: part.spec.target,
            sens: part.spec.sens,
            speed: st.speed,
            ball: part.saved.ball,
            throttle: if self.govs.contains_key(&pos) {
                st.g
            } else {
                1.0
            },
            swing: st.history.swing(),
            engines,
            vent_behind: behind,
            on_steam: self
                .net_of
                .get(&pos)
                .is_some_and(|&ni| self.nets[ni].content == Some(Content::Steam)),
        })
    }

    /// A governor's look: ball step (0 to 7) and lever (0 to 3).
    pub fn governor_look(&self, pos: Pos) -> (u8, u8) {
        let ball = self.parts.get(&pos).map_or(0.0, |p| p.saved.ball);
        let g = self.govs.get(&pos).map_or(1.0, |s| s.g);
        (
            crate::sim::steam::govern::Balls { x: ball, v: 0.0 }.step_shown(),
            crate::sim::steam::govern::lever(g),
        )
    }

    /// A damper: its opening (0 to 1), set pressure, boiler pressure and
    /// whether it is attached to a formed boiler.
    pub fn damper_info(&self, pos: Pos) -> Option<DamperInfo> {
        let part = self.parts.get(&pos)?;
        if part.spec.kind != Kind::Damper {
            return None;
        }
        let shell = self.shells.iter().find(|s| s.dampers.contains(&pos));
        Some(DamperInfo {
            opening: part.saved.opening,
            set: part.spec.set,
            powered: part.spec.powered,
            stuck: part.spec.leaking(),
            bar: shell
                .and_then(|s| s.pool)
                .map(|i| self.steam[i].pool.pressure()),
            red: shell.map(|s| s.kind.rating()),
            boiler_opening: shell.map(|s| s.opening),
        })
    }

    /// A firebox's heat last step (None when it never burned in a step).
    pub fn fire(&self, pos: Pos) -> Option<FireState> {
        self.fires.get(&pos).copied()
    }

    /// A firebox's burn-rate factor from its dampers: 1 with none, 0 when
    /// they are all shut.
    pub fn burn_factor(&self, pos: Pos) -> f64 {
        self.fires.get(&pos).map_or(1.0, |f| f.damper)
    }

    /// The firebox a bellows blows, and whether it counts (the first by
    /// position).
    pub fn bellows_target(&self, pos: Pos) -> Option<Pos> {
        self.bellows_of
            .iter()
            .find(|(_, b)| **b == pos)
            .map(|(f, _)| *f)
    }

    /// Each bellows' load now (a load only while it blows a burning fire).
    pub fn bellows_loads(&self) -> Vec<(Pos, f64)> {
        self.bellows_loads.iter().map(|(q, l)| (*q, *l)).collect()
    }

    /// A heat exchanger last step.
    pub fn exchanger_info(&self, pos: Pos) -> Option<ExState> {
        self.exchangers.get(&pos).copied()
    }

    /// A cooled machine last step.
    pub fn cooling_info(&self, pos: Pos) -> Option<CoolState> {
        self.cooling.get(&pos).copied()
    }

    /// Whether a grinder is cooled now.
    pub fn cooled(&self, pos: Pos) -> bool {
        self.cooling.get(&pos).is_some_and(|c| c.cooled)
    }

    /// What a drain dumps now.
    pub fn drain_flow(&self, pos: Pos) -> Option<(Content, f64)> {
        self.drains.get(&pos).copied()
    }

    /// Drains dumping now.
    pub fn drains(&self) -> Vec<(Pos, Content, f64)> {
        self.drains
            .iter()
            .filter(|(_, (_, f))| *f > 1e-6)
            .map(|(q, (c, f))| (*q, *c, *f))
            .collect()
    }

    /// Leaking parts losing something now.
    pub fn leaks(&self) -> Vec<(Pos, Content, f64)> {
        self.leaks
            .iter()
            .filter(|(_, (_, f))| *f > 1e-6)
            .map(|(q, (c, f))| (*q, *c, *f))
            .collect()
    }

    /// The leak at one part now.
    pub fn leak_at(&self, pos: Pos) -> Option<(Content, f64)> {
        self.leaks.get(&pos).copied()
    }

    /// The leaks on the network a part is on, and their total (mB/s).
    pub fn net_leaks(&self, pos: Pos) -> (usize, f64) {
        let mut members: BTreeSet<Pos> = BTreeSet::new();
        if let Some(i) = self.steam_net_at(pos) {
            let n = &self.steam[i];
            for &ni in &n.nets {
                members.extend(self.nets[ni].pipes.iter().copied());
                members.extend(self.nets[ni].consumers.iter().filter_map(|(e, _)| e.pos()));
            }
            for &si in &n.shells {
                members.extend(self.shells[si].blocks.iter().copied());
            }
        } else if let Some(&ni) = self.net_of.get(&pos) {
            members.extend(self.nets[ni].pipes.iter().copied());
        } else if let Some(s) = self.shell_at(pos) {
            members.extend(s.blocks.iter().copied());
        }
        let mut n = 0;
        let mut total = 0.0;
        for q in members {
            if let Some((_, f)) = self.leaks.get(&q)
                && *f > 1e-6
            {
                n += 1;
                total += f;
            }
        }
        (n, total)
    }

    /// Exhaust each cylinder vents now (mB/s), and what it pipes away.
    pub fn exhaust_at(&self, pos: Pos) -> Option<(f64, f64)> {
        let &ei = self.engine_of.get(&pos)?;
        let e = &self.engines[ei];
        let i = e.cylinders.iter().position(|c| *c == pos)?;
        Some((e.vented[i], (e.drawn[i] - e.vented[i]).max(0.0)))
    }

    /// The governors' throttle on an engine (the lowest over its cylinders).
    pub fn engine_throttle(&self, pos: Pos) -> f64 {
        let Some(&ei) = self.engine_of.get(&pos) else {
            return 1.0;
        };
        self.engines[ei]
            .throttle
            .iter()
            .copied()
            .fold(1.0, f64::min)
    }

    /// The governor parts, for the game's per-tick ball step.
    pub fn governors(&self) -> Vec<Pos> {
        self.parts_of(Kind::Governor)
    }

    /// Every part of one kind, by position.
    pub fn parts_of(&self, kind: Kind) -> Vec<Pos> {
        let mut out: Vec<Pos> = self
            .parts
            .iter()
            .filter(|(_, p)| p.spec.kind == kind)
            .map(|(q, _)| *q)
            .collect();
        out.sort();
        out
    }
}

/// A governor as the overlay and its screen show it.
#[derive(Clone, Debug, PartialEq)]
pub struct GovernorInfo {
    pub target: f64,
    pub sens: u8,
    pub speed: f64,
    pub ball: f64,
    pub throttle: f64,
    /// The speed's swing over the last 10 s (hunting when wide).
    pub swing: f64,
    /// The engines it throttles.
    pub engines: usize,
    /// A vent on its downstream side.
    pub vent_behind: bool,
    pub on_steam: bool,
}

/// A furnace damper as the overlay shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct DamperInfo {
    pub opening: f64,
    pub set: f64,
    pub powered: bool,
    pub stuck: bool,
    pub bar: Option<f64>,
    pub red: Option<f64>,
    pub boiler_opening: Option<f64>,
}
