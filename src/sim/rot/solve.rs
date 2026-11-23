//! Integration of one system's speed (rotational network spec 3.4).
//!
//! A system's net torque, referred to its reference speed ω, is
//! `T(ω) = A − B·ω − C·sign(ω)`: `A` the sources' push at standstill, `B` the
//! slope of the sources' lines plus viscous friction, `C` the loads and the
//! constant friction, which always oppose the motion.

/// Acceleration constant `k`: dω/dt (RPM per second) = k · T / I. Set so a
/// 10-shaft crank line reaches 90% of its settled speed in about a second
/// and coasts to a stop in about four (spec 3.4 asks for 2 s and 3 s; with
/// the spec's friction one constant can't give both).
pub const ACCEL: f64 = 40.0;
/// A server tick, in seconds.
pub const DT: f64 = 1.0 / 20.0;
/// With no push, a system slower than this stops.
pub const SNAP_RPM: f64 = 0.5;

/// A system's torque terms for one tick.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Terms {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl Terms {
    /// The net torque at speed `w`.
    pub fn torque(&self, w: f64) -> f64 {
        self.a - self.b * w - self.c * signum0(w)
    }
}

fn signum0(w: f64) -> f64 {
    if w > 0.0 {
        1.0
    } else if w < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// A source's straight line, `τ = S · (1 − ω_d / V)` in its driving direction
/// `dir` (±1), clamped to `[−S, S]`, linearised at the current local speed
/// `w_local`. Returns `(a, b)` such that `τ ≈ a − b · w_local` near `w_local`,
/// in the part's own (positive-axis) terms.
pub fn source_line(stall: f64, top: f64, dir: f64, w_local: f64) -> (f64, f64) {
    if stall <= 0.0 || top <= 0.0 {
        return (0.0, 0.0);
    }
    let wd = dir * w_local;
    if wd < 0.0 {
        // Driven backwards: a brake at full stall torque.
        (dir * stall, 0.0)
    } else if wd > 2.0 * top {
        // Far above its top speed: drags at full torque.
        (-dir * stall, 0.0)
    } else {
        (dir * stall, stall / top)
    }
}

/// One semi-implicit step of `I·dω/dt = k·T(ω)` from `w`, with stiction.
/// `inertia` must be positive.
pub fn step(w: f64, inertia: f64, t: Terms, dt: f64) -> f64 {
    let k = ACCEL * dt;
    // Standing still: it starts only if the push beats the constant terms.
    if w == 0.0 && t.a.abs() <= t.c {
        return 0.0;
    }
    let dir = if w != 0.0 { signum0(w) } else { signum0(t.a) };
    let next = (inertia * w + k * (t.a - t.c * dir)) / (inertia + k * t.b);
    // Carried through zero: friction and loads hold it there.
    if w != 0.0 && signum0(next) != dir {
        return 0.0;
    }
    if w == 0.0 && signum0(next) != dir {
        return 0.0;
    }
    if next.abs() < SNAP_RPM && t.a.abs() <= t.c {
        return 0.0;
    }
    next
}

/// The speed a system settles at: where `T(ω) = 0`, or 0 if it never starts.
pub fn settled(t: Terms) -> f64 {
    if t.a.abs() <= t.c || t.b <= 0.0 {
        return 0.0;
    }
    (t.a - t.c * signum0(t.a)) / t.b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rot::part::*;

    /// A crank line: one crank and `n` wooden shafts, cranked forwards.
    fn crank_line(n: usize, load: f64) -> (f64, Terms) {
        let inertia = 2.0 + n as f64;
        let friction = 0.005 + 0.0025 * n as f64;
        let consts = CONST_FRICTION * (n + 1) as f64 + load;
        let (a, b) = source_line(CRANK_STALL, CRANK_TOP, 1.0, 0.0);
        (
            inertia,
            Terms {
                a,
                b: b + friction,
                c: consts,
            },
        )
    }

    fn run(w0: f64, inertia: f64, t: Terms, ticks: usize) -> Vec<f64> {
        let mut w = w0;
        let mut out = Vec::new();
        for _ in 0..ticks {
            w = step(w, inertia, t, DT);
            out.push(w);
        }
        out
    }

    #[test]
    fn a_crank_line_spins_up_and_settles_on_the_straight_line() {
        let (inertia, t) = crank_line(10, 0.0);
        let goal = settled(t);
        assert!(goal > 20.0 && goal < CRANK_TOP, "settles at {goal}");
        let trace = run(0.0, inertia, t, 200);
        let at_90 = trace.iter().position(|w| *w >= 0.9 * goal).unwrap();
        let seconds = at_90 as f64 * DT;
        assert!((0.8..=2.6).contains(&seconds), "90% after {seconds} s");
        assert!((trace.last().unwrap() - goal).abs() < 0.01);
        // Monotonic: no overshoot from the implicit step.
        assert!(trace.windows(2).all(|w| w[1] >= w[0]));
    }

    #[test]
    fn a_released_line_coasts_to_a_stop() {
        let (inertia, t) = crank_line(10, 0.0);
        let free = Terms { a: 0.0, ..t };
        // Without the crank's line, only friction remains.
        let free = Terms {
            b: free.b - CRANK_STALL / CRANK_TOP,
            ..free
        };
        let trace = run(settled(t), inertia, free, 400);
        let stop = trace.iter().position(|w| *w == 0.0).unwrap();
        let seconds = stop as f64 * DT;
        assert!((2.0..=6.0).contains(&seconds), "stops after {seconds} s");
        assert!(trace[stop..].iter().all(|w| *w == 0.0));
    }

    #[test]
    fn several_sources_add_up() {
        let (inertia, one) = crank_line(4, 0.0);
        let (a, b) = source_line(CRANK_STALL, CRANK_TOP, 1.0, 0.0);
        let two = Terms {
            a: one.a + a,
            b: one.b + b,
            c: one.c,
        };
        // Same top speed, more push: settles a little faster and gets there
        // sooner; the curve's zero is still 32 RPM.
        assert!(settled(two) > settled(one));
        assert!(settled(two) < CRANK_TOP);
        let t1 = run(0.0, inertia, one, 40);
        let t2 = run(0.0, inertia, two, 40);
        assert!(t2[10] > t1[10]);
    }

    #[test]
    fn opposing_sources_brake_each_other() {
        // Two equal cranks pushing opposite ways: nothing moves.
        let (a1, b1) = source_line(CRANK_STALL, CRANK_TOP, 1.0, 0.0);
        let (a2, b2) = source_line(CRANK_STALL, CRANK_TOP, -1.0, 0.0);
        let t = Terms {
            a: a1 + a2,
            b: b1 + b2,
            c: 0.1,
        };
        assert_eq!(step(0.0, 4.0, t, DT), 0.0);
        // A stronger one wins, the other is driven backwards and brakes at stall.
        let (a3, _) = source_line(CRANK_STALL, CRANK_TOP, -1.0, 5.0);
        assert_eq!(a3, -CRANK_STALL);
    }

    #[test]
    fn a_load_above_stall_never_starts() {
        let (inertia, t) = crank_line(2, 20.0);
        assert!(t.a < t.c);
        assert!(run(0.0, inertia, t, 100).iter().all(|w| *w == 0.0));
        assert_eq!(settled(t), 0.0);
    }

    #[test]
    fn a_load_sags_the_speed() {
        let (_, free) = crank_line(10, 0.0);
        let (_, loaded) = crank_line(10, 6.0);
        assert!(settled(loaded) < settled(free) - 5.0);
    }

    #[test]
    fn driven_past_twice_its_top_a_source_drags() {
        let (a, b) = source_line(10.0, 20.0, 1.0, 50.0);
        assert_eq!((a, b), (-10.0, 0.0));
        let (a, b) = source_line(10.0, 20.0, 1.0, 30.0);
        assert_eq!((a, b), (10.0, 0.5));
        // At 30 RPM that line gives -5 Nm: dragging, as it should.
        assert_eq!(a - b * 30.0, -5.0);
    }

    #[test]
    fn the_step_is_stable_however_stiff() {
        let t = Terms {
            a: 1000.0,
            b: 1000.0,
            c: 0.0,
        };
        let trace = run(0.0, 0.01, t, 50);
        assert!(trace.iter().all(|w| w.is_finite() && *w <= 1.0 + 1e-9));
    }
}
