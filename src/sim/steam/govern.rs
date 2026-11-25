//! The centrifugal governor (regulation spec 3): flyballs that follow the
//! speed at its socket with a lag, and the throttle they set.

/// Flyball natural frequency, rad/s, and damping ratio *(tune)*.
pub const WN: f64 = 1.0;
/// 0.6, not the spec's 0.2: with 0.2 sensitivity 1 hunted ±10 RPM in game at
/// target 64 and 5 bar (docs/development.md, "Where 0.3 differs").
pub const ZETA: f64 = 0.6;
/// Target speed range and step, RPM at the socket.
pub const TARGET_MIN: f64 = 8.0;
pub const TARGET_MAX: f64 = 192.0;
pub const TARGET_STEP: f64 = 8.0;
pub const DEFAULT_TARGET: f64 = 64.0;
/// Sensitivity 1 to 5: the proportional band, RPM.
pub const BANDS: [f64; 5] = [32.0, 16.0, 8.0, 4.0, 2.0];
pub const DEFAULT_SENS: u8 = 1;
/// Working range, load while turning, inertia (rotational side).
pub const MAX_RPM: f64 = 192.0;
pub const LOAD: f64 = 1.0;
pub const INERTIA: f64 = 2.0;
/// Seconds of speed history for the hunting warning.
pub const HISTORY_SECONDS: f64 = 10.0;

/// The band of a sensitivity (1 to 5).
pub fn band(sens: u8) -> f64 {
    BANDS[(sens.clamp(1, 5) - 1) as usize]
}

/// The balls' state: where they show (`x`, RPM) and how fast they move.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Balls {
    pub x: f64,
    pub v: f64,
}

impl Balls {
    /// One step of `x'' = ωn²(s − x) − 2ζωn x'` (semi-implicit Euler).
    pub fn step(&mut self, s: f64, dt: f64) {
        self.v += (WN * WN * (s.abs() - self.x) - 2.0 * ZETA * WN * self.v) * dt;
        self.x += self.v * dt;
        if self.x < 0.0 {
            // The balls rest on their stop.
            self.x = 0.0;
            self.v = self.v.max(0.0);
        }
    }

    /// The arms' angle step for clients, 0 (down) to 7 (full out at 192 RPM).
    pub fn step_shown(&self) -> u8 {
        ((self.x / MAX_RPM) * 8.0).floor().clamp(0.0, 7.0) as u8
    }
}

/// The throttle (0 shut to 1 open) for balls at `x`.
pub fn throttle(x: f64, target: f64, band: f64) -> f64 {
    ((target - x) / band.max(1e-9)).clamp(0.0, 1.0)
}

/// The next target, cycling in 8 RPM steps (`up` or down), clamped.
pub fn next_target(target: f64, up: bool) -> f64 {
    let t = snap(target) + if up { TARGET_STEP } else { -TARGET_STEP };
    t.clamp(TARGET_MIN, TARGET_MAX)
}

fn snap(t: f64) -> f64 {
    ((t / TARGET_STEP).round() * TARGET_STEP).clamp(TARGET_MIN, TARGET_MAX)
}

/// Clamp a saved target onto the grid.
pub fn clean_target(t: f64) -> f64 {
    if t.is_finite() && t > 0.0 {
        snap(t)
    } else {
        DEFAULT_TARGET
    }
}

/// The throttle's lever step for clients, 0 (shut) to 3 (open).
pub fn lever(g: f64) -> u8 {
    (g * 3.0).round().clamp(0.0, 3.0) as u8
}

/// A governor's speed history (one sample per steam step) for the hunting
/// warning: the swing over the last 10 s.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    samples: Vec<f64>,
    next: usize,
}

impl History {
    pub fn push(&mut self, s: f64, cap: usize) {
        if self.samples.len() < cap {
            self.samples.push(s);
        } else if cap > 0 {
            self.samples[self.next % cap] = s;
            self.next = (self.next + 1) % cap;
        }
    }

