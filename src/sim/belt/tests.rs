//! Logistics spec 12's tests for lines, elevators and diverters.

use super::*;

/// A world with no stations whose end takes items while `open`.
struct End {
    open: bool,
    got: Vec<String>,
}

impl Stations<String> for End {
    fn arrive(&mut self, _: usize, _: &String) -> Action {
        Action::Pass
    }
    fn poll(&mut self, _: usize, _: &mut String) -> bool {
        false
    }
    fn deliver(&mut self, item: &String) -> bool {
        if self.open {
            self.got.push(item.clone());
        }
        self.open
    }
}

fn flat(n: usize) -> Line<String> {
    let blocks: Vec<Pos> = (0..n as i32).map(|i| Pos::new(i, 0, 0)).collect();
    Line::new(blocks, Dir::East, vec![Slope::Flat; n])
}

fn spaced_ok(line: &Line<String>) {
    for w in line.entries.windows(2) {
        assert!(w[1].x - w[0].x >= SPACING - 1e-9, "{:?}", line.snapshot());
    }
}

#[test]
fn speed_follows_rpm() {
    assert_eq!(blocks_per_second(32.0), 1.0);
    assert_eq!(blocks_per_second(-64.0), 2.0);
    assert_eq!(blocks_per_second(128.0), 4.0);
    assert!((blocks_per_tick(64.0) - 0.1).abs() < 1e-12);
}

#[test]
fn drives_needed_is_one_per_sixteen() {
    assert_eq!(drives_needed(1), 1);
    assert_eq!(drives_needed(16), 1);
    assert_eq!(drives_needed(17), 2);
    assert_eq!(drives_needed(32), 2);
    assert_eq!(drives_needed(64), 4);
}

#[test]
fn drive_sign_follows_the_right_hand_rule() {
    // Positive rotation about +x moves the top toward +z (south).
    assert_eq!(drive_sign(Dir::South), 1.0);
    assert_eq!(drive_sign(Dir::North), -1.0);
    assert_eq!(drive_sign(Dir::West), 1.0);
    assert_eq!(drive_sign(Dir::East), -1.0);
    assert_eq!(drive_axis(Dir::East), Axis::Z);
}

#[test]
fn spacing_holds_at_any_speed_and_a_blocked_end_backs_up() {
    for rpm in [1.0, 13.0, 64.0, 100.0, 128.0] {
        let mut line = flat(4);
        let mut world = End {
            open: false,
            got: vec![],
        };
        for t in 0..4000 {
            if t % 3 == 0 {
                let _ = line.insert_on(0, format!("i{t}"));
            }
            line.step(blocks_per_tick(rpm), &[], &mut world);
            spaced_ok(&line);
        }
        // Full: two per block, nothing lost, nothing past the end.
        assert_eq!(line.count(), line.capacity(), "rpm {rpm}");
        assert!(line.entries.iter().all(|e| e.x <= 4.0 + 1e-9));
    }
}

#[test]
fn an_open_end_delivers_at_most_one_item_per_spacing() {
    let mut line = flat(8);
    let mut world = End {
        open: true,
        got: vec![],
    };
    for i in 0..16 {
        line.insert(i as f64 * 0.5, format!("{i}")).unwrap();
    }
    // 64 RPM: 0.1 blocks per tick: 2 items per second.
    for _ in 0..100 {
        line.step(blocks_per_tick(64.0), &[], &mut world);
    }
    // 5 s at 2 blocks/s: 10 blocks of travel, 20 items' worth; 16 were there.
    assert_eq!(world.got.len(), 16);
    // In order: the front first.
    assert_eq!(world.got[0], "15");
}

#[test]
fn insertion_is_refused_without_room() {
    let mut line = flat(2);
    assert!(line.insert_on(0, "a".into()).is_ok());
    // The centre is taken; within ±0.25 there is no spot 0.5 away from it.
    assert!(line.insert_on(0, "b".into()).is_err());
    // The next block's centre is 1 away: fine.
    assert!(line.insert_on(1, "c".into()).is_ok());
    // A spot off-centre is found when the centre is 0.3 from an item.
    let mut line = flat(2);
    line.insert(0.2, "a".into()).unwrap();
    let x = line.insert_on(0, "b".into()).unwrap();
    assert!((x - 0.7).abs() < 1e-9);
}

#[test]
fn side_loading_lands_at_the_block_centre() {
    let mut line = flat(5);
    let x = line.insert_on(3, "side".into()).unwrap();
    assert_eq!(x, 3.5);
}

#[test]
fn slopes_cost_more_going_up() {
    let blocks = vec![Pos::new(0, 0, 0), Pos::new(1, 1, 0), Pos::new(2, 1, 0)];
    let s = slopes(&blocks);
    assert_eq!(s, vec![Slope::Rising, Slope::Flat, Slope::Flat]);
    let mut line: Line<String> = Line::new(blocks, Dir::East, s);
    line.insert(0.5, "up".into()).unwrap();
    let base = LOAD_PER_BLOCK * 3.0;
    assert!((line.load(true) - (base + 0.5)).abs() < 1e-12);
    // Running backwards the item goes down the slope: flat cost.
    assert!((line.load(false) - (base + 0.125)).abs() < 1e-12);
    // Falling slopes: the reverse.
    let blocks = vec![Pos::new(0, 1, 0), Pos::new(1, 0, 0)];
    assert_eq!(slopes(&blocks), vec![Slope::Flat, Slope::Falling]);
}

