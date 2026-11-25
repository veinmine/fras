//! Groups and the cuboid rule (multiblocks spec 3.1, 5 and 9).

use std::collections::{BTreeSet, VecDeque};

use crate::sim::geom::{Axis, Dir, Pos};

/// A same-kind group of blocks found from one block.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Its blocks, sorted (all of them unless `too_big`).
    pub blocks: Vec<Pos>,
    /// The search passed the limit and stopped.
    pub too_big: bool,
}

/// Flood-fill the face-connected blocks for which `member` holds, from
/// `start`, stopping once more than `limit` are found.
pub fn group(start: Pos, member: &dyn Fn(Pos) -> bool, limit: usize) -> Group {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    if !member(start) {
        return Group {
            blocks: Vec::new(),
            too_big: false,
        };
    }
    seen.insert(start);
    queue.push_back(start);
    while let Some(p) = queue.pop_front() {
        for d in Dir::ALL {
            let q = p.offset(d);
            if !seen.contains(&q) && member(q) {
                seen.insert(q);
                if seen.len() > limit {
                    return Group {
                        blocks: seen.into_iter().collect(),
                        too_big: true,
                    };
                }
                queue.push_back(q);
            }
        }
    }
    Group {
        blocks: seen.into_iter().collect(),
        too_big: false,
    }
}

/// A box of blocks: its lowest corner and its size.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cuboid {
    pub min: Pos,
    pub size: [i32; 3],
}

impl Cuboid {
    pub fn volume(&self) -> usize {
        (self.size[0] * self.size[1] * self.size[2]) as usize
    }

    pub fn max(&self) -> Pos {
        Pos::new(
            self.min.x + self.size[0] - 1,
            self.min.y + self.size[1] - 1,
            self.min.z + self.size[2] - 1,
        )
    }

    pub fn contains(&self, p: Pos) -> bool {
        let m = self.max();
        (self.min.x..=m.x).contains(&p.x)
            && (self.min.y..=m.y).contains(&p.y)
            && (self.min.z..=m.z).contains(&p.z)
    }

    /// Every position in it, sorted.
    pub fn positions(&self) -> Vec<Pos> {
        let mut out = Vec::with_capacity(self.volume());
        let m = self.max();
        for x in self.min.x..=m.x {
            for y in self.min.y..=m.y {
                for z in self.min.z..=m.z {
                    out.push(Pos::new(x, y, z));
                }
            }
        }
        out
    }

    /// The long axis: the longest dimension; ties go to vertical, then X (spec 4, 6.1).
    pub fn long_axis(&self) -> Axis {
        let [x, y, z] = self.size;
        if y >= x && y >= z {
            Axis::Y
        } else if x >= z {
            Axis::X
        } else {
            Axis::Z
        }
    }

    /// The bounding box of some positions.
    pub fn bounding(blocks: &[Pos]) -> Option<Cuboid> {
        let first = blocks.first()?;
        let (mut lo, mut hi) = (*first, *first);
        for p in blocks {
            lo = Pos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Pos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        Some(Cuboid {
            min: lo,
            size: [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1],
        })
    }

    /// The bottom layer (its lowest y).
    pub fn bottom(&self) -> Vec<Pos> {
        self.positions()
            .into_iter()
            .filter(|p| p.y == self.min.y)
            .collect()
    }

    /// The top layer.
    pub fn top(&self) -> Vec<Pos> {
        let y = self.max().y;
        self.positions().into_iter().filter(|p| p.y == y).collect()
    }
}

/// Why a group doesn't form.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// More blocks than the kind holds (the count found before stopping).
    TooBig(usize),
    /// Not a full box: the blocks outside the largest box and the holes, for particles.
    NotCuboid { outside: Vec<Pos>, holes: Vec<Pos> },
}

/// Whether a group forms one multiblock of at most `limit` blocks: if and
/// only if it fills a complete cuboid (spec 4, 3.1: all or nothing).
pub fn check(group: &Group, limit: usize) -> Result<Cuboid, Refusal> {
    if group.too_big || group.blocks.len() > limit {
        return Err(Refusal::TooBig(group.blocks.len()));
    }
    let Some(b) = Cuboid::bounding(&group.blocks) else {
        return Err(Refusal::NotCuboid {
            outside: Vec::new(),
            holes: Vec::new(),
        });
    };
    if b.volume() == group.blocks.len() {
        return Ok(b);
    }
    let set: BTreeSet<Pos> = group.blocks.iter().copied().collect();
    let best = largest_cuboid(&set, &b);
    let outside = group
        .blocks
        .iter()
        .copied()
        .filter(|p| best.is_none_or(|c| !c.contains(*p)))
        .collect();
    let holes = b
        .positions()
        .into_iter()
        .filter(|p| !set.contains(p))
        .collect();
    Err(Refusal::NotCuboid { outside, holes })
}

