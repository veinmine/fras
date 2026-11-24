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
}
