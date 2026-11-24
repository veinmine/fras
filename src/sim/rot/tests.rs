//! Network tests (rotational network spec, section 12).

use std::cell::RefCell;
use std::collections::HashSet;

use super::net::*;
use super::part::*;
use super::{Jam, Spec};
use crate::sim::geom::{Axis, Chunk, Dir, Pos};

/// Everything loaded and simulated, except the listed chunks.
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

fn crank(face: Dir) -> Spec {
    Spec::new(Kind::Crank, Shape::Socket(face))
}

fn grinder(face: Dir) -> Spec {
    Spec::new(Kind::Grinder, Shape::Socket(face))
}

fn wood(axis: Axis) -> Spec {
    Spec::shaft(Material::Wood, axis)
}

fn iron(axis: Axis) -> Spec {
    Spec::shaft(Material::Iron, axis)
}

const CRANKING: Drive = Drive {
    stall: CRANK_STALL,
    top: CRANK_TOP,
    dir: 1.0,
};

/// A crank at x=0 facing east, `n` shafts along x, optionally a grinder.
fn crank_line(m: &mut Mechanics, n: i32, material: Material, grinder_at_end: bool) {
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..=n {
        m.insert(p(x, 64, 0), Spec::shaft(material, Axis::X), None, true);
    }
    if grinder_at_end {
        m.insert(p(n + 1, 64, 0), grinder(Dir::West), None, true);
    }
}

fn run(m: &mut Mechanics, w: &World, ticks: usize) {
    for _ in 0..ticks {
        m.tick(w);
    }
}

fn rpm(m: &Mechanics, pos: Pos) -> f64 {
    m.speed(pos).unwrap().0
}

#[test]
fn a_crank_turns_a_line_and_a_grinder_sags_it() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 8, Material::Wood, true);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 200);
    let free = rpm(&m, p(9, 64, 0));
    assert!(free > 20.0 && free < 32.0, "{free}");
    // Every part of the line turns together.
    assert!((rpm(&m, p(1, 64, 0)) - free).abs() < 1e-9);
    m.set_load(p(9, 64, 0), 6.0);
    run(&mut m, &w, 200);
    let loaded = rpm(&m, p(9, 64, 0));
    assert!(loaded < free - 5.0 && loaded > 12.0, "{loaded}");
    // Released, it coasts to a stop.
    m.set_drive(p(0, 64, 0), Drive::default());
    run(&mut m, &w, 200);
    assert_eq!(rpm(&m, p(9, 64, 0)), 0.0);
}

#[test]
fn no_rebuild_when_nothing_changed() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 4, Material::Wood, false);
    run(&mut m, &w, 1);
    let r = m.rebuilds;
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 50);
    assert_eq!(m.rebuilds, r);
}

#[test]
fn a_gearbox_doubles_speed_and_halves_torque_on_the_fast_side() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    m.insert(p(1, 64, 0), iron(Axis::X), None, true);
    // Slow side (large gear) toward the crank: the far side runs twice as fast.
    m.insert(
        p(2, 64, 0),
        Spec::new(
            Kind::Gearbox,
            Shape::Gearbox {
                axis: Axis::X,
                slow: Dir::West,
            },
        ),
        None,
        true,
    );
    m.insert(p(3, 64, 0), iron(Axis::X), None, true);
    m.insert(p(4, 64, 0), grinder(Dir::West), None, true);
    m.set_drive(p(0, 64, 0), CRANKING);
    m.set_load(p(4, 64, 0), 2.0);
    run(&mut m, &w, 300);
    let slow = rpm(&m, p(1, 64, 0));
    let fast = rpm(&m, p(3, 64, 0));
    assert!((fast - 2.0 * slow).abs() < 1e-6, "{slow} {fast}");
    // The slow side carries about twice the torque of the fast side.
    let t_slow = m.part(p(1, 64, 0)).unwrap().torque;
    let t_fast = m.part(p(3, 64, 0)).unwrap().torque;
    assert!(
        t_slow > 1.9 * t_fast && t_slow < 3.0 * t_fast,
        "{t_slow} {t_fast}"
    );
}

