//! The rotational network, free of the game (rotational network spec).
//!
//! - `part`: what each part is, how it connects and its numbers;
//! - `solve`: integrating a system's speed;
//! - `flow`: torque flow through the joints;
//! - `damage`: warning timers and jams;
//! - `net`: the registry, building networks, the tick, belts and pausing.

pub mod damage;
pub mod flow;
pub mod net;
pub mod part;
pub mod solve;

#[cfg(test)]
mod tests;

pub use damage::Jam;
pub use net::{Drive, Event, Mechanics, PartState, Reading, Saved, View};
pub use part::{BeltEnd, Kind, Material, Shape, Spec};
