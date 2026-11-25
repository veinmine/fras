//! Warning timers and jams (rotational network spec 6.2 to 6.4).

/// Seconds a part survives just over its limit.
pub const WARN_SECONDS: f64 = 5.0;
/// At this multiple of its limit the timer fills faster...
pub const FAST_FACTOR: f64 = 1.5;
/// ...by this much.
pub const FAST_RATE: f64 = 5.0;

/// Why a part jammed. Saved with the part.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Jam {
    /// Turned too fast for too long, or past the hard cap.
    Overspeed,
    /// Carried more torque than its limit for too long (shafts).
    Shear,
    /// Joined two networks turning opposite ways.
    Shock,
    /// Closed a loop that demands two speeds for one part.
    Conflict,
    /// A flywheel whose plane a solid block entered (multiblocks spec 4).
    Blocked,
    /// A part rusted through (rust spec 6): scraping clears it.
    Rusted,
}

impl Jam {
    pub fn name(self) -> &'static str {
        match self {
            Jam::Overspeed => "overspeed",
            Jam::Shear => "shear",
            Jam::Shock => "shock",
            Jam::Conflict => "conflict",
            Jam::Blocked => "blocked",
            Jam::Rusted => "rusted",
        }
    }

    pub fn from_name(name: &str) -> Option<Jam> {
        [
            Jam::Overspeed,
            Jam::Shear,
            Jam::Shock,
            Jam::Conflict,
            Jam::Blocked,
            Jam::Rusted,
        ]
        .into_iter()
        .find(|j| j.name() == name)
    }
}

/// One tick of a warning timer (0 to 1) for a part at `value` against its
/// `limit`. Returns the new timer; 1 or more means the part jams.
pub fn tick_timer(timer: f64, value: f64, limit: f64, dt: f64) -> f64 {
    let base = dt / WARN_SECONDS;
    let value = value.abs();
    if value > limit {
        let rate = if value >= FAST_FACTOR * limit {
            FAST_RATE
        } else {
            1.0
        };
        timer + base * rate
    } else {
        (timer - base).max(0.0)
    }
}

/// The warning level clients get (0 to 3) for a timer.
pub fn warning_level(timer: f64) -> u8 {
    if timer <= 0.0 {
        0
    } else if timer < 0.4 {
        1
    } else if timer < 0.75 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.05;

    fn ticks_to_jam(value: f64, limit: f64) -> usize {
        let mut t = 0.0;
        for i in 1..10_000 {
            t = tick_timer(t, value, limit, DT);
            if t >= 1.0 {
                return i;
            }
        }
        panic!("never jammed");
    }

    #[test]
    fn just_over_jams_after_five_seconds() {
        assert_eq!(ticks_to_jam(70.0, 64.0), 100);
    }

    #[test]
    fn far_over_jams_five_times_faster() {
        assert_eq!(ticks_to_jam(96.0, 64.0), 20);
        assert_eq!(ticks_to_jam(-200.0, 64.0), 20);
    }

    #[test]
    fn the_timer_drains_back_in_range_and_never_goes_negative() {
        let mut t = 0.0;
        for _ in 0..50 {
            t = tick_timer(t, 70.0, 64.0, DT);
        }
        assert!((t - 0.5).abs() < 1e-9);
        for _ in 0..30 {
            t = tick_timer(t, 10.0, 64.0, DT);
        }
        assert!((t - 0.2).abs() < 1e-9);
        for _ in 0..100 {
            t = tick_timer(t, 10.0, 64.0, DT);
        }
        assert_eq!(t, 0.0);
    }

    #[test]
    fn repeated_spikes_add_up() {
        let mut t = 0.0;
        for _ in 0..4 {
            for _ in 0..30 {
                t = tick_timer(t, 70.0, 64.0, DT);
            }
            for _ in 0..10 {
                t = tick_timer(t, 10.0, 64.0, DT);
            }
        }
        assert!(t > 0.7);
    }

    #[test]
    fn names_round_trip() {
        for j in [
            Jam::Overspeed,
            Jam::Shear,
            Jam::Shock,
            Jam::Conflict,
            Jam::Blocked,
            Jam::Rusted,
        ] {
            assert_eq!(Jam::from_name(j.name()), Some(j));
        }
        assert_eq!(warning_level(0.0), 0);
        assert_eq!(warning_level(0.9), 3);
    }
}