#[test]
fn the_reference_loads_from_the_spec() {
    // 16 flat blocks with 32 items: 6 Nm; 64 with 128: 24 Nm.
    let mut line = flat(16);
    for i in 0..32 {
        line.insert(i as f64 * 0.5, "x".into()).unwrap();
    }
    assert!((line.load(true) - 6.0).abs() < 1e-12);
    let mut line = flat(64);
    for i in 0..128 {
        line.insert(i as f64 * 0.5, "x".into()).unwrap();
    }
    assert!((line.load(true) - 24.0).abs() < 1e-12);
    // An elevator of 16 full of items: 18 Nm.
    let mut lift: Line<String> = Line::elevator((0..16).map(|y| Pos::new(0, y, 0)).collect());
    for i in 0..32 {
        lift.insert(i as f64 * 0.5, "x".into()).unwrap();
    }
    assert!((lift.load(true) - 18.0).abs() < 1e-12);
}

#[test]
fn a_reversed_line_delivers_at_the_other_end() {
    let mut line = flat(4);
    line.insert_on(2, "a".into()).unwrap();
    let mut world = End {
        open: true,
        got: vec![],
    };
    assert_eq!(line.end_target(false), Pos::new(-1, 0, 0));
    for _ in 0..40 {
        line.step(blocks_per_tick(-64.0), &[], &mut world);
    }
    assert_eq!(world.got, vec!["a".to_string()]);
}

/// A diverter at block 2: pushes items off while powered, if its side takes them.
struct Diverter {
    powered: bool,
    side_full: bool,
    pushed: usize,
    pulses: usize,
}

impl Stations<String> for Diverter {
    fn arrive(&mut self, _: usize, _: &String) -> Action {
        self.pulses += 1;
        if self.powered && !self.side_full {
            self.pushed += 1;
            Action::Remove
        } else {
            Action::Pass
        }
    }
    fn poll(&mut self, _: usize, _: &mut String) -> bool {
        false
    }
    fn deliver(&mut self, _: &String) -> bool {
        true
    }
}

#[test]
fn the_diverter_pushes_off_and_passes_straight_when_its_side_is_full() {
    for (powered, full, pushed) in [(true, false, 3), (true, true, 0), (false, false, 0)] {
        let mut line = flat(5);
        for i in 0..3 {
            line.insert(i as f64 * 0.5, format!("{i}")).unwrap();
        }
        let mut d = Diverter {
            powered,
            side_full: full,
            pushed: 0,
            pulses: 0,
        };
        let mut left = 0;
        for _ in 0..200 {
            left += line.step(blocks_per_tick(64.0), &[2], &mut d).left;
        }
        assert_eq!(d.pushed, pushed);
        assert_eq!(d.pulses, 3, "every item pulses once");
        assert_eq!(left, 3);
        assert!(line.entries.is_empty());
    }
}

/// A machine at block 1 working 10 ticks on items named "ore", making "dust"
/// plus a byproduct behind.
struct Machine {
    timer: u32,
}

impl Stations<String> for Machine {
    fn arrive(&mut self, _: usize, item: &String) -> Action {
        if item == "ore" {
            self.timer = 10;
            Action::Hold
        } else {
            Action::Pass
        }
    }
    fn poll(&mut self, _: usize, item: &mut String) -> bool {
        if self.timer > 0 {
            self.timer -= 1;
            if self.timer == 0 {
                *item = "dust".into();
                return false;
            }
            return true;
        }
        false
    }
    fn deliver(&mut self, _: &String) -> bool {
        false
    }
}

#[test]
fn a_machine_holds_its_item_and_lets_others_pass() {
    let mut line = flat(4);
    line.insert(0.0, "stone".into()).unwrap();
    let mut m = Machine { timer: 0 };
    let mut held_seen = false;
    for _ in 0..60 {
        let out = line.step(blocks_per_tick(64.0), &[1], &mut m);
        let _ = out;
        if line.entries.iter().any(|e| e.hold.is_some()) {
            held_seen = true;
        }
    }
    assert!(!held_seen, "stone passes untouched");
    let mut line = flat(4);
    line.insert(0.0, "ore".into()).unwrap();
    line.insert(-0.0 + 0.0, "x".into()).unwrap_err();
    let mut ticks_held = 0;
    for _ in 0..60 {
        line.step(blocks_per_tick(64.0), &[1], &mut m);
        if line.entries[0].hold.is_some() {
            ticks_held += 1;
            assert_eq!(line.entries[0].x, 1.5);
        }
    }
    assert!(ticks_held >= 9, "held while it works: {ticks_held}");
    assert_eq!(line.entries[0].item, "dust");
    assert_eq!(line.entries[0].x, 4.0, "then it moves on to the end");
}

