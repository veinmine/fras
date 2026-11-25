//! A steam network's steam (steam and heat spec 5): one pressure per
//! network, `P = steam / C`.

/// The steam of one network.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pool {
    /// mB of steam held.
    pub steam: f64,
    /// Steam capacity, mB per bar.
    pub capacity: f64,
}

impl Pool {
    pub fn new(steam: f64, capacity: f64) -> Pool {
        Pool { steam, capacity }
    }

    /// Gauge pressure, bar.
    pub fn pressure(&self) -> f64 {
        if self.capacity <= 0.0 {
            0.0
        } else {
            (self.steam / self.capacity).max(0.0)
        }
    }

    /// Two networks joined pool their steam.
    pub fn join(self, other: Pool) -> Pool {
        Pool::new(self.steam + other.steam, self.capacity + other.capacity)
    }

    /// The steam a part of `capacity` holds when the network splits: in
    /// proportion, so every part keeps the pressure.
    pub fn share(&self, capacity: f64) -> f64 {
        if self.capacity <= 0.0 {
            0.0
        } else {
            self.steam * capacity / self.capacity
        }
    }

    /// Take up to `amount` mB; returns what was taken.
    pub fn take(&mut self, amount: f64) -> f64 {
        let got = amount.clamp(0.0, self.steam.max(0.0));
        self.steam -= got;
        got
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_is_steam_over_capacity() {
        assert_eq!(Pool::new(800.0, 800.0).pressure(), 1.0);
        assert_eq!(Pool::new(0.0, 0.0).pressure(), 0.0);
    }

    #[test]
    fn joining_pools_and_splitting_keeps_pressure() {
        let a = Pool::new(1600.0, 400.0); // 4 bar
        let b = Pool::new(0.0, 400.0); // 0 bar
        let j = a.join(b);
        assert_eq!(j.pressure(), 2.0);
        let part = j.share(100.0);
        assert_eq!(part / 100.0, j.pressure());
    }

    #[test]
    fn taking_never_goes_below_zero() {
        let mut p = Pool::new(10.0, 100.0);
        assert_eq!(p.take(25.0), 10.0);
        assert_eq!(p.pressure(), 0.0);
    }
}
