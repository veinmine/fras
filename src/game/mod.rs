//! The game layer: FRAS on Horizon. Kept thin; the rules live in `sim`.
//!
//! No mixins: everything here goes through the mod API's hooks and parts.

pub mod blocks;
pub mod draw;
pub mod effects;
pub mod firebox;
pub mod governor;
pub mod grinder;
pub mod parts;
pub mod pipes;
pub mod rust;
pub mod server;
pub mod shell_model;
pub mod steam;
pub mod steam_draw;
pub mod steam_inspect;
pub mod tools;
pub mod world;

use std::sync::OnceLock;

use mod_api::{BlockFace, BlockKey, Direction, ItemKey, ParticleKey};

use crate::sim::geom::Dir;

/// Keys handed out at registration, for the hooks.
pub struct Keys {
    pub wrench: ItemKey,
    pub leather_belt: ItemKey,
    pub bearing: ItemKey,
    pub wooden_shaft: ItemKey,
    pub iron_shaft: ItemKey,
    pub bearing_block: BlockKey,
    pub stick: ItemKey,
    pub iron_nugget: ItemKey,
    pub flint: ItemKey,
    pub water_bucket: ItemKey,
    pub bucket: ItemKey,
    pub copper_ingot: ItemKey,
    pub copper_nugget: ItemKey,
    pub iron_plate: ItemKey,
    pub leather: ItemKey,
    pub paint: ItemKey,
    /// The pre-painted items (rust spec 7.2).
    pub painted: Vec<ItemKey>,
    pub rust_flakes: ItemKey,
    pub smoke: ParticleKey,
    pub crit: ParticleKey,
    pub cloud: ParticleKey,
    pub ember: ParticleKey,
    pub wisp: ParticleKey,
    pub drip: ParticleKey,
}

pub static KEYS: OnceLock<Keys> = OnceLock::new();

pub fn keys() -> Option<&'static Keys> {
    KEYS.get()
}

pub fn direction(d: Dir) -> Direction {
    match d {
        Dir::Down => Direction::Down,
        Dir::Up => Direction::Up,
        Dir::North => Direction::North,
        Dir::South => Direction::South,
        Dir::West => Direction::West,
        Dir::East => Direction::East,
    }
}

pub fn dir(d: Direction) -> Dir {
    match d {
        Direction::Down => Dir::Down,
        Direction::Up => Dir::Up,
        Direction::North => Dir::North,
        Direction::South => Dir::South,
        Direction::West => Dir::West,
        Direction::East => Dir::East,
    }
}

pub fn face(d: Dir) -> BlockFace {
    match d {
        Dir::Down => BlockFace::Down,
        Dir::Up => BlockFace::Up,
        Dir::North => BlockFace::North,
        Dir::South => BlockFace::South,
        Dir::West => BlockFace::West,
        Dir::East => BlockFace::East,
    }
}

/// The face a unit vector (as hooks give clicked faces) points through.
pub fn dir_of_vec(v: [i32; 3]) -> Option<Dir> {
    Dir::ALL.into_iter().find(|d| d.step() == v)
}

/// The horizontal direction a yaw (degrees, vanilla's) looks toward.
pub fn yaw_dir(yaw: f32) -> Dir {
    let i = (((yaw as f64 / 90.0) + 0.5).floor() as i64).rem_euclid(4);
    // Vanilla: 0 south, 1 west, 2 north, 3 east.
    [Dir::South, Dir::West, Dir::North, Dir::East][i as usize]
}

/// Format a number for players: at most one decimal.
pub fn num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_to_direction() {
        assert_eq!(yaw_dir(0.0), Dir::South);
        assert_eq!(yaw_dir(90.0), Dir::West);
        assert_eq!(yaw_dir(-90.0), Dir::East);
        assert_eq!(yaw_dir(180.0), Dir::North);
        assert_eq!(yaw_dir(-180.0), Dir::North);
    }

    #[test]
    fn numbers_read_well() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(12.04), "12");
        assert_eq!(num(-3.25), "-3.3");
        assert_eq!(num(0.55), "0.6");
    }

    fn lang(code: &str) -> mod_api::serde_json::Map<String, mod_api::serde_json::Value> {
        let path = format!(
            "{}/assets/fras/lang/{code}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(path).unwrap();
        mod_api::serde_json::from_str::<mod_api::serde_json::Value>(&text)
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn polish_has_every_english_key() {
        let en = lang("en_us");
        let pl = lang("pl_pl");
        let missing: Vec<&String> = en.keys().filter(|k| !pl.contains_key(*k)).collect();
        assert!(missing.is_empty(), "pl_pl lacks {missing:?}");
        let extra: Vec<&String> = pl.keys().filter(|k| !en.contains_key(*k)).collect();
        assert!(extra.is_empty(), "pl_pl has keys English hasn't: {extra:?}");
    }

    #[test]
    fn every_text_the_code_uses_is_translated() {
        let en = lang("en_us");
        let mut keys: Vec<String> = Vec::new();
        for b in blocks::Block::ALL {
            keys.push(format!("block.fras.{}", b.name()));
            keys.push(format!("tooltip.fras.{}", b.name()));
        }
        for i in crate::ITEMS {
            keys.push(format!("item.fras.{i}"));
            keys.push(format!("tooltip.fras.{i}"));
        }
        for j in [
            crate::sim::rot::Jam::Overspeed,
            crate::sim::rot::Jam::Shear,
            crate::sim::rot::Jam::Shock,
            crate::sim::rot::Jam::Conflict,
            crate::sim::rot::Jam::Blocked,
        ] {
            keys.push(format!("fras.jam.{}", j.name()));
        }
        for code in 0..8 {
            keys.push(format!(
                "fras.grinder.status.{}",
                crate::sim::grind::Status::from_code(code).key()
            ));
        }
        // Literal keys in the sources.
        for m in steam::MODES
            .iter()
            .chain(rust::ON_CHOICES.iter())
            .chain(rust::SPEED_CHOICES.iter())
        {
            keys.push(m.to_string());
        }
        for i in crate::PAINTED {
            keys.push(format!("item.fras.painted_{i}"));
        }
        for c in ["steam", "waterlogged", "water", "rain"] {
            keys.push(format!("fras.rust.cause.{c}"));
        }
        for st in ["clean", "spotty", "rusty", "leaking", "painted"] {
            keys.push(format!("fras.rust.stage.{st}"));
        }
        keys.push("fras.jam.rusted".into());
        keys.push("tooltip.fras.painted".into());
        for c in ["water", "steam", "hot_water", "exhaust", "empty"] {
            keys.push(format!("fras.content.{c}"));
        }
        for s in ["rust", "rust_speed"] {
            keys.push(format!("fras.settings.{s}"));
            keys.push(format!("fras.settings.{s}.tooltip"));
        }
        for file in [
            "tools.rs",
            "parts.rs",
            "grinder.rs",
            "steam.rs",
            "steam_inspect.rs",
            "pipes.rs",
            "firebox.rs",
            "governor.rs",
            "rust.rs",
        ] {
            let src =
                std::fs::read_to_string(format!("{}/src/game/{file}", env!("CARGO_MANIFEST_DIR")))
                    .unwrap();
            for part in src.split('"').skip(1).step_by(2) {
                if (part.starts_with("fras.") || part.starts_with("tooltip.fras."))
                    && !part.ends_with('.')
                    && !part.contains('{')
                {
                    keys.push(part.to_string());
                }
            }
        }
        let missing: Vec<&String> = keys.iter().filter(|k| !en.contains_key(*k)).collect();
        assert!(missing.is_empty(), "en_us lacks {missing:?}");
    }
}
