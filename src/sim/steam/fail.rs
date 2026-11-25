//! Rated pressures and the failure ladder (steam and heat spec 6 and 7).

/// Seconds a part survives just over its rating.
pub const WARN_SECONDS: f64 = 10.0;
/// From this multiple of its rating the timer fills five times faster...
pub const FAST_FACTOR: f64 = 1.25;
pub const FAST_RATE: f64 = 5.0;
/// ...and from this one the part fails at once.
pub const INSTANT_FACTOR: f64 = 1.5;
/// The hiss and the wisps start at this share of the rating.
pub const SIMMER_SHARE: f64 = 0.9;
/// The strongest rupture in tier 1 (TNT).
pub const POWER_CAP: f64 = 4.0;
/// How long a steam cloud lasts, seconds.
pub const CLOUD_SECONDS: f64 = 5.0;
/// A dry boiler starts glowing at this shell temperature.
pub const GLOW_TEMP: f64 = 120.0;

/// Rated pressures, bar *(tune)*.
pub const COPPER_BOILER: f64 = 4.0;
pub const IRON_BOILER: f64 = 6.0;
pub const COPPER_PIPE: f64 = 4.0;
pub const IRON_PIPE: f64 = 8.0;
pub const VALVE: f64 = 8.0;
pub const STEAM_TANK: f64 = 6.0;
pub const CYLINDER: f64 = 8.0;

/// Dry-firing rupture temperatures, °C.
pub const COPPER_DRY: f64 = 300.0;
pub const IRON_DRY: f64 = 400.0;

/// One step of a warning timer (0 to 1) at pressure `p` against `rating`.
/// Returns the new timer; 1 or more means the part fails (at once from 1.5×).
pub fn tick_timer(timer: f64, p: f64, rating: f64, dt: f64) -> f64 {
    let ratio = p / rating;
    if ratio >= INSTANT_FACTOR {
        return 1.0;
    }
    let base = dt / WARN_SECONDS;
    if ratio > 1.0 {
        let rate = if ratio >= FAST_FACTOR { FAST_RATE } else { 1.0 };
        timer + base * rate
    } else {
        (timer - base).max(0.0)
    }
}

/// Whether a timer has filled.
pub fn failed(timer: f64) -> bool {
    timer >= 1.0 - 1e-9
}

/// The warning level clients show: 0 none, 1 simmer (from 90%), 2 warning
/// (over the rating), 3 critical (1.25× or more, or the timer past half).
pub fn warning_level(p: f64, rating: f64, timer: f64) -> u8 {
    let ratio = p / rating;
    if ratio >= FAST_FACTOR || timer > 0.5 {
        3
    } else if ratio > 1.0 || timer > 0.0 {
        2
    } else if ratio >= SIMMER_SHARE {
        1
    } else {
        0
    }
}

/// The power of a rupture at pressure `p` of a multiblock of `n` blocks.
pub fn rupture_power(p: f64, n: u32) -> f64 {
    (0.25 * p.max(0.0) * (n as f64).cbrt()).min(POWER_CAP)
}

/// A dry rupture: from the pressure, but at least 1.
pub fn dry_rupture_power(p: f64, n: u32) -> f64 {
    rupture_power(p, n).max(1.0)
}

/// The steam cloud's radius, blocks.
pub fn cloud_radius(n: u32) -> f64 {
    1.0 + (n as f64).cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.2;

    fn steps_to_fail(p: f64, rating: f64) -> usize {
        let mut t = 0.0;
        for i in 1..10_000 {
            t = tick_timer(t, p, rating, DT);
            if failed(t) {
                return i;
            }
        }
        panic!("never");
    }

    #[test]
    fn just_over_fails_after_ten_seconds_faster_far_over_at_once_beyond() {
        assert_eq!(steps_to_fail(4.2, 4.0), 50);
        assert_eq!(steps_to_fail(5.0, 4.0), 10);
        assert_eq!(steps_to_fail(6.0, 4.0), 1);
    }

    #[test]
    fn the_timer_drains_under_the_rating() {
        let mut t = 0.0;
        for _ in 0..25 {
            t = tick_timer(t, 4.2, 4.0, DT);
        }
        assert!((t - 0.5).abs() < 1e-9);
        for _ in 0..10 {
            t = tick_timer(t, 3.0, 4.0, DT);
        }
        assert!((t - 0.3).abs() < 1e-9);
        for _ in 0..100 {
            t = tick_timer(t, 3.0, 4.0, DT);
        }
        assert_eq!(t, 0.0);
    }

    #[test]
    fn rupture_power_and_its_cap() {
        // A 4-block copper boiler at 5 bar: about 2.
        assert!((rupture_power(5.0, 4) - 1.98).abs() < 0.01);
        // An 8-block iron boiler at 7.5 bar: 3.75.
        assert!((rupture_power(7.5, 8) - 3.75).abs() < 1e-9);
        assert_eq!(rupture_power(30.0, 27), POWER_CAP);
        assert_eq!(dry_rupture_power(0.0, 8), 1.0);
        assert_eq!(cloud_radius(8), 3.0);
    }

    #[test]
    fn warning_levels() {
        assert_eq!(warning_level(3.0, 4.0, 0.0), 0);
        assert_eq!(warning_level(3.7, 4.0, 0.0), 1);
        assert_eq!(warning_level(4.1, 4.0, 0.1), 2);
        assert_eq!(warning_level(5.0, 4.0, 0.1), 3);
        assert_eq!(warning_level(3.0, 4.0, 0.2), 2);
    }
}
