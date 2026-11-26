//! The bellows (regulation spec 5): shaft-driven air for a firebox, more
//! heat from the same fuel.

/// Working range, load while blowing a fire at full burn, inertia *(tune)*.
pub const MIN_RPM: f64 = 16.0;
pub const MAX_RPM: f64 = 128.0;
pub const LOAD: f64 = 6.0;
pub const INERTIA: f64 = 4.0;
/// The most extra heat, and the speed it saturates at.
pub const MAX_BOOST: f64 = 0.5;
pub const FULL_RPM: f64 = 64.0;

/// The load while blowing a fire that burns at `burn` (its dampers' mean
/// opening, 0 to 1): a throttled fire needs less air.
pub fn load(burn: f64) -> f64 {
    LOAD * burn.clamp(0.0, 1.0)
}

/// The boost at `rpm`: 0 below the working range, up to 50% at 64 RPM.
pub fn boost(rpm: f64) -> f64 {
    let w = rpm.abs();
    if w < MIN_RPM {
        0.0
    } else {
        MAX_BOOST * (w / FULL_RPM).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_follows_the_burn_rate() {
        assert_eq!(load(1.0), LOAD);
        assert_eq!(load(0.5), 3.0);
        assert_eq!(load(0.0), 0.0);
        assert_eq!(load(2.0), LOAD);
    }

    #[test]
    fn boost_by_speed() {
        assert_eq!(boost(0.0), 0.0);
        assert_eq!(boost(15.9), 0.0);
        assert_eq!(boost(16.0), 0.125);
        assert_eq!(boost(32.0), 0.25);
        assert_eq!(boost(64.0), 0.5);
        assert_eq!(boost(-100.0), 0.5);
        // A coal firebox at 64 RPM: 1500 kW from the same coal.
        assert_eq!(1000.0 * (1.0 + boost(64.0)), 1500.0);
    }
}
