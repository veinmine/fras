//! Steam and heat, free of the game (steam and heat spec, spec 2).
//!
//! - `boiler`: a boiler's water and temperature, the step of 4.1, dry firing;
//! - `pool`: a steam network's steam, capacity and pressure, joining and splitting;
//! - `engine`: the cylinder's curve, steam demand and fill (9.1, 9.2);
//! - `fail`: rated pressures, warning timers, rupture power, the steam cloud (6, 7);
//! - `fuel`: the firebox's heat-rate table (3.1).
//!
//! The numbers marked *(tune)* in the spec are collected here.

pub mod boiler;
pub mod engine;
pub mod fail;
pub mod fuel;
pub mod pool;

/// Ticks between two steam steps.
pub const STEP_TICKS: u64 = 4;
/// A steam step, in seconds.
pub const DT: f64 = STEP_TICKS as f64 / 20.0;

/// Heat to boil 1 mB of water, kJ.
pub const BOIL_KJ: f64 = 25.0;
/// Heat to warm 1 mB of water by 1 °C, kJ.
pub const WARM_KJ: f64 = 0.1;
/// Heat to warm one boiler block's shell by 1 °C, kJ.
pub const SHELL_KJ: f64 = 10.0;
/// The water's boiling point, whatever the pressure (spec 2, S2).
pub const BOILING: f64 = 100.0;
/// Where a boiler starts, and the temperature of feed water.
pub const AMBIENT: f64 = 20.0;

/// One boiler block's water, mB.
pub const BOILER_WATER: f64 = 1000.0;
/// Steam capacity, mB per bar: a boiler block, a steam tank block, a pipe part.
pub const BOILER_STEAM: f64 = 100.0;
pub const TANK_STEAM: f64 = 400.0;
pub const PIPE_STEAM: f64 = 5.0;
/// One tank block's water, mB.
pub const TANK_WATER: f64 = 4000.0;

/// The most an open emergency valve or steam release vents, mB/s.
pub const VENT_RATE: f64 = 80.0;
/// A steam pipe burns on touch above this pressure, bar.
pub const HOT_PIPE_BAR: f64 = 0.2;
