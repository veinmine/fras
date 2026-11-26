//! Plant tests (steam and heat spec 15, fluids spec 12, multiblocks spec 9).

use std::cell::RefCell;
use std::collections::HashSet;

use super::*;
use crate::sim::geom::{Chunk, Dir};
use crate::sim::rot::View;
use crate::sim::steam::{BOILER_WATER, DT, engine};

#[derive(Default)]
struct World {
    unloaded: RefCell<HashSet<Chunk>>,
    unsimulated: RefCell<HashSet<Chunk>>,
}

impl View for World {
    fn loaded(&self, pos: Pos) -> bool {
        !self.unloaded.borrow().contains(&pos.chunk())
    }
    fn simulated(&self, chunk: Chunk) -> bool {
        !self.unsimulated.borrow().contains(&chunk) && !self.unloaded.borrow().contains(&chunk)
    }
}

fn p(x: i32, y: i32, z: i32) -> Pos {
    Pos::new(x, y, z)
}

const COAL: f64 = 1000.0;

/// A 2×2×2 boiler at the origin, full and at the boil.
fn boiler(w: &mut Works, kind: ShellKind) {
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let saved = Saved {
                    water: BOILER_WATER,
                    temp: 100.0,
                    ..Saved::default()
                };
                w.insert(p(x, y, z), Spec::new(Kind::Shell(kind)), Some(saved));
            }
        }
    }
}

fn fireboxes(w: &mut Works, n: usize) {
    let spots = [p(0, -1, 0), p(1, -1, 0), p(0, -1, 1), p(1, -1, 1)];
    for q in spots.iter().take(n) {
        w.insert(*q, Spec::new(Kind::Firebox), None);
        w.set_heat(*q, COAL);
    }
}

fn pipe(w: &mut Works, at: Pos, faces: &[Dir]) {
    w.insert(
        at,
        Spec::new(Kind::Pipe(PipeMat::Iron)).with_faces(faces),
        None,
    );
}

/// Dome at (0,2,0), iron pipe up and east to a cylinder at (3,3,0) facing a
/// flywheel at (5,3,0) turning about Z.
fn engine_plant(w: &mut Works, fires: usize) {
    boiler(w, ShellKind::IronBoiler);
    fireboxes(w, fires);
    w.insert(p(0, 2, 0), Spec::new(Kind::Dome), None);
    pipe(w, p(0, 3, 0), &[Dir::Down, Dir::East]);
    pipe(w, p(1, 3, 0), &[Dir::West, Dir::East]);
    pipe(w, p(2, 3, 0), &[Dir::West, Dir::East]);
    w.insert(
        p(3, 3, 0),
        Spec::new(Kind::Cylinder).with_dir(Dir::East),
        None,
    );
    w.insert(
        p(5, 3, 0),
        Spec::new(Kind::Flywheel).with_dir(Dir::North),
        None,
    );
}

/// The speed a source settles at against `load` (friction ignored): the
/// rotational side reduced to its steady state.
fn settle(d: Drive, load: f64) -> f64 {
    if d.stall <= load || d.top <= 0.0 {
        0.0
    } else {
        d.top * (1.0 - load / d.stall)
    }
}

fn pressure(w: &Works, at: Pos) -> f64 {
    w.gauge(at).bar
}

fn top_up(w: &mut Works) {
    for s in &mut w.shells {
        if s.is_boiler() {
            s.water = s.water_capacity();
        }
    }
}

/// Run the engine plant for `seconds` against `load` Nm at the flywheel,
/// keeping the boiler full; returns (bar, rpm).
fn run_engine(w: &mut Works, world: &World, load: f64, seconds: f64) -> (f64, f64) {
    let fly = p(5, 3, 0);
    let mut rpm = 0.0;
    for _ in 0..(seconds / DT) as usize {
        w.set_rot(
            fly,
            Rot {
                omega: rpm,
                net: Some(0),
                ..Rot::default()
            },
        );
        w.step(world);
        top_up(w);
        let d = w.drives().into_iter().find(|(q, _)| *q == fly).unwrap().1;
        // The flywheel's inertia, roughly: move a fifth of the way per step.
        rpm += (settle(d, load) - rpm) * 0.2;
    }
    (pressure(w, p(0, 0, 0)), rpm)
}