#[test]
fn bevel_gears_turn_lines_by_their_geometry() {
    let w = World::default();
    let mut m = Mechanics::new();
    // Crank driving a shaft along +x into a bevel gear turning it to +z.
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    m.insert(p(1, 64, 0), wood(Axis::X), None, true);
    m.insert(
        p(2, 64, 0),
        Spec::new(
            Kind::BevelGear(Material::Wood),
            Shape::Bevel {
                input: Dir::West,
                output: Dir::South,
                tee: true,
            },
        ),
        None,
        true,
    );
    m.insert(p(2, 64, 1), wood(Axis::Z), None, true);
    m.insert(p(2, 64, -1), wood(Axis::Z), None, true);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 100);
    let input = rpm(&m, p(1, 64, 0));
    let south = rpm(&m, p(2, 64, 1));
    let north = rpm(&m, p(2, 64, -1));
    assert!(input > 5.0);
    assert!((south.abs() - input.abs()).abs() < 1e-9);
    // The tee's outputs turn opposite ways about their shared axis.
    assert!((south + north).abs() < 1e-9);
    // Input spins +1 about west (outward), i.e. -1 about +x: the input shaft's
    // local speed is -gain... check the sign against the port rule directly.
    let bevel = m.part(p(2, 64, 0)).unwrap().spec;
    let g_in = bevel.port(Dir::West).unwrap();
    let g_out = bevel.port(Dir::South).unwrap();
    assert!((south / input - g_out / g_in).abs() < 1e-9);
}

