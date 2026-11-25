//! The steam engine (multiblocks spec 4): a flywheel and one or two
//! cylinders two blocks beside its hub in its plane, facing it.

use crate::sim::geom::{Axis, Dir, Pos};

/// The horizontal direction across a flywheel's (horizontal) axis.
pub fn across(axis: Axis) -> Axis {
    match axis {
        Axis::X => Axis::Z,
        _ => Axis::X,
    }
}

/// The two cylinder positions of a flywheel at `hub` turning about `axis`,
/// with the facing a cylinder there needs (toward the hub).
pub fn cylinder_spots(hub: Pos, axis: Axis) -> [(Pos, Dir); 2] {
    let a = across(axis);
    let (pos, neg) = (a.positive(), a.negative());
    [(hub.offset_by(pos, 2), neg), (hub.offset_by(neg, 2), pos)]
}

/// The flywheel a cylinder at `pos` facing `facing` would drive: the hub two
/// blocks ahead, if the flywheel there has an axis across the facing.
pub fn hub_of(pos: Pos, facing: Dir) -> Option<Pos> {
    (facing.axis() != Axis::Y).then(|| pos.offset_by(facing, 2))
}

/// The 8 positions round a flywheel's hub in its plane (they must be clear).
pub fn rim(hub: Pos, axis: Axis) -> Vec<Pos> {
    let others: Vec<Axis> = Axis::ALL.into_iter().filter(|a| *a != axis).collect();
    let (u, v) = (others[0].unit(), others[1].unit());
    let mut out = Vec::new();
    for du in -1..=1 {
        for dv in -1..=1 {
            if du == 0 && dv == 0 {
                continue;
            }
            out.push(Pos::new(
                hub.x + du * u[0] + dv * v[0],
                hub.y + du * u[1] + dv * v[1],
                hub.z + du * u[2] + dv * v[2],
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylinders_sit_two_blocks_beside_the_hub_facing_it() {
        let hub = Pos::new(0, 64, 0);
        let spots = cylinder_spots(hub, Axis::X);
        assert_eq!(spots[0], (Pos::new(0, 64, 2), Dir::North));
        assert_eq!(spots[1], (Pos::new(0, 64, -2), Dir::South));
        for (p, f) in spots {
            assert_eq!(hub_of(p, f), Some(hub));
        }
        let z = cylinder_spots(hub, Axis::Z);
        assert_eq!(z[0], (Pos::new(2, 64, 0), Dir::West));
        assert_eq!(rim(hub, Axis::Z).len(), 8);
        assert!(rim(hub, Axis::Z).iter().all(|p| p.z == 0));
    }
}