#[test]
fn the_worked_example_settles_within_ten_percent() {
    let world = World::default();
    // Loaded: 24 Nm at the engine, near 4.8 bar and 105 RPM.
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    let (bar, rpm) = run_engine(&mut w, &world, 24.0, 400.0);
    assert!((bar - 4.8).abs() < 0.48, "loaded bar {bar}");
    assert!((rpm - 105.0).abs() < 10.5, "loaded rpm {rpm}");
    // Idle: near 4 bar and 128 RPM.
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    let (bar, rpm) = run_engine(&mut w, &world, 0.0, 400.0);
    assert!((bar - 4.0).abs() < 0.4, "idle bar {bar}");
    assert!((rpm - 128.0).abs() < 12.8, "idle rpm {rpm}");
    // One firebox: near 2.8 bar and 90 RPM.
    let mut w = Works::new();
    engine_plant(&mut w, 1);
    let (bar, rpm) = run_engine(&mut w, &world, 0.0, 600.0);
    assert!((bar - 2.8).abs() < 0.28, "one fire bar {bar}");
    assert!((rpm - 90.0).abs() < 9.0, "one fire rpm {rpm}");
}

#[test]
fn a_stalled_engine_draws_nothing_and_the_pressure_climbs() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    // 1000 Nm: it never turns.
    for _ in 0..50 {
        w.step(&world);
    }
    let e = w.engine_info(p(5, 3, 0)).unwrap();
    assert_eq!(e.demand, 0.0);
    assert!(e.stall > 0.0);
    // 80 mB/s into 815 mB/bar: about 1 bar in 10 s.
    let bar = pressure(&w, p(0, 0, 0));
    assert!((bar - 80.0 * 10.0 / 815.0).abs() < 0.05, "{bar}");
}

#[test]
fn a_thin_pipe_starves_the_engine() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    w.insert(
        p(1, 3, 0),
        Spec::new(Kind::Pipe(PipeMat::Copper)).with_faces(&[Dir::West, Dir::East]),
        None,
    );
    // Pressurise, then spin the engine fast: it wants more than 80 mB/s.
    w.step(&world);
    w.steam[0].pool.steam = 6.0 * w.steam[0].pool.capacity;
    w.set_rot(
        p(5, 3, 0),
        Rot {
            omega: 150.0,
            net: Some(0),
            ..Rot::default()
        },
    );
    w.step(&world);
    let e = w.engine_info(p(5, 3, 0)).unwrap();
    let demand = engine::demand(150.0, pressure(&w, p(0, 0, 0)));
    assert!(e.demand > 80.0);
    assert!((e.fill - 80.0 / demand).abs() < 0.05, "fill {}", e.fill);
}

/// A boiler with an emergency valve on each of `valves` domes, set to 5 bar.
fn safety_plant(w: &mut Works, fires: usize, valves: usize) {
    boiler(w, ShellKind::IronBoiler);
    fireboxes(w, fires);
    for d in [p(0, 2, 0), p(1, 2, 1)].iter().take(valves) {
        w.insert(*d, Spec::new(Kind::Dome), None);
        let mut v = Spec::new(Kind::Vent { emergency: true }).with_dir(Dir::Down);
        v.set = 5.0;
        w.insert(d.offset(Dir::Up), v, None);
    }
}

fn ruptures(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Rupture { .. }))
        .count()
}

#[test]
fn one_safety_valve_holds_one_firebox_but_not_four() {
    let world = World::default();
    let mut w = Works::new();
    safety_plant(&mut w, 1, 1);
    let mut events = Vec::new();
    let mut highest: f64 = 0.0;
    for _ in 0..(300.0 / DT) as usize {
        w.step(&world);
        top_up(&mut w);
        highest = highest.max(pressure(&w, p(0, 0, 0)));
        events.extend(w.take_events());
    }
    assert_eq!(ruptures(&events), 0);
    assert!(highest < 5.2, "{highest}");
    assert!(events.contains(&Event::ValveOpen(p(0, 3, 0))));
    assert!(events.contains(&Event::ValveShut(p(0, 3, 0))));

    let mut w = Works::new();
    safety_plant(&mut w, 4, 1);
    let mut events = Vec::new();
    for _ in 0..(300.0 / DT) as usize {
        w.step(&world);
        top_up(&mut w);
        events.extend(w.take_events());
        if ruptures(&events) > 0 {
            break;
        }
    }
    assert_eq!(ruptures(&events), 1);

    // Two valves on two domes hold four.
    let mut w = Works::new();
    safety_plant(&mut w, 4, 2);
    let mut events = Vec::new();
    for _ in 0..(300.0 / DT) as usize {
        w.step(&world);
        top_up(&mut w);
        events.extend(w.take_events());
    }
    assert_eq!(ruptures(&events), 0);
    assert!(pressure(&w, p(0, 0, 0)) < 5.5);
}