#[test]
fn byproducts_go_behind_and_wait_when_there_is_no_room() {
    let mut line = flat(4);
    line.insert(1.5, "dust".into()).unwrap();
    assert_eq!(line.insert_behind(1.5, true, "nugget".into()), Ok(1.0));
    // Behind is now taken: the next byproduct can't go.
    assert!(line.insert_behind(1.5, true, "more".into()).is_err());
}

#[test]
fn an_elevator_of_33_doesnt_form() {
    assert_eq!(elevator_ok(1), Err("too_short"));
    assert_eq!(elevator_ok(2), Ok(()));
    assert_eq!(elevator_ok(32), Ok(()));
    assert_eq!(elevator_ok(33), Err("too_tall"));
}

#[test]
fn extrapolation_matches_the_server_over_200_ticks() {
    let mut line = flat(12);
    let mut world = End {
        open: false,
        got: vec![],
    };
    for i in [0.0, 0.6, 1.4, 3.0, 3.5, 7.25] {
        line.insert(i, "x".into()).unwrap();
    }
    let snap = line.snapshot();
    for v in [blocks_per_tick(37.0), blocks_per_tick(-90.0)] {
        let mut l = line.clone();
        for _ in 0..200 {
            l.step(v, &[], &mut world);
        }
        let predicted = extrapolate(&snap, v, 12.0, 200);
        let actual: Vec<f64> = l.entries.iter().map(|e| e.x).collect();
        for (a, b) in predicted.iter().zip(&actual) {
            assert!((a - b).abs() < 1e-9, "{predicted:?} vs {actual:?}");
        }
    }
}

#[test]
fn save_and_load_keep_every_entry_within_a_sixteenth() {
    let mut line = flat(6);
    let xs = [0.1, 0.9, 1.5, 2.0, 2.75, 5.5, 6.0];
    for (i, x) in xs.iter().enumerate() {
        line.insert(*x, format!("{i}")).unwrap();
    }
    let per_block: Vec<Vec<(String, f64)>> = (0..6).map(|i| line.entries_in(i)).collect();
    let total: usize = per_block.iter().map(|v| v.len()).sum();
    assert_eq!(total, xs.len(), "each entry saved once");
    let mut back = flat(6);
    for (i, saved) in per_block.into_iter().enumerate() {
        assert!(back.restore(i, saved).is_empty());
    }
    assert_eq!(back.count(), xs.len());
    for (a, b) in back.entries.iter().zip(&line.entries) {
        assert!((a.x - b.x).abs() <= 1.0 / 16.0);
        assert_eq!(a.item, b.item);
    }
}

#[test]
fn breaking_a_block_takes_its_items() {
    let mut line = flat(3);
    for x in [0.5, 1.2, 1.7, 2.5] {
        line.insert(x, "x".into()).unwrap();
    }
    assert_eq!(line.take_block(1).len(), 2);
    assert_eq!(line.count(), 2);
}

#[test]
fn chains_join_in_line_and_split_at_64() {
    use std::collections::HashMap;
    let mut belts: HashMap<Pos, BeltBlock> = HashMap::new();
    for x in 0..70 {
        belts.insert(Pos::new(x, 0, 0), BeltBlock { heading: Dir::East });
    }
    // A step up at x=70 and a side-by-side belt that doesn't join.
    belts.insert(Pos::new(70, 1, 0), BeltBlock { heading: Dir::East });
    belts.insert(Pos::new(5, 0, 1), BeltBlock { heading: Dir::East });
    belts.insert(
        Pos::new(71, 1, 0),
        BeltBlock {
            heading: Dir::North,
        },
    );
    let get = |p: Pos| belts.get(&p).copied();
    let c = chain(Pos::new(30, 0, 0), &get);
    assert_eq!(c.len(), 71);
    assert_eq!(c[0], Pos::new(0, 0, 0));
    let lines = split(&c);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0.len(), 64);
    assert_eq!(lines[1].0.len(), 7);
    assert_eq!(lines[1].1[5], Slope::Rising);
    assert_eq!(chain(Pos::new(5, 0, 1), &get).len(), 1);
}

#[test]
fn a_v_doesnt_join() {
    let blocks = [Pos::new(0, 1, 0), Pos::new(1, 0, 0), Pos::new(2, 1, 0)];
    let get = |p: Pos| {
        blocks
            .contains(&p)
            .then_some(BeltBlock { heading: Dir::East })
    };
    let c = chain(Pos::new(0, 1, 0), &get);
    assert_eq!(c.len(), 2);
}

#[test]
fn a_full_line_steps_fast() {
    let mut line = flat(64);
    for i in 0..128 {
        line.insert(i as f64 * 0.5, "x".into()).unwrap();
    }
    let mut world = End {
        open: false,
        got: vec![],
    };
    let start = std::time::Instant::now();
    let n = 2000;
    for _ in 0..n {
        line.step(blocks_per_tick(64.0), &[], &mut world);
    }
    let per = start.elapsed().as_secs_f64() / n as f64;
    // Spec target 5 µs in a release build; debug test builds are slower.
    let limit = if cfg!(debug_assertions) { 100e-6 } else { 5e-6 };
    assert!(per < limit, "{per}");
}
