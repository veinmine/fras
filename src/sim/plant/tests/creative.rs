//! Creative parts: sources and drains.

use super::*;
use crate::sim::steam::HOT_WATER;

fn source(w: &mut Works, at: Pos, fluid: Content, rate: f64, bar: f64) {
    let mut s = Spec::new(Kind::Source);
    s.fluid = Some(fluid);
    s.amount = rate;
    s.set = bar;
    w.insert(at, s, None);
}

/// A creative steam source in place of the boiler: pipe east to the engine.
fn creative_engine(w: &mut Works, rate: f64, bar: f64) {
    source(w, p(0, 3, -1), Content::Steam, rate, bar);
    pipe(w, p(0, 3, 0), &[Dir::North, Dir::East]);
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

fn spin(w: &mut Works, world: &World, rpm: f64, seconds: f64) {
    for _ in 0..(seconds / DT) as usize {
        w.set_rot(
            p(5, 3, 0),
            Rot {
                omega: rpm,
                net: Some(0),
                ..Rot::default()
            },
        );
        w.step(world);
    }
}

#[test]
fn a_steam_source_holds_its_pressure_under_load() {
    let world = World::default();
    let mut w = Works::new();
    creative_engine(&mut w, 1000.0, 4.0);
    spin(&mut w, &world, 0.0, 5.0);
    assert!((pressure(&w, p(1, 3, 0)) - 4.0).abs() < 1e-6);
    // Running fast, the engine draws steam; the source keeps up.
    spin(&mut w, &world, 100.0, 5.0);
    assert!((pressure(&w, p(1, 3, 0)) - 4.0).abs() < 0.01);
    let gave = w.source_gave(p(0, 3, -1));
    let drawn: f64 = w.engines[0].drawn.iter().sum();
    assert!(gave > 1.0 && (gave - drawn).abs() < 1.0, "{gave} {drawn}");
    let d = w
        .drives()
        .into_iter()
        .find(|(q, _)| *q == p(5, 3, 0))
        .unwrap()
        .1;
    assert!(d.stall > 0.0);
}

#[test]
fn a_slow_steam_source_sags() {
    let world = World::default();
    let mut w = Works::new();
    creative_engine(&mut w, 5.0, 4.0);
    spin(&mut w, &world, 0.0, 10.0);
    spin(&mut w, &world, 120.0, 20.0);
    assert!(pressure(&w, p(1, 3, 0)) < 3.0);
    assert!(w.source_gave(p(0, 3, -1)) <= 5.0 + 1e-9);
}

#[test]
fn a_steam_source_straight_on_a_cylinder_keeps_a_pressure() {
    let world = World::default();
    let mut w = Works::new();
    source(&mut w, p(2, 3, 0), Content::Steam, 500.0, 3.0);
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
    spin(&mut w, &world, 0.0, 2.0);
    let d = w
        .drives()
        .into_iter()
        .find(|(q, _)| *q == p(5, 3, 0))
        .unwrap()
        .1;
    assert!(d.stall > 0.0, "{d:?}");
}

#[test]
fn a_water_source_feeds_a_boiler_and_a_creative_drain_takes_the_rest() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    for s in &mut w.parts.values_mut() {
        s.saved.water = 0.0;
    }
    source(&mut w, p(-2, 0, 0), Content::Water, 100.0, 0.0);
    pipe(&mut w, p(-1, 0, 0), &[Dir::West, Dir::East, Dir::Down]);
    w.insert(p(-1, -1, 0), Spec::new(Kind::Void), None);
    w.step(&world);
    w.step(&world);
    let water = w.shells[0].water;
    assert!(water > 0.0, "{water}");
    // A full boiler takes nothing: the drain takes it all.
    top_up(&mut w);
    w.step(&world);
    let (c, f) = w.void_flow(p(-1, -1, 0)).unwrap();
    assert_eq!(c, Content::Water);
    assert!((f - 100.0).abs() < 1e-6, "{f}");
    assert!((w.source_gave(p(-2, 0, 0)) - 100.0).abs() < 1e-6);
}

#[test]
fn a_hot_water_source_feeds_hot() {
    let world = World::default();
    let mut w = Works::new();
    boiler(&mut w, ShellKind::IronBoiler);
    for s in &mut w.parts.values_mut() {
        s.saved.water = 0.0;
        s.saved.temp = 20.0;
    }
    source(&mut w, p(-1, 0, 0), Content::HotWater, 200.0, 0.0);
    w.step(&world);
    w.step(&world);
    assert_eq!(w.shells[0].feed_temp, HOT_WATER);
    // Changing the rate needs no rebuild; changing what it gives does.
    let mut s = w.part(p(-1, 0, 0)).unwrap().spec;
    s.amount = 50.0;
    let before = w.rebuilds;
    w.update(p(-1, 0, 0), s);
    w.step(&world);
    assert_eq!(w.rebuilds, before);
    s.fluid = Some(Content::Water);
    w.update(p(-1, 0, 0), s);
    w.step(&world);
    assert_eq!(w.rebuilds, before + 1);
}

#[test]
fn a_creative_drain_takes_exhaust_but_never_steam() {
    let world = World::default();
    let mut w = Works::new();
    creative_engine(&mut w, 1000.0, 4.0);
    // On the cylinder's top: exhaust.
    w.insert(p(3, 4, 0), Spec::new(Kind::Void), None);
    // Beside the steam pipe: no link.
    w.insert(p(1, 4, 0), Spec::new(Kind::Void), None);
    spin(&mut w, &world, 0.0, 2.0);
    spin(&mut w, &world, 100.0, 3.0);
    let (c, f) = w.void_flow(p(3, 4, 0)).unwrap();
    assert_eq!(c, Content::Exhaust);
    assert!(f > 1.0);
    assert_eq!(w.void_flow(p(1, 4, 0)), None);
    assert!((pressure(&w, p(1, 3, 0)) - 4.0).abs() < 0.01);
}

#[test]
fn a_creative_firebox_may_ignore_a_shut_damper() {
    let world = World::default();
    for free in [false, true] {
        let mut w = Works::new();
        boiler(&mut w, ShellKind::IronBoiler);
        w.insert(p(0, 2, 0), Spec::new(Kind::Dome), None);
        let mut f = Spec::new(Kind::Firebox);
        f.free = free;
        w.insert(p(0, -1, 0), f, None);
        w.set_heat(p(0, -1, 0), 1000.0);
        let mut d = Spec::new(Kind::Damper).with_dir(Dir::West);
        d.powered = true;
        w.insert(p(-1, 0, 0), d, None);
        for _ in 0..100 {
            w.step(&world);
            top_up(&mut w);
        }
        let heat = w.shells[0].heat;
        if free {
            assert!((heat - 1000.0).abs() < 1e-6, "{heat}");
        } else {
            assert!(heat < 1.0, "{heat}");
        }
        // Changing it needs no rebuild.
        let before = w.rebuilds;
        let mut f2 = w.part(p(0, -1, 0)).unwrap().spec;
        f2.free = !free;
        w.update(p(0, -1, 0), f2);
        w.step(&world);
        assert_eq!(w.rebuilds, before);
    }
}
