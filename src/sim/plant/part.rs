//! What each steam, fluid and multiblock part is, as its block tells the plant.

use crate::sim::fluid::{self, Content};
use crate::sim::geom::{Axis, Dir};
use crate::sim::multi::ShellKind;
use crate::sim::multi::shape::Cuboid;
use crate::sim::steam::{AMBIENT, fail};

/// Pipe materials.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PipeMat {
    Copper,
    Iron,
}

/// What a part is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A boiler block (copper or iron) or a storage tank block.
    Shell(ShellKind),
    Dome,
    Pipe(PipeMat),
    /// Manual valve, or redstone valve (open while powered).
    Valve {
        redstone: bool,
    },
    /// Steam release, or emergency valve.
    Vent {
        emergency: bool,
    },
    Cylinder,
    Firebox,
    Pump,
    Flywheel,
}

impl Kind {
    /// Parts of a pipe network that carry flow.
    pub fn is_pipe(self) -> bool {
        matches!(self, Kind::Pipe(_) | Kind::Valve { .. })
    }

    /// The throughput of a pipe part fully open, mB/s.
    pub fn throughput(self) -> f64 {
        match self {
            Kind::Pipe(PipeMat::Copper) => fluid::COPPER_PIPE,
            Kind::Pipe(PipeMat::Iron) => fluid::IRON_PIPE,
            Kind::Valve { .. } | Kind::Vent { .. } => fluid::VALVE,
            Kind::Dome => fluid::DOME,
            _ => 0.0,
        }
    }

    /// Rated pressure on a steam network, bar (None for parts that have none).
    pub fn rating(self) -> Option<f64> {
        Some(match self {
            Kind::Shell(k) => k.rating(),
            Kind::Pipe(PipeMat::Copper) => fail::COPPER_PIPE,
            Kind::Pipe(PipeMat::Iron) => fail::IRON_PIPE,
            Kind::Valve { .. } | Kind::Vent { .. } => fail::VALVE,
            Kind::Cylinder => fail::CYLINDER,
            _ => return None,
        })
    }
}

/// How a part sits and how it is set: what its block decides.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spec {
    pub kind: Kind,
    /// Pipes: the connected faces, one bit per `Dir::index`.
    pub faces: u8,
    /// Valves and the flywheel: along this axis (`dir.axis()`). Vents: their
    /// pipe face. Cylinder: its facing (toward the hub). Pump: its shaft socket.
    pub dir: Dir,
    /// Manual valve 0 to 1; redstone valve and steam release 0 or 1.
    pub opening: f64,
    /// Emergency valve: its set pressure, bar.
    pub set: f64,
    /// Burst (pipes, valves), jammed (cylinder) or damaged (shell blocks).
    pub damaged: bool,
    /// Flywheel: the reversing gear.
    pub reverse: bool,
}

impl Spec {
    pub fn new(kind: Kind) -> Spec {
        Spec {
            kind,
            faces: 0,
            dir: Dir::North,
            opening: 1.0,
            set: crate::sim::fluid::valve::DEFAULT_SET,
            damaged: false,
            reverse: false,
        }
    }

    pub fn with_dir(mut self, dir: Dir) -> Spec {
        self.dir = dir;
        self
    }

    pub fn with_faces(mut self, faces: &[Dir]) -> Spec {
        self.faces = faces.iter().fold(0, |m, d| m | (1 << d.index()));
        self
    }

    pub fn face(&self, d: Dir) -> bool {
        self.faces & (1 << d.index()) != 0
    }

    pub fn axis(&self) -> Axis {
        self.dir.axis()
    }

    /// Whether a pipe part lets flow through: a closed valve is a boundary.
    pub fn open(&self) -> bool {
        match self.kind {
            Kind::Valve { .. } => self.opening > 1e-9,
            _ => true,
        }
    }

    /// Its throughput now (a part-open valve throttles).
    pub fn throughput_now(&self) -> f64 {
        match self.kind {
            Kind::Valve { .. } => self.kind.throughput() * self.opening.clamp(0.0, 1.0),
            k => k.throughput(),
        }
    }

    /// Whether a change from `self` to `other` needs the networks rebuilt
    /// (spec 3, 3.3): not a valve's opening between two open positions, an
    /// emergency valve's set pressure or a steam release's toggle.
    pub fn needs_rebuild(&self, other: &Spec) -> bool {
        let mut a = *self;
        let mut b = *other;
        if a.kind == b.kind && a.open() == b.open() {
            a.opening = 0.0;
            b.opening = 0.0;
        }
        a.set = 0.0;
        b.set = 0.0;
        a.reverse = false;
        b.reverse = false;
        if matches!(a.kind, Kind::Vent { .. }) {
            a.opening = 0.0;
            b.opening = 0.0;
        }
        a != b
    }
}

/// What a part saved with its block: its share of a multiblock, its steam.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Saved {
    /// Shell blocks: water (boilers, water tanks), mB.
    pub water: f64,
    /// Boiler blocks: °C.
    pub temp: f64,
    /// Steam, mB (shell blocks and, in memory only, pipe parts).
    pub steam: f64,
    /// The multiblock's warning timer.
    pub timer: f64,
    /// Tank blocks: what the tank holds.
    pub content: Option<Content>,
    /// The multiblock it was part of when saved.
    pub formation: Option<Cuboid>,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            water: 0.0,
            temp: AMBIENT,
            steam: 0.0,
            timer: 0.0,
            content: None,
            formation: None,
        }
    }
}
