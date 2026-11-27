//! The copper golem crank (`specs/copper-golem.md`): where an assigned golem
//! stands, which way it faces, its torque, and how its arm is posed to follow
//! the crank's handle. Free of the game.
//!
//! Frames used here:
//! - **world**: blocks, the crank block's centre as origin where noted.
//! - **body**: the golem's own frame at its feet, as `render_living_pre` poses
//!   its drawing: +z (`f`) the way it faces, +x (`l`) its left, +y (`h`) up,
//!   in blocks.
//! - **model**: a vanilla mob model's frame, in texels (16 to a block): +y
//!   down, its front towards -z, its feet 24 texels down (1.501 blocks, as
//!   `LivingEntityRenderer` translates).

use super::geom::{Axis, Dir, Pos};

/// An assigned golem turns its crank at a steady speed, the grinder's
/// minimum, as long as the load stays within what it can push: up to
/// `GOLEM_TORQUE` Nm, beyond which it slows (Marcin, 2026-11-27, replacing
/// spec 4's 8 Nm / 16 RPM curve, which sagged to 3 RPM under a grinder).
pub const GOLEM_RPM: f64 = 12.0;
pub const GOLEM_TORQUE: f64 = 12.0;
/// How stiffly it holds its speed (Nm per unit of the drive line), as the
/// creative engine does.
const STIFF: f64 = 100_000.0;

/// The golem's drive at the crank's speed `w` (RPM, forward positive): the
/// stall and top speed of the straight line the network takes for this
/// tick. Its top is set a hair above `GOLEM_RPM` so the speed is exactly
/// `GOLEM_RPM` at full torque and only above it under less (a grinder needs
/// at least 12, not 11.999); below that speed the stall is lowered so the
/// torque stays `GOLEM_TORQUE`.
pub fn drive(w: f64) -> (f64, f64) {
    let top = GOLEM_RPM / (1.0 - GOLEM_TORQUE / STIFF);
    let w = w.max(0.0);
    if w >= top {
        return (STIFF, top);
    }
    let stall = (GOLEM_TORQUE / (1.0 - w / top)).min(STIFF);
    (stall, top)
}

/// How far an assigned golem may be from where it stands at the crank
/// (blocks, horizontally) and still count as there: closer than this it is
/// snapped into place and starts turning.
pub const ARRIVE: f64 = 1.1;
/// Farther than this from its crank the golem is let go (its lead would snap).
pub const LEASH_RANGE: f64 = 10.0;

/// Where the golem's feet stand off the working spot's centre: towards the
/// crank (close, but its head clear of the grip at the top of the turn) and to its left (so
/// its right arm lines up with the crank's axle). Both keep its 0.49-wide box
/// inside the spot's block.
pub const FORWARD: f64 = 0.12;
pub const LEFT: f64 = 0.24;

/// The crank's grip (draw.rs): 5.5 texels off the axle, its middle 1.5
/// texels into the block from the handle side.
const GRIP_RADIUS: f64 = 5.5 / 16.0;
const GRIP_INSET: f64 = 6.5 / 16.0;

/// The working spot: the block on the crank's handle side, the side away
/// from its socket. None for a crank turned up or down, where a golem can't
/// reach the handle.
pub fn spot(crank: Pos, socket: Dir) -> Option<Pos> {
    (socket.axis() != Axis::Y).then(|| crank.offset(socket.opposite()))
}

fn unit(d: Dir) -> [f64; 3] {
    let s = d.step();
    [s[0] as f64, s[1] as f64, s[2] as f64]
}

