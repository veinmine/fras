//! Multiblocks, free of the game (multiblocks spec, spec 4): which blocks form
//! one boiler or tank, where fireboxes and domes go, where an engine's
//! cylinders go.
//!
//! - `shape`: same-kind groups (a capped flood fill), the cuboid rule, the long axis;
//! - `boiler`: firebox positions, water and temperature on split and merge;
//! - `engine`: a flywheel's plane and its cylinder positions.

pub mod boiler;
pub mod engine;
pub mod shape;

/// The kinds of block that form cuboid multiblocks. Different kinds never
/// join, and may touch (spec 4, 3.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShellKind {
    CopperBoiler,
    IronBoiler,
    Tank,
}

impl ShellKind {
    pub const ALL: [ShellKind; 3] = [
        ShellKind::CopperBoiler,
        ShellKind::IronBoiler,
        ShellKind::Tank,
    ];

    /// The most blocks one multiblock of this kind holds.
    pub fn limit(self) -> usize {
        match self {
            ShellKind::CopperBoiler => 4,
            ShellKind::IronBoiler => 8,
            ShellKind::Tank => 27,
        }
    }

    pub fn is_boiler(self) -> bool {
        !matches!(self, ShellKind::Tank)
    }

    /// Rated pressure, bar.
    pub fn rating(self) -> f64 {
        use crate::sim::steam::fail;
        match self {
            ShellKind::CopperBoiler => fail::COPPER_BOILER,
            ShellKind::IronBoiler => fail::IRON_BOILER,
            ShellKind::Tank => fail::STEAM_TANK,
        }
    }

    /// The shell temperature a dry boiler ruptures at.
    pub fn dry_limit(self) -> f64 {
        use crate::sim::steam::fail;
        match self {
            ShellKind::CopperBoiler => fail::COPPER_DRY,
            _ => fail::IRON_DRY,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ShellKind::CopperBoiler => "copper_boiler",
            ShellKind::IronBoiler => "iron_boiler",
            ShellKind::Tank => "storage_tank",
        }
    }

    pub fn from_name(name: &str) -> Option<ShellKind> {
        ShellKind::ALL.into_iter().find(|k| k.name() == name)
    }
}
