//! A boiler's water and temperature (steam and heat spec 4). Steam is not
//! held here: it goes to the boiler's steam network.

use super::{AMBIENT, BOIL_KJ, BOILER_WATER, BOILING, SHELL_KJ, WARM_KJ};

/// A formed boiler's thermal state.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Boiler {
    /// Its blocks (not its domes).
    pub blocks: u32,
    /// mB of water.
    pub water: f64,
    /// °C of the water and the shell together.
    pub temp: f64,
}

impl Boiler {
    pub fn new(blocks: u32) -> Boiler {
        Boiler {
            blocks,
            water: 0.0,
            temp: AMBIENT,
        }
    }

    /// The most water it holds, mB.
    pub fn capacity(&self) -> f64 {
        self.blocks as f64 * BOILER_WATER
    }

    /// Room for more water, mB.
    pub fn room(&self) -> f64 {
        (self.capacity() - self.water).max(0.0)
    }

    /// The shell's heat capacity, kJ/°C.
    pub fn shell(&self) -> f64 {
        SHELL_KJ * self.blocks as f64
    }

    /// The heat capacity of water and shell, kJ/°C (4.1 step 1).
    pub fn heat_capacity(&self) -> f64 {
        WARM_KJ * self.water + self.shell()
    }

    /// Dry: no water left.
    pub fn dry(&self) -> bool {
        self.water <= 1e-9
    }

    /// Heat it with `heat` kW for `dt` seconds (4.1 steps 2 to 4). Returns
    /// the mB of water boiled into steam.
    pub fn heat(&mut self, heat: f64, dt: f64) -> f64 {
        let mut energy = heat.max(0.0) * dt;
        if energy <= 0.0 {
            return 0.0;
        }
        // Warming, up to the boiling point.
        if self.temp < BOILING {
            let m = self.heat_capacity();
            let need = (BOILING - self.temp) * m;
            if energy < need {
                self.temp += energy / m;
                return 0.0;
            }
            energy -= need;
            self.temp = BOILING;
        }
        // Boiling, while there is water.
        let mut boiled = 0.0;
        if !self.dry() && self.temp <= BOILING {
            boiled = (energy / BOIL_KJ).min(self.water);
            self.water -= boiled;
            energy -= boiled * BOIL_KJ;
            if self.water < 1e-9 {
                self.water = 0.0;
            }
        }
        // A dry shell heats up by itself.
        if self.dry() && energy > 0.0 {
            self.temp += energy / self.shell();
        }
        boiled
    }

    /// Feed `fed` mB of water at `t_feed` °C into it (4.1 step 5). Returns
    /// the mB that flash into steam on a shell hotter than the boiling point.
    pub fn feed(&mut self, fed: f64, t_feed: f64) -> f64 {
        if fed <= 0.0 {
            return 0.0;
        }
        let m = self.heat_capacity();
        let added = WARM_KJ * fed;
        self.temp = (m * self.temp + added * t_feed) / (m + added);
        self.water += fed;
        if self.temp <= BOILING {
            return 0.0;
        }
        // The excess heat boils what it can at once.
        let excess = (self.temp - BOILING) * self.heat_capacity();
        let flashed = (excess / BOIL_KJ).min(self.water);
        self.water -= flashed;
        let left = excess - flashed * BOIL_KJ;
        self.temp = BOILING + left / self.heat_capacity();
        flashed
    }
}

#[cfg(test)]
mod tests {
    use super::super::DT;
    use super::*;

    fn full(blocks: u32) -> Boiler {
        let mut b = Boiler::new(blocks);
        b.water = b.capacity();
        b
    }

    /// Seconds until `pred` holds, heating at `heat` kW.
    fn seconds_until(b: &mut Boiler, heat: f64, mut pred: impl FnMut(&Boiler, f64) -> bool) -> f64 {
        let mut boiled = 0.0;
        for i in 1..100_000 {
            boiled += b.heat(heat, DT);
            if pred(b, boiled) {
                return i as f64 * DT;
            }
        }
        panic!("never");
    }

    #[test]
    fn a_cold_iron_boiler_boils_in_about_35_seconds() {
        // 2×2×2, two coal fireboxes (2000 kW).
        let mut b = full(8);
        let t = seconds_until(&mut b, 2000.0, |b, _| b.temp >= BOILING);
        assert!((30.0..40.0).contains(&t), "{t}");
        // Then about 80 mB/s: 1 bar of its 800 mB/bar every 10 s.
        let mut steam = 0.0;
        for _ in 0..50 {
            steam += b.heat(2000.0, DT);
        }
        assert!((steam / 800.0 - 1.0).abs() < 0.02, "{steam}");
    }

    #[test]
    fn a_tall_copper_boiler_on_one_firebox_matches_too() {
        let mut b = full(4);
        let t = seconds_until(&mut b, 1000.0, |b, _| b.temp >= BOILING);
        assert!((30.0..40.0).contains(&t), "{t}");
    }

    #[test]
    fn one_firebox_boils_40_a_second_and_empties_a_big_boiler_in_200() {
        let mut b = full(8);
        b.temp = BOILING;
        let t = seconds_until(&mut b, 1000.0, |b, _| b.dry());
        assert!((t - 200.0).abs() < 0.5, "{t}");
    }

    #[test]
    fn cold_feed_stops_the_boil_for_a_moment() {
        let mut b = full(4);
        b.temp = BOILING;
        b.water = 2000.0;
        assert_eq!(b.feed(1000.0, AMBIENT), 0.0);
        assert!(b.temp < BOILING - 10.0, "{}", b.temp);
        // The next steps warm it back before anything boils.
        assert_eq!(b.heat(1000.0, DT), 0.0);
    }

    #[test]
    fn a_dry_shell_heats_up_and_flashes_feed_water() {
        let mut b = Boiler::new(8);
        b.temp = BOILING;
        // Dry, two coal fireboxes: 25 °C/s.
        let before = b.temp;
        b.heat(2000.0, 1.0);
        assert!((b.temp - before - 25.0).abs() < 1e-9);
        b.temp = 300.0;
        let flashed = b.feed(100.0, AMBIENT);
        assert!(flashed > 50.0, "{flashed}");
        assert!(b.temp >= BOILING);
    }
}
