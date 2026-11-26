//! 0.3 plant tests: regulation spec 10, cooling spec 13, rust spec 11.

use super::*;
use crate::sim::fluid::{cool, exchange};
use crate::sim::rust::{LEAK_RATE, Stage};
use crate::sim::steam::{HOT_WATER, bellows, damper};

const GOV: Pos = Pos { x: 1, y: 3, z: 0 };
const FLY: Pos = Pos { x: 5, y: 3, z: 0 };
const CYL: Pos = Pos { x: 3, y: 3, z: 0 };

/// The engine plant with a governor in place of the pipe at (1,3,0).
fn governed(w: &mut Works, fires: usize, target: f64, sens: u8) {
    engine_plant(w, fires);
    let mut s = Spec::new(Kind::Governor).with_dir(Dir::East);
    s.target = target;
    s.sens = sens;
    w.insert(GOV, s, None);
}

/// Run a governed engine on the rotational solver (one flywheel and ~10 of
/// line, as the spec's tuning rig), the governor's socket on the flywheel's
/// shaft. Returns the speeds seen, one per step.
fn run_governed(
    w: &mut Works,
    world: &World,
    load: impl Fn(usize) -> f64,
    seconds: f64,
) -> Vec<f64> {
    use crate::sim::rot::part::{CONST_FRICTION, FLYWHEEL_INERTIA};
    use crate::sim::rot::solve::{self, Terms};
    let inertia = FLYWHEEL_INERTIA + 10.0;
    let mut rpm: f64 = 0.0;
    let mut seen = Vec::new();
    for i in 0..(seconds / DT) as usize {
        w.set_rot(
            FLY,
            Rot {
                omega: rpm,
                net: Some(0),
                ..Rot::default()
            },
        );
        w.step(world);
        top_up(w);
        let d = w.drives().into_iter().find(|(q, _)| *q == FLY).unwrap().1;
        for _ in 0..4 {
            w.tick_governor(GOV, rpm, solve::DT);
            let (a, b) = solve::source_line(d.stall, d.top, 1.0, rpm);
            let terms = Terms {
                a,
                b: b + 0.0125,
                c: load(i) + CONST_FRICTION,
            };
            rpm = solve::step(rpm, inertia, terms, solve::DT);
        }
        seen.push(rpm);
    }
    seen
}

fn demand(w: &Works) -> f64 {
    w.engines[w.engine_of[&FLY]].demand.iter().sum()
}

