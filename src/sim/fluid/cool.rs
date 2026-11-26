//! Machine water cooling (cooling spec 9): water in while working, hot
//! water out, a higher speed limit while cooled.

/// Water use, mB/s per RPM while working *(tune)*.
pub const PER_RPM: f64 = 0.1;
/// The hot water a machine holds before it backs up, mB.
pub const BUFFER: f64 = 100.0;
/// The grinder's limit while cooled, RPM.
pub const GRINDER_COOLED_MAX: f64 = 128.0;

/// A working machine's water demand at `rpm`, mB/s (nothing while idle).
pub fn demand(rpm: f64, working: bool) -> f64 {
    if working { PER_RPM * rpm.abs() } else { 0.0 }
}

/// Cooled: it got all the water it asked for and its hot water can leave.
/// A machine asking for nothing counts as cooled.
pub fn cooled(demand: f64, received: f64, buffer: f64) -> bool {
    received + 1e-6 >= demand && buffer < BUFFER - 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demand_and_cooled_state() {
        assert!((demand(96.0, true) - 9.6).abs() < 1e-9);
        assert!((demand(-120.0, true) - 12.0).abs() < 1e-9);
        assert_eq!(demand(120.0, false), 0.0);
        assert!(cooled(12.0, 12.0, 0.0));
        assert!(!cooled(12.0, 6.0, 0.0), "no water");
        assert!(!cooled(12.0, 12.0, BUFFER), "hot water can't get out");
        assert!(cooled(0.0, 0.0, 0.0));
    }
}