#[test]
fn a_boiler_without_a_dome_is_sealed_and_ruptures() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::CopperBoiler);
    // 2×2×2 is too big for copper: use a 1×1×4 column instead.
    let mut w2 = Works::new();
    for y in 0..4 {
        let saved = Saved {
            water: BOILER_WATER,
            temp: 100.0,
            ..Saved::default()
        };
        w2.insert(
            p(0, y, 0),
            Spec::new(Kind::Shell(ShellKind::CopperBoiler)),
            Some(saved),
        );
    }
    w2.insert(p(0, -1, 0), Spec::new(Kind::Firebox), None);
    w2.set_heat(p(0, -1, 0), COAL);
    w.step(&world);
    assert!(w.refusal(p(0, 0, 0)).is_some());
    let mut events = Vec::new();
    for _ in 0..(200.0 / DT) as usize {
        w2.step(&world);
        events.extend(w2.take_events());
        if ruptures(&events) > 0 {
            break;
        }
    }
    let Some(Event::Rupture { power, dry, .. }) =
        events.iter().find(|e| matches!(e, Event::Rupture { .. }))
    else {
        panic!("no rupture");
    };
    assert!(!dry);
    // Over 4 bar on 4 blocks: about 1.6 to 2.4.
    assert!(*power > 1.5 && *power < 2.5, "{power}");
}

#[test]
fn a_dry_boiler_glows_then_ruptures() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    w.step(&world);
    w.shells[0].water = 0.0;
    let mut events = Vec::new();
    let mut steps = 0;
    while ruptures(&events) == 0 && steps < 1000 {
        w.step(&world);
        events.extend(w.take_events());
        steps += 1;
    }
    // 2000 kW into an 80 kJ/°C shell: 25 °C/s, about 12 s from 100 to 400.
    let secs = steps as f64 * DT;
    assert!((secs - 12.0).abs() < 1.0, "{secs}");
    let Some(Event::Rupture { power, dry, .. }) = events.last() else {
        panic!()
    };
    assert!(*dry);
    assert!(*power >= 1.0);
}

#[test]
fn a_paused_plant_keeps_every_value() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    for _ in 0..100 {
        w.step(&world);
    }
    let before = (
        pressure(&w, p(0, 0, 0)),
        w.shells[0].water,
        w.shells[0].temp,
    );
    // The flywheel's chunk leaves the simulated area: the whole plant pauses.
    world.unsimulated.borrow_mut().insert(p(5, 3, 0).chunk());
    w.insert(p(20, 3, 0), Spec::new(Kind::Flywheel), None); // unrelated
    world.unsimulated.borrow_mut().clear();
    world.unsimulated.borrow_mut().insert(Chunk { x: 0, z: 0 });
    for _ in 0..100 {
        w.step(&world);
    }
    assert!(w.paused(p(0, 0, 0)));
    assert!(w.held_parts().contains(&p(5, 3, 0)));
    let after = (
        pressure(&w, p(0, 0, 0)),
        w.shells[0].water,
        w.shells[0].temp,
    );
    // (The rebuild for the new part sums the steam again: equal to rounding.)
    assert!((before.0 - after.0).abs() < 1e-9);
    assert_eq!((before.1, before.2), (after.1, after.2));
    // A rotational network paused by its own rules pauses the plant.
    world.unsimulated.borrow_mut().clear();
    w.set_rot(
        p(5, 3, 0),
        Rot {
            paused: true,
            net: Some(3),
            ..Rot::default()
        },
    );
    w.step(&world);
    assert!(w.paused(p(0, 0, 0)));
    w.set_rot(p(5, 3, 0), Rot::default());
    w.step(&world);
    assert!(!w.paused(p(0, 0, 0)));
    assert!(w.held_parts().is_empty());
}