#[test]
fn an_idle_governed_engine_holds_near_its_target_and_saves_steam() {
    let world = World::default();
    let mut free = Works::new();
    engine_plant(&mut free, 2);
    let (_, free_rpm) = run_engine(&mut free, &world, 2.0, 200.0);
    let free_demand = demand(&free);
    assert!(free_rpm > 110.0, "unthrottled idle {free_rpm}");

    let mut w = Works::new();
    governed(&mut w, 2, 64.0, 1);
    // A damper keeps the saved steam from climbing to the red line.
    let mut ds = Spec::new(Kind::Damper).with_dir(Dir::West);
    ds.set = 4.0;
    w.insert(p(-1, 0, 0), ds, None);
    let seen = run_governed(&mut w, &world, |_| 2.0, 200.0);
    let tail = &seen[seen.len() - 100..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    assert!((mean - 64.0).abs() < 16.0, "governed idle {mean}");
    assert!(
        demand(&w) < 0.5 * free_demand,
        "governed {} vs free {free_demand}",
        demand(&w)
    );
    let info = w.governor_info(GOV).unwrap();
    assert_eq!(info.engines, 1);
    assert!(info.on_steam);
    assert!(info.throttle < 0.5);
}

#[test]
fn a_governor_that_does_not_turn_opens_wide() {
    let world = World::default();
    let mut w = Works::new();
    governed(&mut w, 2, 64.0, 1);
    for _ in 0..20 {
        w.step(&world);
    }
    assert_eq!(w.governor_info(GOV).unwrap().throttle, 1.0);
    assert_eq!(w.engine_throttle(FLY), 1.0);
}

#[test]
fn the_throttle_scales_the_engine_stall_torque() {
    let world = World::default();
    let mut w = Works::new();
    governed(&mut w, 2, 64.0, 1);
    run_engine(&mut w, &world, 0.0, 60.0);
    let open = w.engines[0].drive.stall;
    // Balls at 56 RPM: a quarter open.
    w.parts.get_mut(&GOV).unwrap().saved.ball = 56.0;
    w.step(&world);
    let g = w.engine_throttle(FLY);
    assert!((g - 0.25).abs() < 1e-9, "throttle {g}");
    let stall = w.engines[0].drive.stall;
    assert!(
        (stall / open - 0.25).abs() < 0.05,
        "stall {stall} vs {open}"
    );
}

#[test]
fn a_vent_behind_a_governor_is_throttled() {
    let world = World::default();
    let mut w = Works::new();
    governed(&mut w, 4, 64.0, 1);
    // A steam release above the pipe after the governor, held open.
    pipe(&mut w, p(2, 3, 0), &[Dir::West, Dir::East, Dir::Up]);
    w.insert(
        p(2, 4, 0),
        Spec::new(Kind::Vent { emergency: false }).with_dir(Dir::Down),
        None,
    );
    run_engine(&mut w, &world, 0.0, 30.0);
    assert!(w.vent_state(p(2, 4, 0)).rate > 1.0);
    w.parts.get_mut(&GOV).unwrap().saved.ball = 200.0;
    w.step(&world);
    assert_eq!(w.vent_state(p(2, 4, 0)).rate, 0.0);
    assert!(w.governor_info(GOV).unwrap().vent_behind);
}

/// A boiler with a damper on its west face at `set` bar.
fn dampered(w: &mut Works, set: f64) -> Pos {
    boiler(w, ShellKind::IronBoiler);
    fireboxes(w, 4);
    let d = p(-1, 0, 0);
    let mut s = Spec::new(Kind::Damper).with_dir(Dir::West);
    s.set = set;
    w.insert(d, s, None);
    d
}

#[test]
fn a_damper_holds_the_boiler_at_its_set_pressure() {
    let world = World::default();
    let mut w = Works::new();
    let d = dampered(&mut w, 2.0);
    for _ in 0..(600.0 / DT) as usize {
        w.step(&world);
        top_up(&mut w);
    }
    let bar = pressure(&w, p(0, 0, 0));
    assert!(bar > 1.0 && bar < 2.1, "bar {bar}");
    let info = w.damper_info(d).unwrap();
    assert!(info.opening < 0.5, "opening {}", info.opening);
    // The fireboxes burn slower in step.
    let f = w.fire(p(0, -1, 0)).unwrap();
    assert!((f.damper - info.opening).abs() < 1e-9);
    assert!(w.burn_factor(p(0, -1, 0)) < 0.5);
}

#[test]
fn a_powered_damper_shuts_at_its_travel_rate() {
    let world = World::default();
    let mut w = Works::new();
    let d = dampered(&mut w, 8.0);
    w.step(&world);
    assert_eq!(w.damper_info(d).unwrap().opening, 1.0);
    let mut s = w.parts[&d].spec;
    s.powered = true;
    w.update(d, s);
    // 0.25 per second: half shut after 2 s, shut after 4.
    for _ in 0..(2.0 / DT).round() as usize {
        w.step(&world);
    }
    let h = w.damper_info(d).unwrap().opening;
    assert!((h - 0.5).abs() < 0.06, "after 2 s {h}");
    for _ in 0..(2.0 / DT).round() as usize {
        w.step(&world);
    }
    assert!(w.damper_info(d).unwrap().opening < 1e-9);
    assert_eq!(w.shells[0].heat, 0.0);
    assert_eq!(w.burn_factor(p(0, -1, 0)), 0.0);
    assert!(damper::target(0.0, 8.0, true) == 0.0);
}

#[test]
fn bellows_boost_the_heat_and_load_the_shaft() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    fireboxes(&mut w, 1);
    let b = p(-1, -1, 0);
    w.insert(b, Spec::new(Kind::Bellows).with_dir(Dir::East), None);
    for (rpm, boost) in [
        (0.0, 0.0),
        (16.0, 0.125),
        (32.0, 0.25),
        (64.0, 0.5),
        (100.0, 0.5),
    ] {
        w.set_rot(
            b,
            Rot {
                omega: rpm,
                ..Rot::default()
            },
        );
        w.step(&world);
        let f = w.fire(p(0, -1, 0)).unwrap();
        assert!((f.boost - boost).abs() < 1e-9, "{rpm} RPM: {}", f.boost);
        assert!((f.boost - bellows::boost(rpm)).abs() < 1e-9);
        assert!((w.shells[0].heat - COAL * (1.0 + boost)).abs() < 1e-6);
        // The burn rate is the damper's alone: bellows never shorten it.
        assert_eq!(w.burn_factor(p(0, -1, 0)), 1.0);
        let load = w
            .bellows_loads()
            .into_iter()
            .find(|(q, _)| *q == b)
            .unwrap()
            .1;
        assert_eq!(load, if boost > 0.0 { bellows::LOAD } else { 0.0 });
    }
    assert_eq!(w.bellows_target(b), Some(p(0, -1, 0)));
}

