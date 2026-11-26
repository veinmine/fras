//! The furnace damper (regulation spec 4): a flap that turns a boiler's
//! fire down as its pressure nears the set pressure, and shuts on redstone.

/// The band below the set pressure over which it closes, bar *(tune)*.
pub const BAND: f64 = 1.0;
/// Travel, share per second (4 s open to shut).
pub const TRAVEL: f64 = 0.25;
/// Set pressure range and step, bar.
pub const SET_MIN: f64 = 1.0;
pub const SET_MAX: f64 = 8.0;
pub const SET_STEP: f64 = 0.5;
/// A new damper's set pressure: the red line minus this.
pub const BELOW_RED: f64 = 1.0;
/// With nothing to read, the set pressure a damper takes.
pub const DEFAULT_SET: f64 = 3.0;
/// The firebox's light while banked.
pub const BANKED_LIGHT: u8 = 7;

/// Where the flap heads: open 1 bar below the set pressure, shut at it,
/// shut whenever powered.
pub fn target(p: f64, set: f64, powered: bool) -> f64 {
    if powered {
        0.0
    } else {
        ((set - p) / BAND).clamp(0.0, 1.0)
    }
}

/// One step of the flap toward `target`, at most `TRAVEL` per second.
pub fn travel(h: f64, target: f64, dt: f64) -> f64 {
    let max = TRAVEL * dt;
    (h + (target - h).clamp(-max, max)).clamp(0.0, 1.0)
}

/// The set pressure a new damper takes on a boiler with red line `red`.
pub fn default_set(red: Option<f64>) -> f64 {
    match red {
        Some(r) if r > 0.0 => snap(r - BELOW_RED),
        _ => DEFAULT_SET,
    }
}

/// The next set pressure, up in 0.5 bar steps and back to 1.0.
pub fn next_set(set: f64) -> f64 {
    let next = snap(set + SET_STEP);
    if next > SET_MAX + 1e-9 || next <= set {
        SET_MIN
    } else {
        next
    }
}

fn snap(v: f64) -> f64 {
    ((v / SET_STEP).round() * SET_STEP).clamp(SET_MIN, SET_MAX)
}

/// The flap's look, 0 (shut) to 3 (open).
pub fn flap(h: f64) -> u8 {
    (h * 3.0).round().clamp(0.0, 3.0) as u8
}

/// A firebox's burn rate: the mean opening of the boilers it heats (1 when
/// it heats nothing).
pub fn burn_factor(openings: &[f64]) -> f64 {
    if openings.is_empty() {
        1.0
    } else {
        openings.iter().sum::<f64>() / openings.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_from_pressure_travel_and_redstone() {
        assert_eq!(target(2.0, 3.5, false), 1.0);
        assert_eq!(target(3.0, 3.5, false), 0.5);
        assert_eq!(target(3.6, 3.5, false), 0.0);
        assert_eq!(target(0.0, 3.5, true), 0.0);
        // 4 s from open to shut.
        let mut h = 1.0;
        let mut steps = 0;
        while h > 0.0 {
            h = travel(h, 0.0, 0.2);
            steps += 1;
        }
        assert_eq!(steps, 20);
        assert_eq!(travel(0.4, 0.45, 0.2), 0.45);
        assert_eq!(default_set(Some(6.0)), 5.0);
        assert_eq!(default_set(Some(4.0)), 3.0);
        assert_eq!(default_set(None), DEFAULT_SET);
        assert_eq!(next_set(7.5), 8.0);
        assert_eq!(next_set(8.0), 1.0);
        assert_eq!(flap(0.5), 2);
    }

    #[test]
    fn burn_rate_is_the_mean_over_its_boilers() {
        assert_eq!(burn_factor(&[]), 1.0);
        assert_eq!(burn_factor(&[1.0, 0.0]), 0.5);
        assert_eq!(burn_factor(&[0.0]), 0.0);
    }
}