/// The golem's left when it faces `forward` (horizontal).
fn left_of(forward: [f64; 3]) -> [f64; 3] {
    [forward[2], 0.0, -forward[0]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where the golem's feet go (world, blocks) for a crank at `crank` with its
/// socket on `socket`, and the yaw (degrees, vanilla's: 0 south, 90 west) that
/// faces it towards the crank.
pub fn stand(crank: Pos, socket: Dir) -> Option<([f64; 3], f32)> {
    let spot = spot(crank, socket)?;
    let f = unit(socket);
    let l = left_of(f);
    let at = [
        spot.x as f64 + 0.5 + FORWARD * f[0] + LEFT * l[0],
        spot.y as f64,
        spot.z as f64 + 0.5 + FORWARD * f[2] + LEFT * l[2],
    ];
    Some((at, yaw(socket)))
}

/// The yaw facing `dir` (vanilla's: 0 south, 90 west, 180 north, -90 east).
pub fn yaw(dir: Dir) -> f32 {
    let f = unit(dir);
    (-f[0]).atan2(f[2]).to_degrees() as f32
}

/// The grip's centre in the golem's body frame (`[l, h, f]`, blocks: left, up,
/// forward, matching the drawing's x, y, z) with the crank turned to `angle`
/// degrees (its spin angle about the axle's positive end, counterclockwise
/// seen from there, draw.rs).
pub fn grip(socket: Dir, angle: f64) -> [f64; 3] {
    let axis = socket.axis();
    let f = unit(socket);
    let l = left_of(f);
    let a = {
        let u = axis.unit();
        [u[0] as f64, u[1] as f64, u[2] as f64]
    };
    // The grip at angle 0 (draw.rs `span`'s first cross axis).
    let u0 = match axis {
        Axis::X => [0.0, 1.0, 0.0],
        _ => [1.0, 0.0, 0.0],
    };
    let w = cross(a, u0);
    let (s, c) = angle.to_radians().sin_cos();
    let mut g = [0.0; 3];
    for i in 0..3 {
        g[i] = -GRIP_INSET * f[i] + GRIP_RADIUS * (u0[i] * c + w[i] * s);
    }
    // From the golem's feet: the crank's centre is 1 - FORWARD ahead, LEFT to
    // its right and half a block up.
    let mut t = [0.0; 3];
    for i in 0..3 {
        t[i] = g[i] + (1.0 - FORWARD) * f[i] - LEFT * l[i];
    }
    t[1] += 0.5;
    [dot(t, l), t[1], dot(t, f)]
}

/// A body-frame point (`[l, h, f]`, blocks) in model texels.
pub fn to_model(p: [f64; 3]) -> [f64; 3] {
    [p[0] * 16.0, (1.501 - p[1]) * 16.0, -p[2] * 16.0]
}

/// How the arm is posed: rotations (radians, as a model part takes them,
/// applied about x first, then z, vanilla's `rotationZYX`) and how much it is
/// stretched along its length.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ArmPose {
    pub x_rot: f32,
    pub z_rot: f32,
    pub stretch: f32,
}

/// The shortest and longest the arm is drawn, as a share of its length.
pub const STRETCH: [f64; 2] = [0.6, 1.5];

/// The pose that points an arm hanging from `shoulder` (model texels) along
/// +y, `length` texels from the shoulder to the hand, at `target` (model
/// texels), stretching it to reach.
pub fn arm_pose(shoulder: [f64; 3], length: f64, target: [f64; 3]) -> ArmPose {
    let d = [
        target[0] - shoulder[0],
        target[1] - shoulder[1],
        target[2] - shoulder[2],
    ];
    let n = dot(d, d).sqrt().max(1e-6);
    let u = [d[0] / n, d[1] / n, d[2] / n];
    // R = Rz(z) Ry(0) Rx(x) applied to (0, 1, 0) is
    // (-cos x sin z, cos x cos z, sin x).
    let x_rot = u[2].clamp(-1.0, 1.0).asin();
    let z_rot = (-u[0]).atan2(u[1]);
    let stretch = (n / length.max(1e-6)).clamp(STRETCH[0], STRETCH[1]);
    ArmPose {
        x_rot: x_rot as f32,
        z_rot: z_rot as f32,
        stretch: stretch as f32,
    }
}

/// Where a posed arm puts its hand (model texels): the inverse of
/// `arm_pose`, for checking it.
pub fn hand(shoulder: [f64; 3], length: f64, pose: ArmPose) -> [f64; 3] {
    let (sx, cx) = (pose.x_rot as f64).sin_cos();
    let (sz, cz) = (pose.z_rot as f64).sin_cos();
    let r = length * pose.stretch as f64;
    [
        shoulder[0] - r * cx * sz,
        shoulder[1] + r * cx * cz,
        shoulder[2] + r * sx,
    ]
}

/// Vanilla's copper golem's weathering stages, from its saved
/// `weather_state`: 0 unaffected to 3 oxidized.
pub fn stage(name: &str) -> u8 {
    match name.trim_start_matches("minecraft:") {
        "exposed" => 1,
        "weathered" => 2,
        "oxidized" => 3,
        _ => 0,
    }
}

/// The vanilla texture of a copper golem at weathering `stage`.
pub fn texture(stage: u8) -> &'static str {
    match stage {
        1 => "entity/copper_golem/copper_golem_exposed",
        2 => "entity/copper_golem/copper_golem_weathered",
        3 => "entity/copper_golem/copper_golem_oxidized",
        _ => "entity/copper_golem/copper_golem",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDES: [Dir; 4] = [Dir::North, Dir::South, Dir::East, Dir::West];

    #[test]
    fn spot_is_on_the_handle_side() {
        let c = Pos::new(0, 64, 0);
        assert_eq!(spot(c, Dir::East), Some(Pos::new(-1, 64, 0)));
        assert_eq!(spot(c, Dir::North), Some(Pos::new(0, 64, 1)));
        assert_eq!(spot(c, Dir::Up), None);
        assert_eq!(spot(c, Dir::Down), None);
    }

    #[test]
    fn faces_the_crank() {
        assert_eq!(yaw(Dir::South), 0.0);
        assert_eq!(yaw(Dir::West), 90.0);
        assert!((yaw(Dir::North).abs() - 180.0).abs() < 1e-4);
        assert_eq!(yaw(Dir::East), -90.0);
        // The golem stands towards the crank from its spot's centre and its
        // box stays inside the spot.
        let c = Pos::new(10, 64, -3);
        for s in SIDES {
            let (at, _) = stand(c, s).unwrap();
            let spot = spot(c, s).unwrap();
            let f = unit(s);
            let ahead = (at[0] - spot.x as f64 - 0.5) * f[0] + (at[2] - spot.z as f64 - 0.5) * f[2];
            assert!((ahead - FORWARD).abs() < 1e-9, "{s:?}");
            for (v, b) in [(at[0], spot.x), (at[2], spot.z)] {
                assert!(v - 0.245 >= b as f64 - 1e-9 && v + 0.245 <= b as f64 + 1.0 + 1e-9);
            }
        }
    }

    #[test]
    fn grip_circles_in_front_of_the_golem() {
        for s in SIDES {
            let mut lows = f64::MAX;
            let mut highs = f64::MIN;
            for i in 0..36 {
                let [l, h, f] = grip(s, i as f64 * 10.0);
                // Always the same distance ahead: the grip turns in the
                // crank's face plane, inset from the golem's side.
                assert!((f - (1.0 - FORWARD - GRIP_INSET)).abs() < 1e-9, "{s:?}");
                // About the axle, which is LEFT to the golem's right, half a
                // block up.
                let r = ((l + LEFT).powi(2) + (h - 0.5).powi(2)).sqrt();
                assert!((r - GRIP_RADIUS).abs() < 1e-9);
                lows = lows.min(h);
                highs = highs.max(h);
            }
            assert!((highs - lows - 2.0 * GRIP_RADIUS).abs() < 1e-3);
        }
    }

    #[test]
    fn grip_turns_with_the_axle() {
        // A crank on the x axis (socket east): angle 0 has the grip on top,
        // and turning counterclockwise seen from +x (y towards z) brings it
        // towards +z.
        let top = grip(Dir::East, 0.0);
        assert!((top[1] - (0.5 + GRIP_RADIUS)).abs() < 1e-9);
        let quarter = grip(Dir::East, 90.0);
        // Socket east: the golem faces east, its left is north (-z), so the
        // grip is on its right.
        assert!((quarter[0] + GRIP_RADIUS + LEFT).abs() < 1e-9);
    }

    #[test]
    fn the_arm_reaches_the_grip() {
        // Vanilla's copper golem: its right shoulder 4 texels right of its
        // middle, 13 down from its model origin, the arm about 8 texels to
        // the hand.
        let shoulder = [-5.5, 13.0, 0.0];
        for s in SIDES {
            for i in 0..72 {
                let target = to_model(grip(s, i as f64 * 5.0));
                let pose = arm_pose(shoulder, 8.0, target);
                assert!(pose.stretch > STRETCH[0] as f32 && pose.stretch < STRETCH[1] as f32);
                let h = hand(shoulder, 8.0, pose);
                for k in 0..3 {
                    assert!(
                        (h[k] - target[k]).abs() < 1e-3,
                        "{s:?} {i} {h:?} {target:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn hanging_arm_is_unposed() {
        let pose = arm_pose([0.0; 3], 8.0, [0.0, 8.0, 0.0]);
        assert_eq!(pose.x_rot, 0.0);
        assert_eq!(pose.z_rot, 0.0);
        assert_eq!(pose.stretch, 1.0);
    }

    /// The torque the drive gives at `w`, as the network's straight line does.
    fn torque(w: f64) -> f64 {
        let (stall, top) = drive(w);
        (stall * (1.0 - w / top)).clamp(-stall, stall)
    }

    #[test]
    fn holds_twelve_rpm_up_to_its_torque() {
        // Pushes no more than its torque however slow it turns.
        for w in [0.0, 3.0, 6.0, 11.0, 11.99] {
            assert!((torque(w) - GOLEM_TORQUE).abs() < 1e-6, "{w}");
        }
        // Settles where the line meets the load: at 12 RPM or a hair above
        // for any load it can carry (a grinder's 6 Nm and friction).
        for load in [0.5, 3.0, 6.6, 12.0] {
            let (stall, top) = drive(GOLEM_RPM);
            let settled = top * (1.0 - load / stall);
            assert!(settled >= GOLEM_RPM - 1e-9, "{load} {settled}");
            assert!(settled < GOLEM_RPM + 0.01, "{load} {settled}");
        }
        // Past it, it slows instead of pushing harder.
        assert!(torque(12.5) < 0.0);
    }

    #[test]
    fn weathering_stages() {
        assert_eq!(stage("unaffected"), 0);
        assert_eq!(stage("exposed"), 1);
        assert_eq!(stage("weathered"), 2);
        assert_eq!(stage("minecraft:oxidized"), 3);
    }
}