/// A bellows counts on any face of a firebox, its mouth included.
#[test]
fn bellows_count_on_any_face_even_the_mouth() {
    let world = World::default();
    for mouth in [Dir::North, Dir::South, Dir::East, Dir::West] {
        for face in [Dir::North, Dir::South, Dir::East, Dir::West] {
            let mut w = Works::new();
            boiler(&mut w, ShellKind::IronBoiler);
            let fire = p(0, -1, 0);
            w.insert(fire, Spec::new(Kind::Firebox).with_dir(mouth), None);
            w.set_heat(fire, COAL);
            let b = fire.offset(face);
            if w.parts.contains_key(&b) {
                continue;
            }
            w.insert(b, Spec::new(Kind::Bellows).with_dir(face.opposite()), None);
            w.step(&world);
            assert_eq!(
                w.bellows_target(b),
                Some(fire),
                "mouth {mouth:?}, face {face:?}"
            );
        }
    }
}

/// A bellows pulls its full load only while its fire burns wide open: less
/// as the dampers close, none when banked.
#[test]
fn bellows_load_follows_the_burn_rate() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    fireboxes(&mut w, 1);
    let fire = p(0, -1, 0);
    let b = p(-1, -1, 0);
    w.insert(b, Spec::new(Kind::Bellows).with_dir(Dir::East), None);
    let d = p(-1, 1, 0);
    let mut ds = Spec::new(Kind::Damper).with_dir(Dir::West);
    ds.powered = true;
    w.insert(d, ds, None);
    let load = |w: &Works| {
        w.bellows_loads()
            .into_iter()
            .find(|(q, _)| *q == b)
            .unwrap()
            .1
    };
    let mut partly = false;
    for _ in 0..30 {
        w.set_rot(
            b,
            Rot {
                omega: 64.0,
                ..Rot::default()
            },
        );
        w.step(&world);
        let burn = w.burn_factor(fire);
        assert!(
            (load(&w) - bellows::LOAD * burn).abs() < 1e-9,
            "burn {burn}"
        );
        partly |= burn > 0.1 && burn < 0.9;
    }
    assert!(partly, "the damper never passed half way");
    assert_eq!(w.burn_factor(fire), 0.0);
    assert_eq!(load(&w), 0.0);
}

#[test]
fn save_and_reload_keep_ball_settings_and_openings() {
    let world = World::default();
    let mut w = Works::new();
    governed(&mut w, 2, 96.0, 3);
    run_governed(&mut w, &world, |_| 2.0, 20.0);
    let d = p(-1, 1, 0);
    let mut ds = Spec::new(Kind::Damper).with_dir(Dir::West);
    ds.set = 1.0;
    w.insert(d, ds, None);
    for _ in 0..40 {
        w.step(&world);
    }
    let gs = w.saved(GOV).unwrap();
    let dsv = w.saved(d).unwrap();
    let mut fresh = Works::new();
    for (q, part) in &w.parts {
        let saved = w.saved(*q);
        fresh.insert(*q, part.spec, saved);
    }
    fresh.step(&world);
    let g = &fresh.parts[&GOV];
    assert_eq!((g.spec.target, g.spec.sens), (96.0, 3));
    assert!((g.saved.ball - gs.ball).abs() < 1e-9);
    assert!((g.saved.ball_v - gs.ball_v).abs() < 1e-9);
    assert!((fresh.parts[&d].saved.opening - dsv.opening).abs() < 0.06);
}