    /// Max minus min over the window.
    pub fn swing(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let lo = self.samples.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = self
            .samples
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        hi - lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rot::part::{CONST_FRICTION, FLYWHEEL_INERTIA};
    use crate::sim::rot::solve::{self, Terms};
    use crate::sim::steam::{STEP_TICKS, engine};

    /// The spec's tuning rig (3.5): one cylinder at 4 bar against `load` at
    /// the engine, `flywheels` flywheels plus ~10 of line, target 90, the
    /// rotational solver's step every tick and the throttle every 4 ticks.
    /// Returns (time, speed) samples.
    fn rig(sens: u8, flywheels: f64, load: impl Fn(f64) -> f64, seconds: f64) -> Vec<(f64, f64)> {
        rig_at(4.0, 90.0, sens, flywheels, load, seconds)
    }

    /// The rig at another pressure and target.
    fn rig_at(
        p: f64,
        target: f64,
        sens: u8,
        flywheels: f64,
        load: impl Fn(f64) -> f64,
        seconds: f64,
    ) -> Vec<(f64, f64)> {
        let inertia = FLYWHEEL_INERTIA * flywheels + 10.0;
        let mut w: f64 = 0.0;
        let mut balls = Balls::default();
        let mut g = 1.0;
        let mut out = Vec::new();
        let dt = solve::DT;
        for n in 0..(seconds / dt) as usize {
            let t = n as f64 * dt;
            balls.step(w, dt);
            if (n as u64).is_multiple_of(STEP_TICKS) {
                g = throttle(balls.x, target, band(sens));
            }
            let stall = engine::stall(p, 1.0) * g;
            let top = engine::top(p);
            let (a, b) = solve::source_line(stall, top, 1.0, w);
            let terms = Terms {
                a,
                b: b + 0.0125,
                c: load(t) + CONST_FRICTION,
            };
            w = solve::step(w, inertia, terms, dt);
            out.push((t, w));
        }
        out
    }

    fn span(trace: &[(f64, f64)], t0: f64, t1: f64) -> (f64, f64) {
        let ws: Vec<f64> = trace
            .iter()
            .filter(|(t, _)| *t >= t0 && *t < t1)
            .map(|(_, w)| *w)
            .collect();
        (
            ws.iter().copied().fold(f64::INFINITY, f64::min),
            ws.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Spec 3.5's table as retuned with `ZETA` 0.6, within ±2 RPM:
    /// (sensitivity, flywheels, low, high). At the spec's rig hunting now
    /// starts at sensitivity 4 with one flywheel, and a second steadies it.
    #[test]
    fn the_tuning_table() {
        let table = [
            (1, 1.0, 71.0, 71.0),
            (1, 3.0, 71.0, 71.0),
            (2, 1.0, 79.0, 79.0),
            (3, 1.0, 84.0, 84.0),
            (4, 1.0, 84.0, 88.5),
            (4, 2.0, 86.7, 86.7),
            (5, 1.0, 85.0, 89.5),
            (5, 2.0, 87.0, 89.0),
        ];
        for (sens, fw, lo, hi) in table {
            let trace = rig(sens, fw, |_| 16.0, 300.0);
            let (a, b) = span(&trace, 250.0, 300.0);
            assert!(
                (a - lo).abs() <= 2.0 && (b - hi).abs() <= 2.0,
                "sensitivity {sens}, {fw} flywheels: {a:.1} to {b:.1}, spec {lo} to {hi}"
            );
            // Steady rows swing under a few RPM, hunting rows more.
            let hunting = hi - lo > 2.0;
            assert_eq!(b - a > 2.0, hunting, "sensitivity {sens}, {fw} flywheels");
        }
    }

    #[test]
    fn a_load_drop_overshoots_less_with_a_second_flywheel() {
        let drop = |t: f64| if t < 200.0 { 16.0 } else { 0.0 };
        let one = span(&rig(4, 1.0, drop, 230.0), 200.0, 230.0).1;
        let two = span(&rig(4, 2.0, drop, 230.0), 200.0, 230.0).1;
        assert!((one - 96.0).abs() <= 2.0, "{one}");
        assert!(two < one - 2.0, "{one} vs {two}");
    }

    /// The default never hunts: sensitivity 1 on one flywheel is steady from
    /// 2 to 6 bar, targets 32 to 128 and loads up to 24 Nm (it hunted ±10 RPM
    /// in game at target 64 and 5 bar with the spec's damping of 0.2).
    #[test]
    fn sensitivity_one_is_steady_everywhere() {
        for p in [2.0, 4.0, 5.5, 6.0] {
            for target in [32.0, 64.0, 90.0, 128.0] {
                for load in [0.0, 8.0, 16.0, 24.0] {
                    let (a, b) = span(&rig_at(p, target, 1, 1.0, |_| load, 300.0), 250.0, 300.0);
                    assert!(
                        b - a < 1.0,
                        "{p} bar, target {target}, {load} Nm: {a:.1} to {b:.1}"
                    );
                }
            }
        }
        // Sensitivity 4 still hunts there, and a second flywheel calms it.
        let one = span(&rig_at(5.5, 64.0, 4, 1.0, |_| 16.0, 300.0), 250.0, 300.0);
        let two = span(&rig_at(5.5, 64.0, 4, 2.0, |_| 16.0, 300.0), 250.0, 300.0);
        assert!(one.1 - one.0 > 10.0, "{one:?}");
        assert!(two.1 - two.0 < 0.6 * (one.1 - one.0), "{one:?} vs {two:?}");
    }

    #[test]
    fn sensitivity_one_droops_under_load() {
        let loaded = span(&rig(1, 1.0, |_| 16.0, 200.0), 150.0, 200.0).0;
        let idle = span(&rig(1, 1.0, |_| 0.0, 200.0), 150.0, 200.0).0;
        assert!(idle - loaded > 10.0, "{idle} vs {loaded}");
        assert!(idle <= 90.0 + 0.5);
    }

    #[test]
    fn throttle_and_settings() {
        assert_eq!(throttle(50.0, 64.0, 32.0), 14.0 / 32.0);
        assert_eq!(throttle(70.0, 64.0, 32.0), 0.0);
        assert_eq!(throttle(0.0, 64.0, 2.0), 1.0);
        assert_eq!(next_target(64.0, true), 72.0);
        assert_eq!(next_target(8.0, false), 8.0);
        assert_eq!(next_target(192.0, true), 192.0);
        assert_eq!(band(1), 32.0);
        assert_eq!(band(9), 2.0);
        assert_eq!(clean_target(f64::NAN), DEFAULT_TARGET);
        // Balls not turning fall to zero: the throttle opens wide.
        let mut b = Balls { x: 90.0, v: 0.0 };
        for _ in 0..400 {
            b.step(0.0, 0.05);
        }
        assert!(b.x < 5.0, "{}", b.x);
        assert_eq!(throttle(b.x, 64.0, 32.0), 1.0);
        let mut h = History::default();
        for s in [80.0, 90.0, 85.0] {
            h.push(s, 2);
        }
        assert_eq!(h.swing(), 5.0);
    }
}
