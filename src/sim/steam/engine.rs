//! The steam cylinder (steam and heat spec 9): its curve and its steam.

/// Stall torque per bar of pressure, Nm.
pub const STALL_PER_BAR: f64 = 16.0;
/// Top speed per bar, RPM.
pub const TOP_PER_BAR: f64 = 32.0;
/// Steam demand, mB/s per RPM per bar.
pub const DEMAND: f64 = 0.16;

/// A cylinder's stall torque at pressure `p` getting `fill` of its demand.
pub fn stall(p: f64, fill: f64) -> f64 {
    STALL_PER_BAR * p.max(0.0) * fill.clamp(0.0, 1.0)
}

/// A cylinder's top speed at pressure `p`.
pub fn top(p: f64) -> f64 {
    TOP_PER_BAR * p.max(0.0)
}

/// A cylinder's steam demand, mB/s, at flywheel speed `omega` and pressure `p`.
pub fn demand(omega: f64, p: f64) -> f64 {
    DEMAND * omega.abs() * p.max(0.0)
}

/// The share of its demand a cylinder got.
pub fn fill(delivered: f64, demand: f64) -> f64 {
    if demand <= 1e-9 {
        1.0
    } else {
        (delivered / demand).clamp(0.0, 1.0)
    }
}

/// The torque of `cylinders` cylinders at local speed `omega` (the
/// rotational source line, rotational spec 3.2), for tests.
pub fn torque(cylinders: f64, p: f64, fill: f64, omega: f64) -> f64 {
    let v = top(p);
    if v <= 0.0 {
        return 0.0;
    }
    cylinders * stall(p, fill) * (1.0 - omega / v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_at_several_pressures() {
        assert_eq!(stall(4.0, 1.0), 64.0);
        assert_eq!(top(4.0), 128.0);
        assert_eq!(stall(2.0, 1.0), 32.0);
        assert_eq!(top(2.0), 64.0);
        assert_eq!(torque(1.0, 4.0, 1.0, 128.0), 0.0);
        // A twin doubles the stall torque, not the top speed.
        assert_eq!(torque(2.0, 4.0, 1.0, 0.0), 128.0);
    }

    #[test]
    fn a_starved_engine_pushes_less_and_a_stalled_one_draws_nothing() {
        assert_eq!(stall(4.0, 0.5), 32.0);
        assert_eq!(demand(0.0, 4.0), 0.0);
        assert!((demand(100.0, 4.0) - 64.0).abs() < 1e-9);
        assert_eq!(fill(31.0, 62.0), 0.5);
        assert_eq!(fill(0.0, 0.0), 1.0);
    }
}
