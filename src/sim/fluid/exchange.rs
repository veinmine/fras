//! The heat exchanger, which is also the condenser (cooling spec 5), and
//! its radiators (6).

use super::Content;

/// Cooling power, kW *(tune)*.
pub const IN_AIR: f64 = 250.0;
pub const WATERLOGGED: f64 = 500.0;
pub const COPPER_RADIATOR: f64 = 125.0;
pub const IRON_RADIATOR: f64 = 75.0;
pub const MAX_RADIATORS: usize = 4;
/// Heat taken out per mB, kJ: condensing exhaust, cooling hot water.
pub const CONDENSE_KJ: f64 = 26.0;
pub const COOL_KJ: f64 = 7.0;
/// Its throughput (an iron valve body), mB/s, and its outlet buffer, mB.
pub const THROUGHPUT: f64 = 200.0;
pub const BUFFER: f64 = 100.0;

/// What a radiator on a side face gives.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Radiator {
    Copper,
    Iron,
    /// An iron radiator rusted through: no cooling (rust spec 6).
    Leaking,
}

impl Radiator {
    pub fn kw(self) -> f64 {
        match self {
            Radiator::Copper => COPPER_RADIATOR,
            Radiator::Iron => IRON_RADIATOR,
            Radiator::Leaking => 0.0,
        }
    }
}

/// Its cooling power, kW: its own (more waterlogged) and up to four radiators.
pub fn power(waterlogged: bool, radiators: &[Radiator]) -> f64 {
    let own = if waterlogged { WATERLOGGED } else { IN_AIR };
    own + radiators
        .iter()
        .take(MAX_RADIATORS)
        .map(|r| r.kw())
        .sum::<f64>()
}

/// What it turns `inlet` into, if anything.
pub fn converts(inlet: Content) -> Option<Content> {
    match inlet {
        Content::Exhaust => Some(Content::HotWater),
        Content::HotWater => Some(Content::Water),
        _ => None,
    }
}

/// How much of `inlet` it takes at most, mB/s, at `power` kW.
pub fn capacity(inlet: Content, power: f64) -> f64 {
    let kj = match inlet {
        Content::Exhaust => CONDENSE_KJ,
        Content::HotWater => COOL_KJ,
        _ => return 0.0,
    };
    (power / kj).min(THROUGHPUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_from_air_water_and_radiators() {
        assert_eq!(power(false, &[]), 250.0);
        assert_eq!(power(true, &[]), 500.0);
        assert_eq!(power(false, &[Radiator::Copper, Radiator::Iron]), 450.0);
        assert_eq!(power(true, &[Radiator::Copper; 4]), 1000.0);
        assert_eq!(power(true, &[Radiator::Copper; 6]), 1000.0);
        assert_eq!(power(false, &[Radiator::Leaking]), 250.0);
    }

    #[test]
    fn capacity_on_exhaust_and_on_hot_water() {
        // One fully fitted, waterlogged exchanger condenses about one coal
        // firebox's steam (40 mB/s).
        let c = capacity(Content::Exhaust, 1000.0);
        assert!((c - 38.46).abs() < 0.01, "{c}");
        assert!((capacity(Content::Exhaust, 750.0) - 28.8).abs() < 0.1);
        assert!((capacity(Content::HotWater, 1000.0) - 142.86).abs() < 0.01);
        assert_eq!(capacity(Content::HotWater, 5000.0), THROUGHPUT);
        assert_eq!(capacity(Content::Water, 1000.0), 0.0);
        assert_eq!(converts(Content::Exhaust), Some(Content::HotWater));
        assert_eq!(converts(Content::HotWater), Some(Content::Water));
        assert_eq!(converts(Content::Steam), None);
    }
}