// ---- Cooling -------------------------------------------------------------------

/// The engine plant's exhaust up into an exchanger, its outlet to a drain.
fn condensing(w: &mut Works, rads: usize) -> Pos {
    engine_plant(w, 2);
    pipe(w, p(3, 4, 0), &[Dir::Down, Dir::Up]);
    let x = p(3, 5, 0);
    w.insert(x, Spec::new(Kind::Exchanger).with_dir(Dir::Up), None);
    pipe(w, p(3, 6, 0), &[Dir::Down, Dir::East]);
    pipe(w, p(4, 6, 0), &[Dir::West, Dir::East]);
    w.insert(p(5, 6, 0), Spec::new(Kind::Drain).with_dir(Dir::West), None);
    for d in [Dir::North, Dir::South, Dir::East, Dir::West]
        .iter()
        .take(rads)
    {
        let s = Spec::new(Kind::Radiator { copper: true }).with_dir(*d);
        w.insert(x.offset(*d), s, None);
    }
    x
}

#[test]
fn unpiped_exhaust_vents_at_the_cylinder() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    run_engine(&mut w, &world, 10.0, 60.0);
    let (vented, piped) = w.exhaust_at(CYL).unwrap();
    assert!(vented > 1.0);
    assert_eq!(piped, 0.0);
    assert_eq!(w.exhausts(), vec![CYL]);
}

#[test]
fn an_exchanger_condenses_exhaust_for_the_drain() {
    let world = World::default();
    let mut w = Works::new();
    let x = condensing(&mut w, 0);
    run_engine(&mut w, &world, 10.0, 60.0);
    assert_eq!(w.content_at(p(3, 4, 0)), Some(Content::Exhaust));
    // Exhaust condenses to hot water (a second exchanger would cool it).
    assert_eq!(w.content_at(p(3, 6, 0)), Some(Content::HotWater));
    let e = w.exchanger_info(x).unwrap();
    assert_eq!(e.inlet, Some(Content::Exhaust));
    assert!((e.power - exchange::power(false, &[])).abs() < 1e-9);
    assert!((e.capacity - exchange::capacity(Content::Exhaust, e.power)).abs() < 1e-9);
    assert!(e.took > 0.0 && e.took <= e.capacity + 1e-9);
    let (vented, piped) = w.exhaust_at(CYL).unwrap();
    assert!(piped > 0.0);
    assert!(
        (piped - e.took).abs() < 1e-6,
        "piped {piped} took {}",
        e.took
    );
    let _ = vented;
    let (c, f) = w.drain_flow(p(5, 6, 0)).unwrap();
    assert_eq!(c, Content::HotWater);
    assert!(f > 0.0);
}

#[test]
fn radiators_and_water_add_cooling_power() {
    let world = World::default();
    let mut w = Works::new();
    let x = condensing(&mut w, 2);
    w.step(&world);
    let e = w.exchanger_info(x).unwrap();
    assert_eq!(e.radiators, (2, 0));
    let r = exchange::Radiator::Copper;
    assert!((e.power - exchange::power(false, &[r, r])).abs() < 1e-9);
    let mut s = w.parts[&x].spec;
    s.wet = true;
    w.update(x, s);
    w.step(&world);
    let e = w.exchanger_info(x).unwrap();
    assert!((e.power - exchange::power(true, &[r, r])).abs() < 1e-9);
}

#[test]
fn an_exchanger_without_an_outlet_takes_nothing() {
    let world = World::default();
    let mut w = Works::new();
    let x = condensing(&mut w, 0);
    w.remove(p(3, 6, 0));
    run_engine(&mut w, &world, 10.0, 30.0);
    let e = w.exchanger_info(x).unwrap();
    assert_eq!(e.blocked, Some(ExBlock::NoOutlet));
    assert_eq!(e.took, 0.0);
    let (vented, piped) = w.exhaust_at(CYL).unwrap();
    assert_eq!(piped, 0.0);
    assert!(vented > 0.0);
}

