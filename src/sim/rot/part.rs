//! The parts of a rotational network: what each kind is, how it connects and
//! its numbers (rotational network spec, sections 5 to 9 and 14).

use crate::sim::geom::{Axis, Dir};

/// Shaft and gear materials.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Wood,
    Iron,
}

impl Material {
    pub fn name(self) -> &'static str {
        match self {
            Material::Wood => "wood",
            Material::Iron => "iron",
        }
    }
}

/// What a part is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Shaft(Material),
    Bearing,
    Clutch,
    Pulley {
        large: bool,
    },
    BevelGear(Material),
    Gearbox,
    Crank,
    WaterWheel,
    Grinder,
    /// The steam engine's flywheel: a source driven by its cylinders.
    Flywheel,
    /// A water pump: a machine.
    Pump,
}

/// How a part sits in the world: the faces it connects through.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    /// Along an axis, both ends connect: shafts, bearings, clutches,
    /// pulleys, the water wheel's axle.
    Inline(Axis),
    /// Inline 2:1, the large gear (slow side) on face `slow`.
    Gearbox { axis: Axis, slow: Dir },
    /// A bevel gear: `input`, `output` at 90°, and with `tee` also the face
    /// opposite `output`.
    Bevel { input: Dir, output: Dir, tee: bool },
    /// A machine or source with one shaft socket.
    Socket(Dir),
}

/// One end of a belt, kept on a pulley.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BeltEnd {
    pub partner: crate::sim::geom::Pos,
    pub crossed: bool,
}

/// Everything about a part that its block decides: the game reports it,
/// the network reads it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Spec {
    pub kind: Kind,
    pub shape: Shape,
    /// A clutch's engagement (false while powered); true for every other part.
    pub engaged: bool,
    /// A pulley's belt.
    pub belt: Option<BeltEnd>,
}

impl Spec {
    pub fn new(kind: Kind, shape: Shape) -> Spec {
        Spec {
            kind,
            shape,
            engaged: true,
            belt: None,
        }
    }

    pub fn shaft(material: Material, axis: Axis) -> Spec {
        Spec::new(Kind::Shaft(material), Shape::Inline(axis))
    }

    /// The faces this part connects through and their gains: the local speed
    /// of the neighbour across `face`, about the positive end of the face's
    /// axis, is `gain` times this part's own speed variable.
    ///
    /// The own variable is: for inline parts and sockets, the speed about the
    /// positive axis; for a gearbox, the fast side's; for a bevel gear, the
    /// spin about the input face's outward direction. Two meshing bevel gears
    /// turn opposite ways about their outward directions, so every face but
    /// the input spins the other way (spec 4.3).
    pub fn ports(&self) -> Vec<(Dir, f64)> {
        if !self.engaged {
            return Vec::new();
        }
        match self.shape {
            Shape::Inline(axis) => vec![(axis.negative(), 1.0), (axis.positive(), 1.0)],
            Shape::Gearbox { axis, slow } => {
                let fast = if slow.axis() == axis {
                    slow.opposite()
                } else {
                    axis.positive()
                };
                vec![(fast, 1.0), (fast.opposite(), GEARBOX_RATIO)]
            }
            Shape::Bevel { input, output, tee } => {
                let mut ports = vec![(input, input.sign()), (output, -output.sign())];
                if tee {
                    let other = output.opposite();
                    ports.push((other, -other.sign()));
                }
                ports
            }
            Shape::Socket(face) => vec![(face, 1.0)],
        }
    }

    /// The part's gain at `face`, if it connects there.
    pub fn port(&self, face: Dir) -> Option<f64> {
        self.ports()
            .into_iter()
            .find(|(d, _)| *d == face)
            .map(|(_, g)| g)
    }

    /// The axis a part turns about, where it has one (for spans and drawing).
    pub fn axis(&self) -> Option<Axis> {
        match self.shape {
            Shape::Inline(axis) | Shape::Gearbox { axis, .. } => Some(axis),
            Shape::Socket(face) => Some(face.axis()),
            Shape::Bevel { input, .. } => Some(input.axis()),
        }
    }