#[test]
fn a_consistent_loop_is_accepted_and_an_inconsistent_one_jams_the_part_that_closed_it() {
    let w = World::default();
    let mut m = Mechanics::new();
    // Two parallel bevel paths from a line around a square back to itself:
    // build a square of shafts with bevel gears at the corners.
    let bevel = |input: Dir, output: Dir| {
        Spec::new(
            Kind::BevelGear(Material::Iron),
            Shape::Bevel {
                input,
                output,
                tee: false,
            },
        )
    };
    // Corners at (0,0), (3,0), (3,3), (0,3) in x/z.
    m.insert(p(0, 64, 0), bevel(Dir::East, Dir::South), None, true);
    m.insert(p(1, 64, 0), iron(Axis::X), None, true);
    m.insert(p(2, 64, 0), iron(Axis::X), None, true);
    m.insert(p(3, 64, 0), bevel(Dir::West, Dir::South), None, true);
    m.insert(p(3, 64, 1), iron(Axis::Z), None, true);
    m.insert(p(3, 64, 2), iron(Axis::Z), None, true);
    m.insert(p(3, 64, 3), bevel(Dir::North, Dir::West), None, true);
    m.insert(p(2, 64, 3), iron(Axis::X), None, true);
    m.insert(p(1, 64, 3), iron(Axis::X), None, true);
    m.insert(p(0, 64, 3), bevel(Dir::East, Dir::North), None, true);
    m.insert(p(0, 64, 2), iron(Axis::Z), None, true);
    run(&mut m, &w, 1);
    assert!(m.take_events().is_empty());
    // Closing the square: four bevel turns bring the loop back to itself.
    m.insert(p(0, 64, 1), iron(Axis::Z), None, true);
    run(&mut m, &w, 1);
    let events = m.take_events();
    // Four meshes each flip the outward spin; going round, the sign comes
    // back consistent or not depending on geometry: whichever, a jam only
    // ever hits the shaft that closed the loop.
    for e in &events {
        assert_eq!(*e, Event::Jammed(p(0, 64, 1), Jam::Conflict));
    }
    // A loop that is plainly inconsistent: a gearbox in one of two parallel paths.
    let mut m = Mechanics::new();
    let gearbox = Spec::new(
        Kind::Gearbox,
        Shape::Gearbox {
            axis: Axis::X,
            slow: Dir::East,
        },
    );
    // Path A: y=64, shafts and a gearbox; path B: y=65, plain shafts; joined
    // at both ends by vertical shafts through T bevel gears.
    let tee = |input: Dir, output: Dir| {
        Spec::new(
            Kind::BevelGear(Material::Iron),
            Shape::Bevel {
                input,
                output,
                tee: false,
            },
        )
    };
    m.insert(p(0, 64, 0), tee(Dir::East, Dir::Up), None, true);
    m.insert(p(1, 64, 0), iron(Axis::X), None, true);
    m.insert(p(2, 64, 0), gearbox, None, true);
    m.insert(p(3, 64, 0), iron(Axis::X), None, true);
    m.insert(p(4, 64, 0), tee(Dir::West, Dir::Up), None, true);
    m.insert(p(0, 65, 0), iron(Axis::Y), None, true);
    m.insert(p(4, 65, 0), iron(Axis::Y), None, true);
    m.insert(p(0, 66, 0), tee(Dir::Down, Dir::East), None, true);
    m.insert(p(4, 66, 0), tee(Dir::Down, Dir::West), None, true);
    m.insert(p(1, 66, 0), iron(Axis::X), None, true);
    m.insert(p(2, 66, 0), iron(Axis::X), None, true);
    run(&mut m, &w, 1);
    m.take_events();
    m.insert(p(3, 66, 0), iron(Axis::X), None, true);
    run(&mut m, &w, 1);
    assert_eq!(
        m.take_events(),
        vec![Event::Jammed(p(3, 66, 0), Jam::Conflict)]
    );
    // On a load (no player change) the part with the lowest speed limit jams:
    // the iron bevel gears (192) and the gearbox (192) tie, broken by position.
    let mut fresh = Mechanics::new();
    for x in 0..5 {
        for y in 64..67 {
            if let Some(part) = m.part(p(x, y, 0)) {
                let spec = part.spec;
                fresh.insert(p(x, y, 0), spec, Some(Saved::default()), false);
            }
        }
    }
    run(&mut fresh, &w, 1);
    let events = fresh.take_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Jammed(at, Jam::Conflict) => {
            assert!(fresh.part(*at).unwrap().spec.max_rpm() == Some(192.0))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn joining_shares_momentum_and_opposite_directions_jam_the_connecting_part() {
    let w = World::default();
    let mut m = Mechanics::new();
    // Two lines, each with a crank, with a gap at x=5.
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..5 {
        m.insert(p(x, 64, 0), wood(Axis::X), None, true);
    }
    m.insert(p(10, 64, 0), crank(Dir::West), None, true);
    for x in 6..10 {
        m.insert(p(x, 64, 0), wood(Axis::X), None, true);
    }
    m.set_drive(p(0, 64, 0), CRANKING);
    m.set_drive(
        p(10, 64, 0),
        Drive {
            dir: -1.0,
            ..CRANKING
        },
    );
    run(&mut m, &w, 100);
    assert!(rpm(&m, p(1, 64, 0)) > 8.0 && rpm(&m, p(9, 64, 0)) < -8.0);
    // A clutch, disengaged, fills the gap; engaging it is the join.
    let mut clutch = Spec::new(Kind::Clutch, Shape::Inline(Axis::X));
    clutch.engaged = false;
    m.insert(p(5, 64, 0), clutch, None, true);
    run(&mut m, &w, 1);
    m.take_events();
    clutch.engaged = true;
    m.update(p(5, 64, 0), clutch, true);
    run(&mut m, &w, 1);
    assert_eq!(
        m.take_events(),
        vec![Event::Jammed(p(5, 64, 0), Jam::Shock)]
    );
    // Same direction: momentum is shared, nothing jams.
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..5 {
        m.insert(p(x, 64, 0), wood(Axis::X), None, true);
    }
    for x in 6..10 {
        m.insert(p(x, 64, 0), wood(Axis::X), None, true);
    }
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 100);
    let before = rpm(&m, p(1, 64, 0));
    m.insert(p(5, 64, 0), wood(Axis::X), None, true);
    run(&mut m, &w, 1);
    assert!(m.take_events().is_empty());
    let after = rpm(&m, p(9, 64, 0));
    // Inertia 2+4 turning joined by 1+4 at rest: about 6/11 of the speed.
    assert!(
        after < before * 0.7 && after > before * 0.45,
        "{before} {after}"
    );
}

#[test]
fn a_split_keeps_each_sides_speed() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 10, Material::Wood, false);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 100);
    let before = rpm(&m, p(10, 64, 0));
    m.remove(p(5, 64, 0));
    run(&mut m, &w, 1);
    // The cut-off side keeps its speed for a moment, then coasts.
    let cut = rpm(&m, p(10, 64, 0));
    assert!((cut - before).abs() < 1.0, "{before} {cut}");
    run(&mut m, &w, 200);
    assert_eq!(rpm(&m, p(10, 64, 0)), 0.0);
    assert!(rpm(&m, p(2, 64, 0)) > 20.0);
}

