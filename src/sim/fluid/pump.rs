//! The pump (fluids and pipes spec 7).

/// Working range, RPM.
pub const MIN_RPM: f64 = 8.0;
pub const MAX_RPM: f64 = 128.0;
/// Load while it pumps, Nm.
pub const LOAD: f64 = 4.0;
/// mB/s for each water source block it touches, and the most it can draw.
pub const PER_SOURCE: f64 = 25.0;
pub const MAX_SOURCES: u32 = 5;
/// mB/s per RPM.
pub const PER_RPM: f64 = 0.75;

/// The water it can draw from `sources` source blocks (four intake faces,
/// plus itself if waterlogged).
pub fn source_cap(sources: u32) -> f64 {
    PER_SOURCE * sources.min(MAX_SOURCES) as f64
}

/// Its flow at `rpm` with `sources` source blocks, mB/s: nothing below its
/// working range, no more than at its top.
pub fn flow(rpm: f64, sources: u32) -> f64 {
    let w = rpm.abs();
    if w < MIN_RPM {
        return 0.0;
    }
    source_cap(sources).min(PER_RPM * w.min(MAX_RPM))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_from_speed_and_sources() {
        // On an engine at ~100 RPM with four sources: 75 mB/s.
        assert_eq!(flow(100.0, 4), 75.0);
        // On a water wheel (~20 RPM): 15 mB/s.
        assert_eq!(flow(20.0, 4), 15.0);
        // A one-block puddle: 25 at most, at any speed.
        assert_eq!(flow(128.0, 1), 25.0);
        assert_eq!(flow(-200.0, 9), 96.0);
        assert_eq!(flow(7.9, 4), 0.0);
    }
}