#[test]
fn plants_sharing_a_rotational_network_pause_together() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    // A lone pump far away, on the same rotational network as the flywheel.
    let pump = p(100, 0, 0);
    w.insert(pump, Spec::new(Kind::Pump).with_dir(Dir::East), None);
    world.unsimulated.borrow_mut().insert(pump.chunk());
    for q in [p(5, 3, 0), pump] {
        w.set_rot(
            q,
            Rot {
                net: Some(7),
                ..Rot::default()
            },
        );
    }
    w.step(&world);
    assert!(w.paused(p(0, 0, 0)));
}

#[test]
fn save_and_reload_keep_water_temperature_and_pressure() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    for _ in 0..200 {
        w.step(&world);
    }
    let bar = pressure(&w, p(0, 0, 0));
    let water = w.shells[0].water;
    let temp = w.shells[0].temp;
    assert!(bar > 1.0);
    let saved: Vec<(Pos, Spec, Saved)> = w
        .parts
        .iter()
        .map(|(q, part)| (*q, part.spec, w.saved(*q).unwrap()))
        .collect();
    let mut w2 = Works::new();
    for (q, spec, s) in saved {
        w2.insert(q, spec, Some(s));
    }
    w2.rebuild(&world);
    // Pipes keep steam in memory only: the pressure drops by their share.
    let bar2 = pressure(&w2, p(0, 0, 0));
    assert!((bar2 - bar * 800.0 / 815.0).abs() < 1e-6, "{bar} {bar2}");
    assert!((w2.shells[0].water - water).abs() < 1e-6);
    assert!((w2.shells[0].temp - temp).abs() < 1e-9);
}

#[test]
fn a_rebuild_that_finds_the_same_plant_changes_nothing() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    for _ in 0..200 {
        w.step(&world);
    }
    let bar = pressure(&w, p(0, 0, 0));
    // An unrelated part placed and broken again.
    w.insert(p(50, 0, 0), Spec::new(Kind::Firebox), None);
    w.rebuild(&world);
    w.remove(p(50, 0, 0));
    w.rebuild(&world);
    assert!((pressure(&w, p(0, 0, 0)) - bar).abs() < 1e-9);
}

#[test]
fn closing_a_valve_splits_and_opening_joins() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 0);
    let valve = p(1, 3, 0);
    w.insert(
        valve,
        Spec::new(Kind::Valve { redstone: false }).with_dir(Dir::East),
        None,
    );
    w.step(&world);
    w.steam[0].pool.steam = 4.0 * w.steam[0].pool.capacity;
    let mut shut = Spec::new(Kind::Valve { redstone: false }).with_dir(Dir::East);
    shut.opening = 0.0;
    w.update(valve, shut);
    w.step(&world);
    // Both sides keep 4 bar.
    assert!((pressure(&w, p(0, 0, 0)) - 4.0).abs() < 1e-6);
    assert!((pressure(&w, p(2, 3, 0)) - 4.0).abs() < 1e-6);
    assert_ne!(w.steam_net_at(p(0, 0, 0)), w.steam_net_at(p(2, 3, 0)));
    // Vent the far side, then open: the pressure pools.
    let far = w.steam_net_at(p(2, 3, 0)).unwrap();
    w.steam[far].pool.steam = 0.0;
    w.update(
        valve,
        Spec::new(Kind::Valve { redstone: false }).with_dir(Dir::East),
    );
    w.step(&world);
    let expect = 4.0 * 805.0 / 815.0;
    assert!((pressure(&w, p(0, 0, 0)) - expect).abs() < 1e-6);
}

#[test]
fn an_overpressured_copper_pipe_bursts_and_vents() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 0);
    w.insert(
        p(1, 3, 0),
        Spec::new(Kind::Pipe(PipeMat::Copper)).with_faces(&[Dir::West, Dir::East]),
        None,
    );
    w.step(&world);
    // 4.5 bar: over copper's 4, under the boiler's 6. Bursts in 10 s.
    let mut events = Vec::new();
    for i in 0..100 {
        let cap = w.steam[0].pool.capacity;
        w.steam[0].pool.steam = 4.5 * cap;
        w.step(&world);
        events.extend(w.take_events());
        if !events.is_empty() {
            assert_eq!(i, 49);
            break;
        }
    }
    assert_eq!(events, vec![Event::Burst(p(1, 3, 0))]);
    w.step(&world);
    assert!(w.vent_state(p(1, 3, 0)).rate > 0.0);
}