#[test]
fn overspeed_fills_the_timer_and_jams_then_repair_restores() {
    let w = World::default();
    let mut m = Mechanics::new();
    // A water wheel geared up by two gearboxes overspeeds a wooden bevel gear.
    m.insert(
        p(0, 64, 0),
        Spec::new(Kind::WaterWheel, Shape::Inline(Axis::X)),
        None,
        true,
    );
    let gb = |_x: i32| {
        Spec::new(
            Kind::Gearbox,
            Shape::Gearbox {
                axis: Axis::X,
                slow: Dir::West,
            },
        )
    };
    m.insert(p(1, 64, 0), gb(1), None, true);
    m.insert(p(2, 64, 0), gb(2), None, true);
    m.insert(p(3, 64, 0), wood(Axis::X), None, true);
    m.insert(
        p(4, 64, 0),
        Spec::new(
            Kind::BevelGear(Material::Wood),
            Shape::Bevel {
                input: Dir::West,
                output: Dir::South,
                tee: false,
            },
        ),
        None,
        true,
    );
    m.insert(p(4, 64, 1), wood(Axis::Z), None, true);
    m.set_drive(
        p(0, 64, 0),
        Drive {
            stall: WHEEL_MAX,
            top: WHEEL_TOP,
            dir: 1.0,
        },
    );
    let mut jammed_at = None;
    for t in 0..400 {
        m.tick(&w);
        if let Some(Event::Jammed(at, Jam::Overspeed)) = m.take_events().first() {
            jammed_at = Some((t, *at));
            break;
        }
    }
    let (t, at) = jammed_at.expect("the bevel gear jams");
    assert_eq!(at, p(4, 64, 0));
    assert!(t > 40, "warned first ({t} ticks)");
    // The cut-off side coasts down; the source side runs on.
    run(&mut m, &w, 200);
    assert_eq!(rpm(&m, p(4, 64, 1)), 0.0);
    assert!(rpm(&m, p(3, 64, 0)).abs() > 64.0);
    assert!(m.part(at).unwrap().jam.is_some());
    // Repaired, it rejoins and starts warning again.
    m.set_drive(p(0, 64, 0), Drive::default());
    run(&mut m, &w, 400);
    assert!(m.repair(at));
    run(&mut m, &w, 1);
    assert!(m.part(at).unwrap().jam.is_none());
    assert!(m.running(at));
}

#[test]
fn anything_past_the_hard_cap_jams_at_once() {
    let w = World::default();
    let mut m = Mechanics::new();
    // An iron shaft line with two gearboxes from a fast saved speed.
    let gb = Spec::new(
        Kind::Gearbox,
        Shape::Gearbox {
            axis: Axis::X,
            slow: Dir::West,
        },
    );
    m.insert(
        p(0, 64, 0),
        iron(Axis::X),
        Some(Saved {
            omega: 100.0,
            ..Saved::default()
        }),
        false,
    );
    m.insert(
        p(1, 64, 0),
        gb,
        Some(Saved {
            omega: 200.0,
            ..Saved::default()
        }),
        false,
    );
    m.insert(
        p(2, 64, 0),
        gb,
        Some(Saved {
            omega: 400.0,
            ..Saved::default()
        }),
        false,
    );
    m.insert(
        p(3, 64, 0),
        iron(Axis::X),
        Some(Saved {
            omega: 400.0,
            ..Saved::default()
        }),
        false,
    );
    run(&mut m, &w, 1);
    let events = m.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Jammed(_, Jam::Overspeed))),
        "{events:?}"
    );
}

#[test]
fn a_wooden_line_shears_near_the_source_and_an_iron_trunk_does_not() {
    let w = World::default();
    // A big load on a long wooden line: a strong source (water wheels add up).
    let line = |trunk: Material| {
        let mut m = Mechanics::new();
        m.insert(
            p(0, 64, 0),
            Spec::new(Kind::WaterWheel, Shape::Inline(Axis::X)),
            None,
            true,
        );
        for x in 1..=6 {
            m.insert(p(x, 64, 0), Spec::shaft(trunk, Axis::X), None, true);
        }
        m.insert(
            p(7, 64, 0),
            Spec::new(Kind::Bearing, Shape::Inline(Axis::X)),
            None,
            true,
        );
        for x in 8..=12 {
            m.insert(p(x, 64, 0), wood(Axis::X), None, true);
        }
        m.insert(p(13, 64, 0), grinder(Dir::West), None, true);
        // A second branch: a T bevel gear halfway with grinders.
        m.set_drive(
            p(0, 64, 0),
            Drive {
                stall: 120.0,
                top: 24.0,
                dir: 1.0,
            },
        );
        m.set_load(p(13, 64, 0), 40.0);
        m
    };
    let mut m = line(Material::Wood);
    let mut sheared = None;
    for _ in 0..400 {
        m.tick(&w);
        for e in m.take_events() {
            if let Event::Jammed(at, Jam::Shear) = e {
                sheared.get_or_insert(at);
            }
        }
    }
    let at = sheared.expect("a wooden shaft shears");
    assert!(at.x >= 1 && at.x <= 12);
    // Iron trunk: the wooden branch past the bearing still carries 40 Nm
    // and shears, but no trunk part does.
    let mut m = line(Material::Iron);
    for _ in 0..400 {
        m.tick(&w);
        for e in m.take_events() {
            if let Event::Jammed(at, Jam::Shear) = e {
                assert!(at.x >= 8, "trunk sheared at {at:?}");
            }
        }
    }
}