#[test]
fn a_drain_on_exhaust_is_a_refused_mix() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    pipe(&mut w, p(3, 4, 0), &[Dir::Down, Dir::Up]);
    w.insert(p(3, 5, 0), Spec::new(Kind::Drain).with_dir(Dir::Down), None);
    run_engine(&mut w, &world, 10.0, 30.0);
    let ni = w.net_of[&p(3, 4, 0)];
    assert!(w.nets[ni].mixed);
    assert_eq!(w.exhaust_at(CYL).unwrap().1, 0.0);
    assert!(w.drain_flow(p(3, 5, 0)).is_none_or(|(_, f)| f == 0.0));
}

/// A water tank at (5,0,0) piped west into the boiler (the boiler fills
/// from it), holding `content`.
fn tank_feed(w: &mut Works, content: Content) {
    boiler(w, ShellKind::IronBoiler);
    let saved = Saved {
        water: 1000.0,
        content: Some(content),
        ..Saved::default()
    };
    w.insert(
        p(5, 0, 0),
        Spec::new(Kind::Shell(ShellKind::Tank)),
        Some(saved),
    );
    for x in 2..5 {
        pipe(w, p(x, 0, 0), &[Dir::West, Dir::East]);
    }
}

#[test]
fn a_boiler_fed_hot_water_keeps_boiling() {
    let world = World::default();
    let mut w = Works::new();
    tank_feed(&mut w, Content::HotWater);
    fireboxes(&mut w, 2);
    w.step(&world);
    assert_eq!(w.content_at(p(3, 0, 0)), Some(Content::HotWater));
    let bi = w.shell_of[&p(0, 0, 0)];
    w.shells[bi].water *= 0.5;
    w.step(&world);
    assert_eq!(w.shells[bi].feed_temp, HOT_WATER);
    assert!(w.shells[bi].temp >= 99.0, "temp {}", w.shells[bi].temp);
    assert!(w.shells[bi].boiled > 0.0);
}

#[test]
fn hot_feed_comes_before_cold_feed_on_another_face() {
    // The water tank north of the boiler, whose network sorts first, the hot
    // water tank east of it (tank_feed): the boiler still takes hot feed first.
    let world = World::default();
    let mut w = Works::new();
    tank_feed(&mut w, Content::HotWater);
    let cold = Saved {
        water: 1000.0,
        content: Some(Content::Water),
        ..Saved::default()
    };
    w.insert(
        p(0, 0, -4),
        Spec::new(Kind::Shell(ShellKind::Tank)),
        Some(cold),
    );
    for z in -3..0 {
        pipe(&mut w, p(0, 0, z), &[Dir::North, Dir::South]);
    }
    w.step(&world);
    let bi = w.shell_of[&p(0, 0, 0)];
    let cap = w.shells[bi].water_capacity();
    w.shells[bi].water = cap - 20.0;
    let level = |w: &Works, at: Pos| w.shells[w.shell_of[&at]].water;
    let (hot_before, cold_before) = (level(&w, p(5, 0, 0)), level(&w, p(0, 0, -4)));
    w.step(&world);
    let hot_gave = hot_before - level(&w, p(5, 0, 0));
    let cold_gave = cold_before - level(&w, p(0, 0, -4));
    assert!(
        (hot_gave - 20.0).abs() < 1e-6,
        "hot {hot_gave}, cold {cold_gave}"
    );
    assert!(cold_gave.abs() < 1e-6, "hot {hot_gave}, cold {cold_gave}");
}

/// A grinder at (10,0,0), socket east, fed from a water tank on its west and
/// draining hot water north (when `drain`).
fn cooled_grinder(w: &mut Works, water: f64, drain: bool) -> Pos {
    let saved = Saved {
        water,
        content: Some(Content::Water),
        ..Saved::default()
    };
    w.insert(
        p(7, 0, 0),
        Spec::new(Kind::Shell(ShellKind::Tank)),
        Some(saved),
    );
    pipe(w, p(8, 0, 0), &[Dir::West, Dir::East]);
    pipe(w, p(9, 0, 0), &[Dir::West, Dir::East]);
    let g = p(10, 0, 0);
    w.insert(g, Spec::new(Kind::Grinder).with_dir(Dir::East), None);
    pipe(w, p(10, 0, -1), &[Dir::South, Dir::North]);
    if drain {
        w.insert(
            p(10, 0, -2),
            Spec::new(Kind::Drain).with_dir(Dir::South),
            None,
        );
    }
    g
}