    pub fn inertia(&self) -> f64 {
        match self.kind {
            Kind::Shaft(_) | Kind::Bearing => 1.0,
            Kind::Clutch | Kind::BevelGear(_) | Kind::Crank => 2.0,
            Kind::Pulley { large } => {
                if large {
                    4.0
                } else {
                    2.0
                }
            }
            Kind::Gearbox | Kind::Grinder => 4.0,
            Kind::WaterWheel => 20.0,
            Kind::Flywheel => FLYWHEEL_INERTIA,
            Kind::Pump => 4.0,
        }
    }

    /// Viscous friction `c`, Nm per RPM at the part's own speed. A quarter of
    /// spec 14's values, with a larger constant term (`CONST_FRICTION`):
    /// viscous friction on a geared-up side counts with the ratio squared, and
    /// at the spec's values a water wheel geared up 4× couldn't pass 64 RPM
    /// (exit criterion 3). A shaft still costs about 1.5% of a crank's push.
    pub fn friction(&self) -> f64 {
        match self.kind {
            Kind::Shaft(_) | Kind::Bearing => 0.0025,
            Kind::Clutch | Kind::BevelGear(_) | Kind::Pulley { .. } | Kind::Crank => 0.005,
            Kind::Gearbox => 0.01,
            Kind::WaterWheel | Kind::Grinder | Kind::Flywheel | Kind::Pump => 0.0125,
        }
    }

    /// The highest local speed the part survives (spec 6.1); `None` for
    /// shaft segments and sources.
    pub fn max_rpm(&self) -> Option<f64> {
        match self.kind {
            // 56, not the spec's 64: in game a water wheel geared up 4×
            // levels off at ~63 RPM (exit criterion 3).
            Kind::BevelGear(Material::Wood) => Some(56.0),
            Kind::BevelGear(Material::Iron) | Kind::Gearbox => Some(192.0),
            Kind::Pulley { .. } => Some(96.0),
            Kind::Grinder => Some(GRINDER_MAX_RPM),
            Kind::Flywheel | Kind::Pump => Some(192.0),
            _ => None,
        }
    }

    /// For the gearbox, its limit is on the fast side, so the check is the
    /// part's own variable (the fast side's speed). Every other part's own
    /// variable is its local speed (bevel gears turn at 1:1).
    ///
    /// The torque a shaft segment carries before it shears (spec 6.3), Nm.
    pub fn torque_limit(&self) -> Option<f64> {
        match self.kind {
            Kind::Shaft(Material::Wood) => Some(32.0),
            Kind::Shaft(Material::Iron) | Kind::Bearing | Kind::Clutch => Some(128.0),
            _ => None,
        }
    }

    /// Supports end a span (spec 5): bearings and every non-shaft part
    /// except pulleys.
    pub fn is_support(&self) -> bool {
        !matches!(self.kind, Kind::Shaft(_) | Kind::Pulley { .. })
    }

    /// Part of a span: plain shafts and pulleys.
    pub fn in_span(&self) -> bool {
        !self.is_support()
    }

    /// The longest span this part allows: 8 for wood, 16 for iron (pulleys
    /// don't count against it).
    pub fn span_limit(&self) -> Option<usize> {
        match self.kind {
            Kind::Shaft(Material::Wood) => Some(8),
            Kind::Shaft(Material::Iron) => Some(16),
            _ => None,
        }
    }

    pub fn is_source(&self) -> bool {
        matches!(self.kind, Kind::Crank | Kind::WaterWheel | Kind::Flywheel)
    }

    pub fn is_machine(&self) -> bool {
        matches!(self.kind, Kind::Grinder | Kind::Pump)
    }

    /// The lowest speed a machine works at, RPM.
    pub fn min_working(&self) -> Option<f64> {
        match self.kind {
            Kind::Grinder => Some(GRINDER_MIN_RPM),
            Kind::Pump => Some(crate::sim::fluid::pump::MIN_RPM),
            _ => None,
        }
    }

    /// A pulley's diameter, in small-pulley units.
    pub fn pulley_diameter(&self) -> Option<f64> {
        match self.kind {
            Kind::Pulley { large } => Some(if large { 2.0 } else { 1.0 }),
            _ => None,
        }
    }

    /// The speed limit used for the deterministic choice in an
    /// inconsistent loop: unlimited parts sort last.
    pub fn loop_rank(&self) -> f64 {
        self.max_rpm().unwrap_or(f64::INFINITY)
    }
}