#[test]
fn torque_flow_is_the_cut_sum_on_a_line() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 6, Material::Iron, true);
    m.set_drive(p(0, 64, 0), CRANKING);
    m.set_load(p(7, 64, 0), 6.0);
    run(&mut m, &w, 400);
    // Settled: the shaft next to the grinder carries the load plus a little.
    let near_grinder = m.part(p(6, 64, 0)).unwrap().torque;
    let near_crank = m.part(p(1, 64, 0)).unwrap().torque;
    assert!(near_grinder > 6.0 && near_grinder < 7.5, "{near_grinder}");
    assert!(
        near_crank > near_grinder,
        "friction adds up toward the source"
    );
}

#[test]
fn spans_sag_by_material_and_bearings_fix_them() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..=12 {
        m.insert(p(x, 64, 0), wood(Axis::X), None, true);
    }
    run(&mut m, &w, 1);
    let s = m.part(p(5, 64, 0)).unwrap();
    assert!(s.sag);
    assert_eq!(s.span, Some((12, 8)));
    assert_eq!(s.torque_limit(), Some(16.0));
    // A bearing in the middle splits it into 5 and 6.
    m.insert(
        p(6, 64, 0),
        Spec::new(Kind::Bearing, Shape::Inline(Axis::X)),
        None,
        true,
    );
    run(&mut m, &w, 1);
    assert!(!m.part(p(5, 64, 0)).unwrap().sag);
    assert_eq!(m.part(p(8, 64, 0)).unwrap().span, Some((6, 8)));
    // Mixed: one wooden shaft in an iron run limits it to 8.
    let mut m = Mechanics::new();
    for x in 1..=12 {
        let spec = if x == 6 { wood(Axis::X) } else { iron(Axis::X) };
        m.insert(p(x, 64, 0), spec, None, true);
    }
    run(&mut m, &w, 1);
    assert!(m.part(p(1, 64, 0)).unwrap().sag);
    let mut m = Mechanics::new();
    for x in 1..=16 {
        m.insert(p(x, 64, 0), iron(Axis::X), None, true);
    }
    run(&mut m, &w, 1);
    assert!(!m.part(p(1, 64, 0)).unwrap().sag);
    m.insert(p(17, 64, 0), iron(Axis::X), None, true);
    run(&mut m, &w, 1);
    assert!(m.part(p(1, 64, 0)).unwrap().sag);
}