#[test]
fn a_pump_feeds_a_boiler_and_idles_when_it_is_full() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    let pump = p(2, 0, 0);
    // Socket east, outlet west into the boiler.
    w.insert(pump, Spec::new(Kind::Pump).with_dir(Dir::East), None);
    w.set_pump_sources(pump, 4);
    w.set_rot(
        pump,
        Rot {
            omega: 100.0,
            net: Some(1),
            ..Rot::default()
        },
    );
    w.step(&world);
    w.shells[0].water = 4000.0;
    w.step(&world);
    assert!((w.shells[0].water - (4000.0 + 75.0 * DT)).abs() < 1e-6);
    assert_eq!(w.pump_loads(), vec![(pump, 4.0)]);
    // Cold feed cools the boil.
    assert!(w.shells[0].temp < 100.0);
    w.shells[0].water = 8000.0;
    w.step(&world);
    assert_eq!(w.pump_loads(), vec![(pump, 0.0)]);
}

#[test]
fn an_empty_tank_takes_water_and_feeds_a_boiler_through_a_pipe() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    // A tank at (5,0,0), pipe between.
    w.insert(p(5, 0, 0), Spec::new(Kind::Shell(ShellKind::Tank)), None);
    for x in 2..5 {
        pipe(&mut w, p(x, 0, 0), &[Dir::West, Dir::East]);
    }
    // A pump into the tank's far side.
    let pump = p(6, 0, 0);
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
    assert_eq!(w.content_at(p(5, 0, 0)), Some(Content::Water));
    // The boiler is full: the pump fills the tank.
    for _ in 0..10 {
        w.step(&world);
    }
    let tank = w.shell_at(p(5, 0, 0)).unwrap().water;
    assert!(tank > 0.0);
    // Now the boiler draws from the tank.
    let bi = w.shell_of[&p(0, 0, 0)];
    w.shells[bi].water = 0.0;
    w.step(&world);
    assert!(w.shells[bi].water > 0.0);
}

#[test]
fn lone_shell_blocks_and_refused_shapes_do_not_form() {
    let world = World::default();
    let mut w = Works::new();
    // An L of copper boiler blocks.
    for q in [p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)] {
        w.insert(q, Spec::new(Kind::Shell(ShellKind::CopperBoiler)), None);
    }
    w.step(&world);
    assert!(w.shell_at(p(0, 0, 0)).is_none());
    assert!(w.refusal(p(0, 0, 0)).is_some());
    w.insert(
        p(1, 1, 0),
        Spec::new(Kind::Shell(ShellKind::CopperBoiler)),
        None,
    );
    w.step(&world);
    let s = w.shell_at(p(0, 0, 0)).unwrap();
    assert_eq!(s.n(), 4);
    let f = w.take_formations();
    assert_eq!(f.iter().filter(|(_, f)| f.is_some()).count(), 4);
}

#[test]
fn breaking_a_pressurised_boiler_block_puffs_and_unforms() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 2);
    for _ in 0..200 {
        w.step(&world);
    }
    w.take_events();
    w.take_formations();
    w.remove(p(1, 1, 1));
    w.step(&world);
    let ev = w.take_events();
    assert!(ev.contains(&Event::Puff(p(1, 1, 1))));
    assert!(w.shell_at(p(0, 0, 0)).is_none());
    let f = w.take_formations();
    assert_eq!(f.len(), 7);
}

#[test]
fn the_weakest_part_fails_first() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 0);
    w.step(&world);
    let mut events = Vec::new();
    for _ in 0..100 {
        let cap = w.steam[0].pool.capacity;
        w.steam[0].pool.steam = 13.0 * cap; // 1.5×8 is instant; 13 > 12
        w.step(&world);
        events.extend(w.take_events());
        if !events.is_empty() {
            break;
        }
    }
    // The boiler (rated 6) goes first: the weakest part fails first.
    assert_eq!(ruptures(&events), 1);
}

