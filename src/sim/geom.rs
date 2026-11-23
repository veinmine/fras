//! Block positions, axes and faces, free of the game.

/// A block position.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Pos { x, y, z }
    }

    pub fn offset(self, dir: Dir) -> Pos {
        let [dx, dy, dz] = dir.step();
        Pos::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub fn offset_by(self, dir: Dir, n: i32) -> Pos {
        let [dx, dy, dz] = dir.step();
        Pos::new(self.x + dx * n, self.y + dy * n, self.z + dz * n)
    }

    /// The chunk column holding this position.
    pub fn chunk(self) -> Chunk {
        Chunk {
            x: self.x >> 4,
            z: self.z >> 4,
        }
    }

    pub fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn from_array(p: [i32; 3]) -> Pos {
        Pos::new(p[0], p[1], p[2])
    }

    /// The component along `axis`.
    pub fn along(self, axis: Axis) -> i32 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }
}

/// A chunk column.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
}

/// A rotation axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }

    pub fn from_name(name: &str) -> Option<Axis> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    /// The face on the positive end of the axis.
    pub fn positive(self) -> Dir {
        match self {
            Axis::X => Dir::East,
            Axis::Y => Dir::Up,
            Axis::Z => Dir::South,
        }
    }

    pub fn negative(self) -> Dir {
        self.positive().opposite()
    }

    /// The unit vector along the axis.
    pub fn unit(self) -> [i32; 3] {
        self.positive().step()
    }
}

/// A block face, in vanilla's order (`Direction` data values).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Dir {
    pub const ALL: [Dir; 6] = [
        Dir::Down,
        Dir::Up,
        Dir::North,
        Dir::South,
        Dir::West,
        Dir::East,
    ];
    pub const HORIZONTAL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Dir> {
        Dir::ALL.get(i).copied()
    }

    pub fn axis(self) -> Axis {
        match self {
            Dir::Down | Dir::Up => Axis::Y,
            Dir::North | Dir::South => Axis::Z,
            Dir::West | Dir::East => Axis::X,
        }
    }

    /// Whether the face points along its axis's positive direction.
    pub fn is_positive(self) -> bool {
        matches!(self, Dir::Up | Dir::South | Dir::East)
    }

    /// +1 for a positive face, -1 for a negative one.
    pub fn sign(self) -> f64 {
        if self.is_positive() { 1.0 } else { -1.0 }
    }

    pub fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }

    pub fn step(self) -> [i32; 3] {
        match self {
            Dir::Down => [0, -1, 0],
            Dir::Up => [0, 1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
            Dir::East => [1, 0, 0],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
            Dir::West => "west",
            Dir::East => "east",
        }
    }

    pub fn from_name(name: &str) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.name() == name)
    }

    /// The faces perpendicular to this one, in vanilla order.
    pub fn perpendicular(self) -> impl Iterator<Item = Dir> {
        Dir::ALL
            .into_iter()
            .filter(move |d| d.axis() != self.axis())
    }

    /// The next horizontal face clockwise seen from above (N, E, S, W).
    pub fn clockwise(self) -> Dir {
        match self {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_round_trip() {
        for d in Dir::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_eq!(Dir::from_name(d.name()), Some(d));
            assert_eq!(Dir::from_index(d.index()), Some(d));
            assert_eq!(d.axis(), d.opposite().axis());
            assert_ne!(d.is_positive(), d.opposite().is_positive());
            let p = Pos::new(3, 4, 5).offset(d).offset(d.opposite());
            assert_eq!(p, Pos::new(3, 4, 5));
        }
        for a in Axis::ALL {
            assert_eq!(a.positive().axis(), a);
            assert!(a.positive().is_positive());
            assert_eq!(Axis::from_name(a.name()), Some(a));
        }
    }

    #[test]
    fn chunks_floor_negative_coordinates() {
        assert_eq!(Pos::new(-1, 0, 15).chunk(), Chunk { x: -1, z: 0 });
        assert_eq!(Pos::new(16, 0, -17).chunk(), Chunk { x: 1, z: -2 });
    }
}