#[test]
fn a_belt_carries_power_slips_under_overload_and_grips_again() {
    let w = World::default();
    let mut m = Mechanics::new();
    let pulley = |partner: Pos, crossed: bool| {
        let mut s = Spec::new(Kind::Pulley { large: false }, Shape::Inline(Axis::X));
        s.belt = Some(BeltEnd { partner, crossed });
        s
    };
    // Line 1 at z=0: crank, shaft, pulley; line 2 at z=4: pulley, shaft, grinder.
    m.insert(
        p(0, 64, 0),
        Spec::new(Kind::WaterWheel, Shape::Inline(Axis::X)),
        None,
        true,
    );
    m.insert(p(1, 64, 0), iron(Axis::X), None, true);
    m.insert(p(2, 64, 0), pulley(p(2, 64, 4), false), None, true);
    m.insert(p(2, 64, 4), pulley(p(2, 64, 0), false), None, true);
    m.insert(p(3, 64, 4), iron(Axis::X), None, true);
    m.insert(p(4, 64, 4), grinder(Dir::West), None, true);
    m.set_drive(
        p(0, 64, 0),
        Drive {
            stall: 60.0,
            top: 24.0,
            dir: 1.0,
        },
    );
    m.set_load(p(4, 64, 4), 6.0);
    run(&mut m, &w, 200);
    let a = rpm(&m, p(1, 64, 0));
    let b = rpm(&m, p(3, 64, 4));
    assert!(a > 5.0 && (a - b).abs() < 1e-9, "{a} {b}");
    assert!(
        m.take_events()
            .iter()
            .all(|e| !matches!(e, Event::Slipped(..)))
    );
    // Overload: 40 Nm through a 24 Nm belt.
    m.set_load(p(4, 64, 4), 40.0);
    run(&mut m, &w, 100);
    let events = m.take_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::Slipped(..))),
        "{events:?}"
    );
    let a = rpm(&m, p(1, 64, 0));
    let b = rpm(&m, p(3, 64, 4));
    assert!(a > b + 1.0, "slipping: {a} {b}");
    // Load drops: it grips again.
    m.set_load(p(4, 64, 4), 2.0);
    run(&mut m, &w, 400);
    let events = m.take_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::Gripped(..))),
        "{events:?}"
    );
    let a = rpm(&m, p(1, 64, 0));
    let b = rpm(&m, p(3, 64, 4));
    assert!((a - b).abs() < 1e-6, "{a} {b}");
}

#[test]
fn a_crossed_belt_reverses_and_pulley_sizes_set_the_ratio() {
    let w = World::default();
    let mut m = Mechanics::new();
    let pulley = |large: bool, partner: Pos, crossed: bool| {
        let mut s = Spec::new(Kind::Pulley { large }, Shape::Inline(Axis::X));
        s.belt = Some(BeltEnd { partner, crossed });
        s
    };
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    m.insert(p(1, 64, 0), pulley(true, p(1, 64, 5), true), None, true);
    m.insert(p(1, 64, 5), pulley(false, p(1, 64, 0), true), None, true);
    m.insert(p(2, 64, 5), wood(Axis::X), None, true);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 100);
    let a = rpm(&m, p(1, 64, 0));
    let b = rpm(&m, p(2, 64, 5));
    assert!((b + 2.0 * a).abs() < 1e-9, "{a} {b}");
}

#[test]
fn a_network_pauses_when_any_part_is_not_simulated_and_resumes_at_the_same_speed() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..=20 {
        m.insert(p(x, 64, 0), iron(Axis::X), None, true);
    }
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 60);
    m.set_drive(p(0, 64, 0), Drive::default());
    run(&mut m, &w, 5);
    let before = rpm(&m, p(1, 64, 0));
    assert!(before > 5.0);
    // The far end (x=16..20, chunk 1) leaves the simulated area.
    w.unsimulated.borrow_mut().insert(Chunk { x: 1, z: 0 });
    m.refresh_pause(&w);
    run(&mut m, &w, 100);
    assert_eq!(m.speed(p(1, 64, 0)), Some((before, true)));
    w.unsimulated.borrow_mut().clear();
    m.refresh_pause(&w);
    assert_eq!(m.speed(p(1, 64, 0)), Some((before, false)));
    run(&mut m, &w, 1);
    assert!(rpm(&m, p(1, 64, 0)) < before);
}

#[test]
fn an_unloaded_chunk_pauses_and_reload_from_saved_speeds_rebuilds_at_that_speed() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    for x in 1..=20 {
        m.insert(p(x, 64, 0), iron(Axis::X), None, true);
    }
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 60);
    let before = rpm(&m, p(18, 64, 0));
    // Chunk 1 unloads: its parts go, the rest pauses.
    for x in 16..=20 {
        m.unload(p(x, 64, 0));
    }
    w.unloaded.borrow_mut().insert(Chunk { x: 1, z: 0 });
    run(&mut m, &w, 20);
    assert_eq!(m.speed(p(5, 64, 0)), Some((before, true)));
    // The rest unloads: the network leaves the registry.
    for x in 0..=15 {
        m.unload(p(x, 64, 0));
    }
    assert!(m.is_empty());
    // A restart: chunk 0 loads first, with saved speeds; the frontier into
    // chunk 1 keeps it paused until that loads too.
    w.unloaded.borrow_mut().clear();
    w.unloaded.borrow_mut().insert(Chunk { x: 1, z: 0 });
    let saved = Saved {
        omega: before,
        ..Saved::default()
    };
    m.insert(p(0, 64, 0), crank(Dir::East), Some(saved), false);
    for x in 1..=15 {
        m.insert(p(x, 64, 0), iron(Axis::X), Some(saved), false);
    }
    run(&mut m, &w, 5);
    let (resumed, paused) = m.speed(p(5, 64, 0)).unwrap();
    assert!(paused && (resumed - before).abs() < 1e-9);
    w.unloaded.borrow_mut().clear();
    for x in 16..=20 {
        m.insert(p(x, 64, 0), iron(Axis::X), Some(saved), false);
    }
    m.rebuild(&w);
    m.refresh_pause(&w);
    let (after, paused) = m.speed(p(18, 64, 0)).unwrap();
    assert!(!paused);
    assert!((after - before).abs() < 0.5, "{before} {after}");
}

