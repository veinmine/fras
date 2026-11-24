//! Valves (fluids and pipes spec 6).

/// The manual valve opens in steps of this much.
pub const STEP: f64 = 0.1;
/// Emergency valve: set pressure range and step, bar.
pub const SET_MIN: f64 = 1.0;
pub const SET_MAX: f64 = 8.0;
pub const SET_STEP: f64 = 0.5;
/// It simmers from this far below its set pressure, and reseats this far below.
pub const SIMMER_BELOW: f64 = 0.2;
pub const RESEAT_BELOW: f64 = 0.3;
/// Set pressure when it attaches to nothing.
pub const DEFAULT_SET: f64 = 3.5;

/// One more step open (or closed) on a manual valve, 0 to 1.
pub fn step(opening: f64, open: bool) -> f64 {
    let steps = (opening / STEP).round() + if open { 1.0 } else { -1.0 };
    (steps.clamp(0.0, 1.0 / STEP)) * STEP
}

/// An emergency valve's next state: it opens at its set pressure and
/// reseats 0.3 bar below.
pub fn emergency(open: bool, p: f64, set: f64) -> bool {
    if p >= set {
        true
    } else if p < set - RESEAT_BELOW {
        false
    } else {
        open
    }
}

/// Whether it simmers (a warning before it opens).
pub fn simmering(p: f64, set: f64) -> bool {
    p >= set - SIMMER_BELOW && p < set
}

/// The set pressure a new emergency valve takes: the network's red line
/// minus 0.5 bar, or 3.5 with none.
pub fn default_set(red_line: Option<f64>) -> f64 {
    match red_line {
        Some(r) => snap(r - SET_STEP),
        None => DEFAULT_SET,
    }
}

/// The next set pressure, cycling up in 0.5 bar steps and back to 1.0.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_valve_moves_in_tenths() {
        assert!((step(0.0, true) - 0.1).abs() < 1e-9);
        assert_eq!(step(1.0, true), 1.0);
        assert_eq!(step(0.0, false), 0.0);
        assert!((step(0.5, false) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn the_emergency_valve_simmers_opens_and_reseats() {
        let set = 3.5;
        assert!(simmering(3.35, set));
        assert!(!emergency(false, 3.4, set));
        assert!(emergency(false, 3.5, set));
        assert!(emergency(true, 3.3, set));
        assert!(!emergency(true, 3.19, set));
        assert_eq!(default_set(Some(4.0)), 3.5);
        assert_eq!(default_set(Some(8.0)), 7.5);
        assert_eq!(default_set(None), 3.5);
        assert_eq!(next_set(7.5), 8.0);
        assert_eq!(next_set(8.0), 1.0);
    }
}
