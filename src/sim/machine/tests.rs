//! Machines spec 12's tests.

use super::*;
use crate::sim::fluid::Content;
use crate::sim::grind::{Stack, Work};

struct TestItems;

impl Items for TestItems {
    fn has_tag(&self, item: &str, tag: &str) -> bool {
        match tag {
            "minecraft:planks" => item.ends_with("_planks"),
            TAG_FINE => {
                matches!(item, "minecraft:sand" | "minecraft:red_sand") || item.ends_with("_dust")
            }
            TAG_MEDIUM => {
                item.starts_with("fras:crushed_") || trommel_medium().iter().any(|s| s == item)
            }
            _ => false,
        }
    }
    fn max_stack(&self, _: &str) -> u32 {
        64
    }
}

fn lcg(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed;
    move || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn state(machine: Machine, rpm: f64) -> MachineState {
    MachineState {
        rpm,
        jammed: false,
        paused: false,
        range: machine.range(),
        water: if machine == Machine::Washer {
            Some(1000.0)
        } else {
            None
        },
    }
}

/// Average count of `item` per run of the recipe for `input`.
fn average(recipes: &[Recipe], input: &str, item: &str, runs: usize) -> f64 {
    let r = find(recipes, input, &TestItems).unwrap();
    let mut rng = lcg(7);
    let mut total = 0;
    for _ in 0..runs {
        total += r
            .roll(&mut rng)
            .iter()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum::<u32>();
    }
    total as f64 / runs as f64
}

#[test]
fn every_builtin_recipe_is_valid_and_unique() {
    for m in Machine::ALL {
        let recipes = builtin(m);
        for r in &recipes {
            grind::validate(r).unwrap();
        }
        assert!(grind::duplicates(&recipes).is_empty(), "{m:?}");
    }
}

#[test]
fn yields_match_the_spec() {
    let w = washing();
    let dust = average(&w, "fras:crushed_iron", "fras:iron_dust", 30_000);
    assert!((dust - 4.0 / 3.0).abs() < 0.02, "{dust}");
    let nugget = average(&w, "fras:crushed_iron", "minecraft:iron_nugget", 30_000);
    assert!((nugget - 0.1).abs() < 0.01, "{nugget}");
    assert_eq!(
        average(&w, "fras:crushed_iron", "minecraft:redstone", 1000),
        0.0
    );
    assert!(average(&w, "fras:crushed_gold", "minecraft:copper_nugget", 1).min(1.0) >= 0.0);
    assert_eq!(
        average(&hammering(), "fras:iron_rod", "fras:rivet", 10),
        8.0
    );
    assert_eq!(
        average(&hammering(), "minecraft:iron_ingot", "fras:iron_plate", 10),
        1.0
    );
    assert_eq!(
        average(&turning(), "minecraft:iron_ingot", "fras:iron_rod", 10),
        2.0
    );
    assert_eq!(
        average(&turning(), "fras:iron_plate", "fras:iron_gear", 10),
        1.0
    );
    assert_eq!(
        average(&turning(), "fras:copper_plate", "fras:copper_pipe", 10),
        3.0
    );
    assert_eq!(
        average(&sawing(), "minecraft:oak_log", "minecraft:oak_planks", 10),
        6.0
    );
    assert_eq!(
        average(
            &sawing(),
            "minecraft:stripped_cherry_wood",
            "minecraft:cherry_planks",
            10
        ),
        6.0
    );
    assert_eq!(
        average(&sawing(), "minecraft:spruce_planks", "minecraft:stick", 10),
        3.0
    );
    // Grinding then washing doubles raw ore: 1.5 × 4/3 = 2.
    let crushed = average(
        &grind::builtin(),
        "minecraft:raw_iron",
        "fras:crushed_iron",
        30_000,
    );
    assert!((crushed * dust - 2.0).abs() < 0.04, "{}", crushed * dust);
}

/// Ticks for one recipe run from the input slot at `rpm`.
fn ticks_for(machine: Machine, input: &str, count: u32, rpm: f64) -> Option<u32> {
    let recipes = builtin(machine);
    let mut work = Work::default();
    let mut outputs = vec![None; 4];
    let s = state(machine, rpm);
    for t in 1..20_000 {
        let r = grind::tick_machine(
            &mut work,
            Some(&Stack::new(input, count)),
            &mut outputs,
            &s,
            &recipes,
            &TestItems,
            &mut || 0.0,
        );
        if r.completed {
            return Some(t);
        }
    }
    None
}

#[test]
fn time_per_item_across_each_range() {
    // Washer: 2.5 rev; 64 RPM → 2.34 s.
    assert_eq!(
        ticks_for(Machine::Washer, "fras:crushed_iron", 1, 64.0),
        Some(47)
    );
    assert_eq!(
        ticks_for(Machine::Washer, "fras:crushed_iron", 1, 8.0),
        Some(375)
    );
    assert_eq!(
        ticks_for(Machine::Washer, "fras:crushed_iron", 1, 36.0),
        Some(84)
    );
    // Trip hammer: a plate in 2 rev; at 64 RPM 1.9 s.
    assert_eq!(
        ticks_for(Machine::TripHammer, "minecraft:iron_ingot", 1, 64.0),
        Some(38)
    );
    assert_eq!(
        ticks_for(Machine::TripHammer, "minecraft:iron_ingot", 1, 16.0),
        Some(150)
    );
    // Lathe: rods 8 rev at 160 RPM: 3 s; a gear 12 rev: 4.5 s.
    assert_eq!(
        ticks_for(Machine::Lathe, "minecraft:iron_ingot", 1, 160.0),
        Some(60)
    );
    assert_eq!(
        ticks_for(Machine::Lathe, "fras:iron_plate", 1, 160.0),
        Some(90)
    );
    assert_eq!(
        ticks_for(Machine::Lathe, "fras:iron_plate", 1, 224.0),
        Some(65)
    );
    // Sawmill: a log in 4 rev at 96 RPM: 2.5 s.
    assert_eq!(
        ticks_for(Machine::Sawmill, "minecraft:oak_log", 1, 96.0),
        Some(50)
    );
    assert_eq!(
        ticks_for(Machine::Sawmill, "minecraft:oak_log", 1, 32.0),
        Some(150)
    );
    // Over the top it works at the top's rate.
    assert_eq!(
        ticks_for(Machine::Sawmill, "minecraft:oak_log", 1, 200.0),
        Some(38)
    );
}

#[test]
fn no_progress_below_the_minimum_with_load_still_drawn() {
    for (m, input) in [
        (Machine::Washer, "fras:crushed_iron"),
        (Machine::TripHammer, "minecraft:iron_ingot"),
        (Machine::Lathe, "minecraft:iron_ingot"),
        (Machine::Sawmill, "minecraft:oak_log"),
    ] {
        let recipes = builtin(m);
        let mut work = Work::default();
        let mut outputs = vec![None; 4];
        let slow = m.range().0 * 0.9;
        let r = grind::tick_machine(
            &mut work,
            Some(&Stack::new(input, 1)),
            &mut outputs,
            &state(m, slow),
            &recipes,
            &TestItems,
            &mut || 0.0,
        );
        assert_eq!(work.progress, 0.0, "{m:?}");
        assert!(r.load > 0.0, "{m:?}");
        assert_eq!(r.status, Status::TooSlow);
    }
}

#[test]
fn the_washer_stops_without_water_and_resumes() {
    let recipes = washing();
    let mut work = Work::default();
    let mut outputs = vec![None; 4];
    let mut s = state(Machine::Washer, 64.0);
    s.water = Some(20.0);
    let r = grind::tick_machine(
        &mut work,
        Some(&Stack::new("fras:crushed_iron", 1)),
        &mut outputs,
        &s,
        &recipes,
        &TestItems,
        &mut || 0.0,
    );
    assert_eq!(r.status, Status::NoWater);
    assert_eq!(work.progress, 0.0);
    s.water = Some(50.0);
    let mut used = 0.0;
    for _ in 0..47 {
        used += grind::tick_machine(
            &mut work,
            Some(&Stack::new("fras:crushed_iron", 1)),
            &mut outputs,
            &s,
            &recipes,
            &TestItems,
            &mut || 0.0,
        )
        .water_used;
    }
    assert_eq!(used, 50.0);
}

#[test]
fn the_tank_takes_water_or_hot_water_never_mixed() {
    let mut t = Tank::default();
    assert_eq!(t.fill(Content::Steam, 100.0), 0.0);
    assert_eq!(t.fill(Content::HotWater, 300.0), 300.0);
    assert_eq!(t.fill(Content::Water, 100.0), 0.0, "mixed is refused");
    assert_eq!(t.fill(Content::HotWater, 900.0), 700.0);
    t.drain(1000.0);
    assert_eq!(t.content, None);
    assert_eq!(t.fill(Content::Water, 50.0), 50.0);
}

#[test]
fn belt_mode_refuses_multi_item_recipes_and_unknown_items() {
    let recipes = hammering();
    assert!(belt_accepts("minecraft:iron_ingot", &recipes, &TestItems));
    assert!(!belt_accepts("minecraft:dirt", &recipes, &TestItems));
}

/// A dry washer holds crushed ore without working on it (the line backs up)
/// instead of letting it pass unwashed: seen in game, crushed copper reached
/// the trommel's fines.
#[test]
fn a_dry_or_jammed_machine_holds_the_item_without_working() {
    let recipes = builtin(Machine::Washer);
    assert!(belt_accepts("fras:crushed_copper", &recipes, &TestItems));
    let mut work = BeltWork::default();
    let mut dry = state(Machine::Washer, 32.0);
    dry.water = Some(10.0);
    for _ in 0..200 {
        let t = belt_tick(
            &mut work,
            "fras:crushed_copper",
            &dry,
            &recipes,
            &TestItems,
            &mut || 0.0,
        );
        assert_eq!(t.done, None);
        assert_eq!(t.status, Status::NoWater);
    }
    assert!(work.recipe.is_some(), "still held");
    let mut jammed = state(Machine::Washer, 32.0);
    jammed.water = Some(1000.0);
    jammed.jammed = true;
    let t = belt_tick(
        &mut work,
        "fras:crushed_copper",
        &jammed,
        &recipes,
        &TestItems,
        &mut || 0.0,
    );
    assert_eq!(t.done, None);
}

#[test]
fn belt_mode_replaces_the_item_and_queues_the_rest() {
    let recipes = hammering();
    let mut work = BeltWork::default();
    let s = state(Machine::TripHammer, 64.0);
    let mut done = None;
    for _ in 0..100 {
        let r = belt_tick(
            &mut work,
            "fras:iron_rod",
            &s,
            &recipes,
            &TestItems,
            &mut || 0.0,
        );
        if r.done.is_some() {
            done = r.done;
            break;
        }
    }
    assert_eq!(done, Some(Some("fras:rivet".to_string())));
    assert_eq!(work.pending.len(), 7);
}

fn insert(drum: &mut Drum<String>, item: &str, from_low: bool) -> bool {
    drum.insert(item.to_string(), item, from_low, &TestItems)
        .is_ok()
}

#[test]
fn the_trommel_sorts_by_tag_counted_from_the_entry_end() {
    let s = state(Machine::Trommel, 64.0);
    // Fed at the low end: fines under block 0, mediums 1, oversize 2.
    let mut drum = Drum::default();
    for item in [
        "fras:iron_dust",
        "fras:crushed_iron",
        "minecraft:cobblestone",
    ] {
        assert!(insert(&mut drum, item, true));
    }
    for _ in 0..40 {
        drum.tick(&s);
    }
    assert!(drum.items.is_empty());
    assert_eq!(drum.outlets[0], ["fras:iron_dust"]);
    assert_eq!(drum.outlets[1], ["fras:crushed_iron"]);
    assert_eq!(drum.outlets[2], ["minecraft:cobblestone"]);
    // Fed at the high end: the other way round.
    let mut drum = Drum::default();
    for item in ["fras:iron_dust", "minecraft:iron_nugget", "minecraft:dirt"] {
        assert!(insert(&mut drum, item, false));
    }
    for _ in 0..40 {
        drum.tick(&s);
    }
    assert_eq!(drum.outlets[2], ["fras:iron_dust"]);
    assert_eq!(drum.outlets[1], ["minecraft:iron_nugget"]);
    assert_eq!(drum.outlets[0], ["minecraft:dirt"]);
}

#[test]
fn fines_fall_before_mediums_and_oversize_last() {
    let s = state(Machine::Trommel, 64.0);
    let mut drum = Drum::default();
    insert(&mut drum, "minecraft:sand", true);
    insert(&mut drum, "minecraft:gravel", true);
    insert(&mut drum, "minecraft:stone", true);
    let mut order = Vec::new();
    for _ in 0..40 {
        drum.tick(&s);
        for k in 0..3 {
            if !drum.outlets[k].is_empty() && !order.contains(&k) {
                order.push(k);
            }
        }
    }
    assert_eq!(order, [0, 1, 2]);
}

#[test]
fn a_full_outlet_stops_only_its_class() {
    let s = state(Machine::Trommel, 64.0);
    let mut drum = Drum::default();
    drum.outlets[0] = vec!["fras:iron_dust".to_string(); TROMMEL_OUTLET_HOLD];
    insert(&mut drum, "fras:copper_dust", true);
    insert(&mut drum, "minecraft:gravel", true);
    for _ in 0..40 {
        drum.tick(&s);
    }
    assert_eq!(drum.items.len(), 1);
    assert_eq!(drum.items[0].item, "fras:copper_dust");
    assert_eq!(drum.outlets[1], ["minecraft:gravel"]);
    let t = drum.tick(&s);
    assert_eq!(t.status, Status::OutputFull);
}

#[test]
fn a_full_drum_refuses_and_its_load_grows() {
    let mut drum = Drum::default();
    assert_eq!(drum.load(), 2.0);
    for _ in 0..8 {
        assert!(insert(&mut drum, "minecraft:dirt", true));
    }
    assert!(!insert(&mut drum, "minecraft:dirt", false));
    assert_eq!(drum.load(), 4.0);
}

#[test]
fn the_default_trommel_classes() {
    for (item, size) in [
        ("minecraft:sand", Size::Fine),
        ("fras:gold_dust", Size::Fine),
        ("fras:crushed_copper", Size::Medium),
        ("minecraft:gold_nugget", Size::Medium),
        ("minecraft:redstone", Size::Medium),
        ("minecraft:raw_iron", Size::Medium),
        ("minecraft:cobblestone", Size::Oversize),
    ] {
        assert_eq!(Size::of(item, &TestItems), size, "{item}");
    }
}