#[test]
fn a_grinder_with_water_through_it_is_cooled() {
    let world = World::default();
    let mut w = Works::new();
    let g = cooled_grinder(&mut w, 1000.0, true);
    w.set_cool_demand(g, cool::demand(110.0, true));
    for _ in 0..20 {
        w.step(&world);
    }
    assert_eq!(w.content_at(p(10, 0, -1)), Some(Content::HotWater));
    assert!(w.cooled(g));
    let c = w.cooling_info(g).unwrap();
    assert!((c.received - c.demand).abs() < 1e-6);
    let (content, f) = w.drain_flow(p(10, 0, -2)).unwrap();
    assert_eq!(content, Content::HotWater);
    assert!(f > 0.0);
}

#[test]
fn two_grinders_whose_hot_water_lines_merge_both_stay_cooled() {
    // Grinder A at (10,0,0) with its outlet north, grinder B at (10,0,-4)
    // with its outlet south, each fed from its own tank; the outlets meet at
    // (10,0,-2) and run east into a drain. Routed on one tree, the junction
    // went to one grinder and the other's hot water had nowhere to go.
    let world = World::default();
    let mut w = Works::new();
    let a = cooled_grinder(&mut w, 1000.0, false);
    let saved = Saved {
        water: 1000.0,
        content: Some(Content::Water),
        ..Saved::default()
    };
    w.insert(
        p(7, 0, -4),
        Spec::new(Kind::Shell(ShellKind::Tank)),
        Some(saved),
    );
    pipe(&mut w, p(8, 0, -4), &[Dir::West, Dir::East]);
    pipe(&mut w, p(9, 0, -4), &[Dir::West, Dir::East]);
    let b = p(10, 0, -4);
    w.insert(b, Spec::new(Kind::Grinder).with_dir(Dir::East), None);
    pipe(&mut w, p(10, 0, -2), &[Dir::South, Dir::North, Dir::East]);
    pipe(&mut w, p(10, 0, -3), &[Dir::North, Dir::South]);
    w.insert(
        p(11, 0, -2),
        Spec::new(Kind::Drain).with_dir(Dir::West),
        None,
    );
    w.set_cool_demand(a, 11.0);
    w.set_cool_demand(b, 11.0);
    for _ in 0..((cool::BUFFER / 11.0 / DT) as usize + 20) {
        w.step(&world);
    }
    assert!(w.cooled(a), "a {:?}", w.cooling_info(a));
    assert!(w.cooled(b), "b {:?}", w.cooling_info(b));
    let (_, f) = w.drain_flow(p(11, 0, -2)).unwrap();
    assert!((f - 22.0).abs() < 0.5, "drain {f}");
}

#[test]
fn a_grinder_loses_cooling_with_no_water_or_a_full_buffer() {
    let world = World::default();
    // No water.
    let mut w = Works::new();
    let g = cooled_grinder(&mut w, 0.0, true);
    w.set_cool_demand(g, 11.0);
    w.step(&world);
    assert!(!w.cooled(g));
    // Nowhere for the hot water to go: the buffer fills, then it stops.
    let mut w = Works::new();
    let g = cooled_grinder(&mut w, 1000.0, false);
    w.set_cool_demand(g, 11.0);
    w.step(&world);
    assert!(w.cooled(g));
    for _ in 0..((cool::BUFFER / 11.0 / DT) as usize + 5) {
        w.step(&world);
    }
    assert!(!w.cooled(g));
}

#[test]
fn drains_take_only_what_is_left() {
    let world = World::default();
    // A pump fills the boiler first; only the excess reaches the drain.
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    for x in 2..5 {
        pipe(&mut w, p(x, 0, 0), &[Dir::West, Dir::East, Dir::North]);
    }
    w.insert(
        p(2, 0, -1),
        Spec::new(Kind::Drain).with_dir(Dir::South),
        None,
    );
    let pump = p(5, 0, 0);
    w.insert(pump, Spec::new(Kind::Pump).with_dir(Dir::East), None);
    w.set_pump_sources(pump, 5);
    w.set_rot(
        pump,
        Rot {
            omega: 128.0,
            ..Rot::default()
        },
    );
    w.step(&world);
    let bi = w.shell_of[&p(0, 0, 0)];
    w.shells[bi].water = 0.0;
    w.step(&world);
    let fed = w.shells[bi].fed;
    let drained = w.drain_flow(p(2, 0, -1)).map_or(0.0, |d| d.1);
    let flow = w.pump_flow(pump);
    assert!(fed > 0.0);
    assert!(
        (fed + drained - flow).abs() < 1e-6,
        "fed {fed} drained {drained} flow {flow}"
    );
}