#[test]
fn red_line_is_the_weakest_part() {
    let world = World::default();
    let mut w = Works::new();
    engine_plant(&mut w, 0);
    w.step(&world);
    let i = w.steam_info(p(2, 3, 0)).unwrap();
    assert_eq!(i.red, 6.0);
    assert_eq!(i.red_by, Some(Kind::Shell(ShellKind::IronBoiler)));
    w.insert(
        p(1, 3, 0),
        Spec::new(Kind::Pipe(PipeMat::Copper)).with_faces(&[Dir::West, Dir::East]),
        None,
    );
    w.step(&world);
    assert_eq!(w.steam_info(p(2, 3, 0)).unwrap().red, 4.0);
    let g = w.gauge(p(2, 3, 0));
    assert_eq!(g.red, 4.0);
}

#[test]
fn a_new_valve_reads_the_red_line_once_rebuilt() {
    // The game sets a new emergency valve from the red line of what it sits
    // on; inserting it dissolves the networks, so it must rebuild first.
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    w.insert(p(0, 2, 0), Spec::new(Kind::Dome), None);
    w.step(&world);
    w.insert(
        p(0, 3, 0),
        Spec::new(Kind::Vent { emergency: true }).with_dir(Dir::Down),
        None,
    );
    assert_eq!(w.gauge(p(0, 2, 0)).red, 0.0);
    w.rebuild(&world);
    let red = w.gauge(p(0, 2, 0)).red;
    assert_eq!(red, 6.0);
    assert_eq!(crate::sim::fluid::valve::default_set(Some(red)), 5.5);
}

#[test]
fn fifty_plants_step_and_rebuild_quickly() {
    // Exit criterion 14: 50 boilers, 50 engines and 2000 pipe blocks. Each
    // plant: a 2×2×2 iron boiler, a dome, 40 iron pipes east to a cylinder
    // facing a flywheel, 8 blocks apart along z.
    let world = World::default();
    let mut w = Works::new();
    for k in 0..50 {
        let z0 = k * 8;
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    let saved = Saved {
                        water: BOILER_WATER,
                        temp: 100.0,
                        ..Saved::default()
                    };
                    w.insert(
                        p(x, y, z0 + z),
                        Spec::new(Kind::Shell(ShellKind::IronBoiler)),
                        Some(saved),
                    );
                }
            }
        }
        for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            w.insert(p(x, -1, z0 + z), Spec::new(Kind::Firebox), None);
            w.set_heat(p(x, -1, z0 + z), COAL);
        }
        w.insert(p(0, 2, z0), Spec::new(Kind::Dome), None);
        pipe(&mut w, p(0, 3, z0), &[Dir::Down, Dir::East]);
        for x in 1..40 {
            pipe(&mut w, p(x, 3, z0), &[Dir::West, Dir::East]);
        }
        w.insert(
            p(40, 3, z0),
            Spec::new(Kind::Cylinder).with_dir(Dir::East),
            None,
        );
        w.insert(
            p(42, 3, z0),
            Spec::new(Kind::Flywheel).with_dir(Dir::North),
            None,
        );
    }
    let start = std::time::Instant::now();
    w.rebuild(&world);
    let rebuild = start.elapsed();
    for _ in 0..50 {
        w.step(&world);
    }
    let start = std::time::Instant::now();
    for _ in 0..100 {
        w.step(&world);
    }
    let per_step = start.elapsed() / 100;
    // One steam step every 4 ticks.
    eprintln!(
        "50 plants: step {per_step:?} (per tick {:?}), rebuild {rebuild:?}",
        per_step / 4
    );
    assert_eq!(w.shells.iter().filter(|s| s.is_boiler()).count(), 50);
    // Generous bounds for a debug build; the release targets are 0.2 ms per
    // tick and 2 ms for a rebuild.
    assert!(per_step.as_micros() < 8000, "{per_step:?}");
    assert!(rebuild.as_millis() < 200, "{rebuild:?}");
    // A lone network of 2000 pipe blocks off one dome.
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    w.insert(p(0, 2, 0), Spec::new(Kind::Dome), None);
    pipe(&mut w, p(0, 3, 0), &[Dir::Down, Dir::East]);
    for x in 1..2000 {
        pipe(&mut w, p(x, 3, 0), &[Dir::West, Dir::East]);
    }
    let start = std::time::Instant::now();
    w.rebuild(&world);
    let pipes = start.elapsed();
    eprintln!("2000 pipe blocks: rebuild {pipes:?}");
    assert!(pipes.as_millis() < 200, "{pipes:?}");
}

mod creative;
mod v03;