/// Gearbox: the slow side turns at half the fast side's speed.
pub const GEARBOX_RATIO: f64 = 0.5;
/// Constant friction `f` on every part, Nm.
pub const CONST_FRICTION: f64 = 0.15;
/// Any part above this speed jams at once.
pub const HARD_CAP_RPM: f64 = 256.0;
/// The grinder's working range.
pub const GRINDER_MIN_RPM: f64 = 12.0;
pub const GRINDER_MAX_RPM: f64 = 96.0;
/// The flywheel's inertia: 90, not spec 9.4's 200, which spins up in 22 s
/// against the spec's target of about 10 s to 90% of settled speed.
pub const FLYWHEEL_INERTIA: f64 = 90.0;
/// Crank: stall torque and top speed.
pub const CRANK_STALL: f64 = 16.0;
pub const CRANK_TOP: f64 = 32.0;
/// Water wheel: torque per flowing block, its cap, top speed.
pub const WHEEL_PER_BLOCK: f64 = 8.0;
pub const WHEEL_MAX: f64 = 32.0;
pub const WHEEL_TOP: f64 = 24.0;
/// A belt carries at most this torque at its smaller pulley.
pub const BELT_LIMIT: f64 = 24.0;
/// The longest belt, in blocks.
pub const BELT_MAX_DISTANCE: i32 = 16;
/// Joining networks turning opposite ways, both faster than this, jams the
/// connecting part.
pub const SHOCK_RPM: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn bevel(input: Dir, output: Dir, tee: bool) -> Spec {
        Spec::new(
            Kind::BevelGear(Material::Wood),
            Shape::Bevel { input, output, tee },
        )
    }

    /// Two meshing bevel gears: their teeth move the same way at the contact
    /// point. With the input spinning +1 about its outward direction, an
    /// output on face `o` spins -1 about o's outward direction.
    #[test]
    fn bevel_signs_follow_the_geometry_in_every_orientation() {
        for input in Dir::ALL {
            for output in input.perpendicular() {
                let spec = bevel(input, output, true);
                let ports = spec.ports();
                assert_eq!(ports.len(), 3);
                // Spin about the outward direction = gain * face sign.
                for (face, gain) in ports {
                    let outward_spin = gain * face.sign();
                    let want = if face == input { 1.0 } else { -1.0 };
                    assert_eq!(outward_spin, want, "{input:?}->{output:?} at {face:?}");
                }
                // The tee's two outputs turn opposite ways about their shared axis.
                let a = spec.port(output).unwrap();
                let b = spec.port(output.opposite()).unwrap();
                assert_eq!(a, -b);
            }
        }
    }

    /// Velocity check of the mesh geometry: Ω_in × p = Ω_out × p at the
    /// contact point p = u + v (u, v the faces' unit vectors).
    #[test]
    fn bevel_mesh_contact_velocities_match() {
        fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }
        let unit = |d: Dir| d.step().map(|c| c as f64);
        let axis_unit = |d: Dir| d.axis().unit().map(|c| c as f64);
        for input in Dir::ALL {
            for output in input.perpendicular() {
                let spec = bevel(input, output, false);
                let (u, v) = (unit(input), unit(output));
                let p = [u[0] + v[0], u[1] + v[1], u[2] + v[2]];
                // Angular velocity vectors about the global positive axes.
                let wi = axis_unit(input).map(|c| c * spec.port(input).unwrap());
                let wo = axis_unit(output).map(|c| c * spec.port(output).unwrap());
                assert_eq!(cross(wi, p), cross(wo, p), "{input:?}->{output:?}");
            }
        }
    }

    #[test]
    fn gearbox_halves_on_the_slow_side() {
        let g = Spec::new(
            Kind::Gearbox,
            Shape::Gearbox {
                axis: Axis::X,
                slow: Dir::East,
            },
        );
        assert_eq!(g.port(Dir::West), Some(1.0));
        assert_eq!(g.port(Dir::East), Some(0.5));
        assert_eq!(g.port(Dir::Up), None);
    }

    #[test]
    fn a_disengaged_clutch_connects_nothing() {
        let mut c = Spec::new(Kind::Clutch, Shape::Inline(Axis::Z));
        assert_eq!(c.ports().len(), 2);
        c.engaged = false;
        assert!(c.ports().is_empty());
    }
}