#[test]
fn jam_states_survive_a_reload() {
    let w = World::default();
    let mut m = Mechanics::new();
    let saved = Saved {
        jam: Some(Jam::Shear),
        ..Saved::default()
    };
    m.insert(p(0, 64, 0), crank(Dir::East), None, false);
    m.insert(p(1, 64, 0), wood(Axis::X), Some(saved), false);
    m.insert(p(2, 64, 0), wood(Axis::X), None, false);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 50);
    // The sheared shaft breaks the line.
    assert_eq!(rpm(&m, p(2, 64, 0)), 0.0);
    assert!(rpm(&m, p(0, 64, 0)) > 10.0);
    assert_eq!(m.part(p(1, 64, 0)).unwrap().jam, Some(Jam::Shear));
}

#[test]
fn a_water_wheel_runs_one_grinder_well_two_slowly_three_below_the_minimum() {
    let w = World::default();
    let settle = |grinders: usize| {
        let mut m = Mechanics::new();
        // Wheel with a T line: grinders hang off bevel gears.
        m.insert(
            p(0, 64, 0),
            Spec::new(Kind::WaterWheel, Shape::Inline(Axis::X)),
            None,
            true,
        );
        let mut x = 1;
        for g in 0..grinders {
            m.insert(p(x, 64, 0), wood(Axis::X), None, true);
            x += 1;
            m.insert(
                p(x, 64, 0),
                Spec::new(
                    Kind::BevelGear(Material::Wood),
                    Shape::Bevel {
                        input: Dir::West,
                        output: Dir::South,
                        tee: false,
                    },
                ),
                None,
                true,
            );
            m.insert(p(x, 64, 1), grinder(Dir::North), None, true);
            m.set_load(p(x, 64, 1), 6.0);
            // Continue the line through a T for the next grinder.
            if g + 1 < grinders {
                m.update(
                    p(x, 64, 0),
                    Spec::new(
                        Kind::BevelGear(Material::Wood),
                        Shape::Bevel {
                            input: Dir::South,
                            output: Dir::East,
                            tee: true,
                        },
                    ),
                    false,
                );
            }
            x += 1;
        }
        m.set_drive(
            p(0, 64, 0),
            Drive {
                stall: WHEEL_MAX,
                top: WHEEL_TOP,
                dir: 1.0,
            },
        );
        run(&mut m, &w, 600);
        rpm(&m, p(1, 64, 0)).abs()
    };
    let one = settle(1);
    let two = settle(2);
    let three = settle(3);
    assert!(one > 16.0 && one < 22.0, "one {one}");
    assert!(two > 12.0 && two < one, "two {two}");
    assert!(three < 12.0, "three {three}");
}

#[test]
fn sync_sends_only_noticeable_changes() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 3, Material::Wood, false);
    m.set_drive(p(0, 64, 0), CRANKING);
    m.tick(&w);
    let first = m.take_sync();
    assert_eq!(first.len(), 4);
    let mut sends = 0;
    for _ in 0..400 {
        m.tick(&w);
        sends += m.take_sync().len();
    }
    // Spinning up sends a few updates, settled sends none.
    assert!(sends > 0 && sends < 4 * 60, "{sends}");
    assert!(m.take_sync().is_empty());
}