// ---- Rust ----------------------------------------------------------------------

#[test]
fn a_leaking_steam_pipe_vents_ten_millibuckets() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    let q = p(1, 3, 0);
    let mut s = w.parts[&q].spec;
    s.rust = Stage::Leaking;
    w.update(q, s);
    run_engine(&mut w, &world, 10.0, 30.0);
    let (c, f) = w.leak_at(q).unwrap();
    assert_eq!(c, Content::Steam);
    assert!((f - LEAK_RATE).abs() < 1e-6);
    assert_eq!(w.net_leaks(p(0, 0, 0)), (1, f));
}

#[test]
fn leaking_boiler_blocks_vent_from_the_pool() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    for q in [p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)] {
        let mut s = w.parts[&q].spec;
        s.rust = Stage::Leaking;
        w.update(q, s);
    }
    run_engine(&mut w, &world, 10.0, 30.0);
    let (n, total) = w.net_leaks(p(0, 0, 0));
    assert_eq!(n, 3);
    assert!((total - 3.0 * LEAK_RATE).abs() < 1e-6);
}

#[test]
fn a_leaking_water_tank_drips() {
    let world = World::default();
    let mut w = Works::new();
    let saved = Saved {
        water: 100.0,
        content: Some(Content::Water),
        ..Saved::default()
    };
    let mut s = Spec::new(Kind::Shell(ShellKind::Tank));
    s.rust = Stage::Leaking;
    w.insert(p(0, 0, 0), s, Some(saved));
    w.step(&world);
    let t = w.shell_at(p(0, 0, 0)).unwrap();
    assert!((t.water - (100.0 - LEAK_RATE * DT)).abs() < 1e-6);
    assert_eq!(w.leak_at(p(0, 0, 0)).unwrap().0, Content::Water);
}

#[test]
fn a_stuck_damper_keeps_its_opening() {
    let world = World::default();
    let mut w = Works::new();
    let d = dampered(&mut w, 0.0);
    let mut s = w.parts[&d].spec;
    s.rust = Stage::Leaking;
    w.update(d, s);
    for _ in 0..40 {
        w.step(&world);
    }
    let info = w.damper_info(d).unwrap();
    assert!(info.stuck);
    assert_eq!(info.opening, 1.0);
}

#[test]
fn two_exchangers_in_series_give_cold_water() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    pipe(&mut w, p(3, 4, 0), &[Dir::Down, Dir::Up]);
    let a = p(3, 5, 0);
    w.insert(a, Spec::new(Kind::Exchanger).with_dir(Dir::Up), None);
    pipe(&mut w, p(3, 6, 0), &[Dir::Down, Dir::Up]);
    let b = p(3, 7, 0);
    w.insert(b, Spec::new(Kind::Exchanger).with_dir(Dir::Up), None);
    pipe(&mut w, p(3, 8, 0), &[Dir::Down, Dir::Up]);
    w.insert(p(3, 9, 0), Spec::new(Kind::Drain).with_dir(Dir::Down), None);
    run_engine(&mut w, &world, 10.0, 60.0);
    assert_eq!(w.content_at(p(3, 6, 0)), Some(Content::HotWater));
    assert_eq!(w.content_at(p(3, 8, 0)), Some(Content::Water));
    let eb = w.exchanger_info(b).unwrap();
    assert_eq!(eb.inlet, Some(Content::HotWater));
    assert!((eb.capacity - exchange::capacity(Content::HotWater, eb.power)).abs() < 1e-9);
    assert!(eb.took > 0.0);
    assert_eq!(w.drain_flow(p(3, 9, 0)).unwrap().0, Content::Water);
}
