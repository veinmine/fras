#!/usr/bin/env python3
"""Governor tuning sketch for specs/regulation.md, section 3.5.

One steam cylinder at a fixed pressure drives a line through a flyball governor
(section 3.3), with the 0.2 engine curve (steam and heat spec 9.1) and the
rotational solver's semi-implicit step (rotational spec 3.4, ACCEL 40). Prints
the settled speed range per sensitivity and flywheel count, and the overshoot
when the load drops. Not used by the build; run it by hand when retuning.
"""

ACCEL, DT, STEP_TICKS = 40.0, 0.05, 4
STALL_PER_BAR, TOP_PER_BAR = 16.0, 32.0
FLYWHEEL, LINE = 90.0, 10.0  # inertia: the shipped flywheel, and the rest of a line
WN, ZETA = 1.0, 0.2  # flyball natural frequency (rad/s) and damping
BANDS = [32, 16, 8, 4, 2]  # sensitivity 1..5


def run(band, flywheels, load, seconds, target=90.0, pressure=4.0):
    inertia = FLYWHEEL * flywheels + LINE
    w = x = v = 0.0
    g = 1.0
    out = []
    for n in range(int(seconds / DT)):
        t = n * DT
        v += (WN * WN * (w - x) - 2 * ZETA * WN * v) * DT
        x += v * DT
        if n % STEP_TICKS == 0:
            g = max(0.0, min(1.0, (target - x) / band))
        s = STALL_PER_BAR * pressure * g
        a, b, c = s, s / (TOP_PER_BAR * pressure) + 0.0125, load(t) + 0.15
        w = max(0.0, (inertia * w + ACCEL * DT * (a - c)) / (inertia + ACCEL * DT * b))
        out.append((t, w))
    return out


def span(trace, t0, t1):
    ws = [w for t, w in trace if t0 <= t < t1]
    return min(ws), max(ws)


if __name__ == "__main__":
    print("sensitivity band flywheels  loaded (RPM)   peak after load drop")
    for i, band in enumerate(BANDS, 1):
        for fw in (1, 2, 3):
            steady = run(band, fw, lambda t: 16.0, 300)
            lo, hi = span(steady, 250, 300)
            drop = run(band, fw, lambda t: 16.0 if t < 200 else 0.0, 230)
            _, peak = span(drop, 200, 230)
            print(f"{i:>11} {band:>4} {fw:>9}  {lo:5.1f} to {hi:5.1f}   {peak:5.1f}")