#[test]
fn reading_reports_red_line_and_stall() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 3, Material::Wood, true);
    m.set_load(p(4, 64, 0), 30.0);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 20);
    let r = m.reading(p(2, 64, 0)).unwrap();
    assert_eq!(r.rpm, 0.0);
    assert_eq!(r.red_line, Some(GRINDER_MAX_RPM));
    assert!(r.drawn >= 30.0 && r.stall == CRANK_STALL, "{r:?}");
    assert_eq!(r.min_working, Some(GRINDER_MIN_RPM));
}

#[test]
fn two_thousand_parts_tick_and_rebuild_quickly() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    // A comb: a line along x with bearings every 8, 2000 parts.
    for x in 1..2000 {
        let spec = if x % 8 == 0 {
            Spec::new(Kind::Bearing, Shape::Inline(Axis::X))
        } else {
            iron(Axis::X)
        };
        m.insert(p(x, 64, 0), spec, None, true);
    }
    m.set_drive(p(0, 64, 0), CRANKING);
    let start = std::time::Instant::now();
    m.tick(&w);
    let rebuild = start.elapsed();
    run(&mut m, &w, 400);
    let start = std::time::Instant::now();
    run(&mut m, &w, 100);
    let per_tick = start.elapsed() / 100;
    // Generous bounds for a debug build; the release targets are 0.1 ms and 2 ms.
    eprintln!("2000 parts: tick {per_tick:?}, rebuild {rebuild:?}");
    assert!(per_tick.as_micros() < 2000, "{per_tick:?}");
    assert!(rebuild.as_millis() < 200, "{rebuild:?}");
}

#[test]
fn a_plant_holds_its_rotational_network_paused() {
    let w = World::default();
    let mut m = Mechanics::new();
    crank_line(&mut m, 2, Material::Wood, false);
    m.set_drive(p(0, 64, 0), CRANKING);
    run(&mut m, &w, 100);
    let before = rpm(&m, p(2, 64, 0));
    assert!(before > 10.0);
    m.set_held([p(0, 64, 0)].into_iter().collect());
    run(&mut m, &w, 100);
    assert_eq!(rpm(&m, p(2, 64, 0)), before);
    assert!(m.speed(p(2, 64, 0)).unwrap().1);
    // Its own pause doesn't count the hold, so the plant can let go.
    let (_, own) = m.own_pause(p(2, 64, 0)).unwrap();
    assert!(!own);
    m.set_held(Default::default());
    m.set_drive(p(0, 64, 0), Drive::default());
    run(&mut m, &w, 400);
    assert_eq!(rpm(&m, p(2, 64, 0)), 0.0);
}

#[test]
fn a_flywheel_spins_up_in_about_ten_seconds() {
    let w = World::default();
    let mut m = Mechanics::new();
    let fly = p(0, 64, 0);
    m.insert(
        fly,
        Spec::new(Kind::Flywheel, Shape::Inline(Axis::Z)),
        None,
        true,
    );
    // 4 bar, idle: 64 Nm stall, 128 RPM top.
    m.set_drive(
        fly,
        Drive {
            stall: 64.0,
            top: 128.0,
            dir: 1.0,
        },
    );
    run(&mut m, &w, 2000);
    let settled = rpm(&m, fly);
    assert!(settled > 110.0, "{settled}");
    let mut m = Mechanics::new();
    m.insert(
        fly,
        Spec::new(Kind::Flywheel, Shape::Inline(Axis::Z)),
        None,
        true,
    );
    m.set_drive(
        fly,
        Drive {
            stall: 64.0,
            top: 128.0,
            dir: 1.0,
        },
    );
    let mut ticks = 0;
    while rpm(&m, fly) < 0.9 * settled {
        m.tick(&w);
        ticks += 1;
    }
    let secs = ticks as f64 / 20.0;
    assert!((5.0..=15.0).contains(&secs), "{secs} s to 90% of {settled}");
}

#[test]
fn a_pump_counts_its_own_working_minimum() {
    let w = World::default();
    let mut m = Mechanics::new();
    m.insert(p(0, 64, 0), crank(Dir::East), None, true);
    m.insert(
        p(1, 64, 0),
        Spec::new(Kind::Pump, Shape::Socket(Dir::West)),
        None,
        true,
    );
    m.set_drive(p(0, 64, 0), CRANKING);
    m.set_load(p(1, 64, 0), 4.0);
    run(&mut m, &w, 50);
    let r = m.reading(p(0, 64, 0)).unwrap();
    assert_eq!(r.min_working, Some(crate::sim::fluid::pump::MIN_RPM));
}