/// The largest full box inside `set` (within its bounding box `b`), for the
/// wrench's particles: brute force, the groups are small.
fn largest_cuboid(set: &BTreeSet<Pos>, b: &Cuboid) -> Option<Cuboid> {
    let mut best: Option<Cuboid> = None;
    let m = b.max();
    for x0 in b.min.x..=m.x {
        for y0 in b.min.y..=m.y {
            for z0 in b.min.z..=m.z {
                for x1 in x0..=m.x {
                    for y1 in y0..=m.y {
                        for z1 in z0..=m.z {
                            let c = Cuboid {
                                min: Pos::new(x0, y0, z0),
                                size: [x1 - x0 + 1, y1 - y0 + 1, z1 - z0 + 1],
                            };
                            if best.is_some_and(|bc| bc.volume() >= c.volume()) {
                                continue;
                            }
                            if c.positions().iter().all(|p| set.contains(p)) {
                                best = Some(c);
                            }
                        }
                    }
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(blocks: &[Pos], limit: usize) -> Result<Cuboid, Refusal> {
        let set: BTreeSet<Pos> = blocks.iter().copied().collect();
        let g = group(blocks[0], &|p| set.contains(&p), limit);
        check(&g, limit)
    }

    fn boxed(size: [i32; 3], at: Pos) -> Vec<Pos> {
        Cuboid { min: at, size }.positions()
    }

    #[test]
    fn every_valid_shape_in_every_orientation_forms() {
        let o = Pos::new(3, 60, -7);
        let mut shapes = Vec::new();
        for (a, b, c) in [(1, 1, 1), (1, 1, 2), (1, 1, 3), (1, 1, 4), (1, 2, 2)] {
            shapes.push(([a, b, c], 4));
        }
        for (a, b, c) in [(1, 1, 5), (1, 1, 8), (1, 2, 3), (1, 2, 4), (2, 2, 2)] {
            shapes.push(([a, b, c], 8));
        }
        for (size, limit) in shapes {
            let [a, b, c] = size;
            for s in [
                [a, b, c],
                [a, c, b],
                [b, a, c],
                [b, c, a],
                [c, a, b],
                [c, b, a],
            ] {
                let got = form(&boxed(s, o), limit).unwrap();
                assert_eq!(got.size, s);
                assert_eq!(got.min, o);
            }
        }
        // Tanks up to 3×3×3.
        assert!(form(&boxed([3, 3, 3], o), 27).is_ok());
    }

    #[test]
    fn l_shapes_holes_and_too_many_blocks_dont_form() {
        let o = Pos::new(0, 0, 0);
        let mut l = boxed([2, 1, 2], o);
        l.retain(|p| *p != Pos::new(1, 0, 1));
        match form(&l, 8) {
            Err(Refusal::NotCuboid { holes, .. }) => assert_eq!(holes, vec![Pos::new(1, 0, 1)]),
            other => panic!("{other:?}"),
        }
        let mut hollow = boxed([3, 3, 3], o);
        hollow.retain(|p| *p != Pos::new(1, 1, 1));
        assert!(matches!(form(&hollow, 27), Err(Refusal::NotCuboid { .. })));
        let nine = boxed([1, 1, 9], o);
        assert!(matches!(form(&nine, 8), Err(Refusal::TooBig(9))));
        let five = boxed([1, 1, 5], o);
        assert!(matches!(form(&five, 4), Err(Refusal::TooBig(5))));
        // An extra block on a 2×2×2 is reported outside the box.
        let mut extra = boxed([2, 2, 2], o);
        extra.push(Pos::new(2, 0, 0));
        match form(&extra, 27) {
            Err(Refusal::NotCuboid { outside, .. }) => assert_eq!(outside, vec![Pos::new(2, 0, 0)]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_long_axis_and_its_ties() {
        let c = |size| Cuboid {
            min: Pos::new(0, 0, 0),
            size,
        };
        assert_eq!(c([1, 4, 1]).long_axis(), Axis::Y);
        assert_eq!(c([4, 1, 1]).long_axis(), Axis::X);
        assert_eq!(c([1, 1, 4]).long_axis(), Axis::Z);
        assert_eq!(c([2, 2, 2]).long_axis(), Axis::Y);
        assert_eq!(c([2, 1, 2]).long_axis(), Axis::X);
        assert_eq!(c([1, 1, 1]).long_axis(), Axis::Y);
    }

    #[test]
    fn formation_does_not_depend_on_where_the_search_starts() {
        let blocks = boxed([1, 2, 4], Pos::new(5, 5, 5));
        let set: BTreeSet<Pos> = blocks.iter().copied().collect();
        let results: Vec<_> = blocks
            .iter()
            .map(|p| check(&group(*p, &|q| set.contains(&q), 8), 8))
            .collect();
        assert!(results.windows(2).all(|w| w[0] == w[1]));
    }
}
