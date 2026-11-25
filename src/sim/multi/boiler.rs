//! Boiler specifics (multiblocks spec 3): where fireboxes heat it, and its
//! water and temperature when it splits or merges.

use std::collections::BTreeSet;

use super::shape::Cuboid;
use crate::sim::geom::{Dir, Pos};
use crate::sim::steam::{SHELL_KJ, WARM_KJ};

/// The positions a firebox heats this boiler from: directly below a block
/// of its bottom layer, or beside one (spec 4, 3.3).
pub fn firebox_spots(c: &Cuboid) -> BTreeSet<Pos> {
    let mut out = BTreeSet::new();
    for p in c.bottom() {
        out.insert(p.offset(Dir::Down));
        for d in Dir::HORIZONTAL {
            let q = p.offset(d);
            if !c.contains(q) {
                out.insert(q);
            }
        }
    }
    out
}

/// The positions a dome can sit on: above its top layer.
pub fn dome_spots(c: &Cuboid) -> Vec<Pos> {
    c.top().into_iter().map(|p| p.offset(Dir::Up)).collect()
}

/// One block's share of a boiler: its water and the temperature.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Share {
    pub water: f64,
    pub temp: f64,
}

/// Blocks forming a boiler: it sums their water and takes their
/// heat-capacity-weighted temperature (spec 4, 3.5).
pub fn merge(shares: &[Share]) -> Share {
    let water: f64 = shares.iter().map(|s| s.water).sum();
    let cap: f64 = shares.iter().map(|s| WARM_KJ * s.water + SHELL_KJ).sum();
    let heat: f64 = shares
        .iter()
        .map(|s| (WARM_KJ * s.water + SHELL_KJ) * s.temp)
        .sum();
    Share {
        water,
        temp: if cap > 0.0 {
            heat / cap
        } else {
            crate::sim::steam::AMBIENT
        },
    }
}

/// A boiler's water and temperature split evenly over its `n` blocks.
pub fn split(whole: Share, n: usize) -> Share {
    Share {
        water: whole.water / n.max(1) as f64,
        temp: whole.temp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fireboxes_heat_from_below_and_beside_the_bottom_layer_only() {
        // A 1×4×1 tall boiler: 1 below, 4 beside.
        let tall = Cuboid {
            min: Pos::new(0, 10, 0),
            size: [1, 4, 1],
        };
        let spots = firebox_spots(&tall);
        assert_eq!(spots.len(), 5);
        assert!(spots.contains(&Pos::new(0, 9, 0)));
        assert!(spots.contains(&Pos::new(1, 10, 0)));
        assert!(!spots.contains(&Pos::new(1, 11, 0)));
        assert!(!spots.contains(&Pos::new(0, 14, 0)));
        // A 2×1×4 lying boiler: 8 below and 12 around: 20.
        let lying = Cuboid {
            min: Pos::new(0, 10, 0),
            size: [2, 1, 4],
        };
        assert_eq!(firebox_spots(&lying).len(), 20);
        assert_eq!(dome_spots(&lying).len(), 8);
    }

    #[test]
    fn water_and_temperature_on_merge_and_split() {
        let hot = Share {
            water: 1000.0,
            temp: 100.0,
        };
        let cold_empty = Share {
            water: 0.0,
            temp: 20.0,
        };
        let m = merge(&[hot, hot, hot, cold_empty]);
        assert_eq!(m.water, 3000.0);
        // A new, empty block cools a hot boiler a little.
        assert!(m.temp < 100.0 && m.temp > 95.0, "{}", m.temp);
        let s = split(m, 4);
        assert_eq!(s.water, 750.0);
        assert_eq!(merge(&[s, s, s, s]).water, m.water);
        assert!((merge(&[s, s, s, s]).temp - m.temp).abs() < 1e-9);
    }
}
