//! Pipes and fluids, free of the game (fluids and pipes spec, spec 3).
//!
//! - `route`: the widest-path tree of a pipe network and the per-step scaling
//!   of demands to the throughputs on the way (section 5);
//! - `pump`: a pump's flow from its speed and water sources (7.2);
//! - `valve`: openings and the emergency valve's set pressure and reseat (6).
//!
//! Which parts form a network and what it carries is worked out with the
//! other subsystems in `crate::sim::plant`.

pub mod cool;
pub mod exchange;
pub mod pump;
pub mod route;
pub mod valve;

/// What a pipe network carries.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Content {
    Water,
    Steam,
    /// Hot water at 90 °C (cooling spec 7).
    HotWater,
    /// Exhaust steam: flow only, cylinder tops to heat exchangers (cooling spec 4).
    Exhaust,
}

impl Content {
    pub fn name(self) -> &'static str {
        match self {
            Content::Water => "water",
            Content::Steam => "steam",
            Content::HotWater => "hot_water",
            Content::Exhaust => "exhaust",
        }
    }

    pub fn from_name(name: &str) -> Option<Content> {
        match name {
            "water" => Some(Content::Water),
            "steam" => Some(Content::Steam),
            "hot_water" => Some(Content::HotWater),
            "exhaust" => Some(Content::Exhaust),
            _ => None,
        }
    }
}

/// Throughputs, mB/s *(tune)*.
pub const COPPER_PIPE: f64 = 80.0;
pub const IRON_PIPE: f64 = 200.0;
pub const VALVE: f64 = 200.0;
pub const DOME: f64 = 100.0;
/// A drain dumps at most this, mB/s.
pub const DRAIN: f64 = 200.0;

impl Content {
    /// Water or hot water: what boilers, tanks and drains take.
    pub fn is_liquid(self) -> bool {
        matches!(self, Content::Water | Content::HotWater)
    }

    /// Flow-only contents, routed like water (no pool, no pressure).
    pub fn is_flow(self) -> bool {
        self != Content::Steam
    }
}
