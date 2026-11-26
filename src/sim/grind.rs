//! The grinder, free of the game (grinder spec, sections 2 to 5 and 10):
//! recipes and matching, the space check, progress from speed, completion
//! and chance rolls, the status line and the comparator signal.

use crate::sim::rot::part::{GRINDER_MAX_RPM, GRINDER_MIN_RPM};

/// What a recipe takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Ingredient {
    Item(String),
    Tag(String),
}

/// One result entry: `count` items, each kept with probability `chance`.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub item: String,
    pub count: u32,
    pub chance: f64,
}

/// A `fras:grinding` recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub ingredient: Ingredient,
    pub count: u32,
    pub results: Vec<Output>,
    pub revolutions: f64,
    pub load: f64,
}

/// The default load, Nm.
pub const DEFAULT_LOAD: f64 = 6.0;
/// Push and pull cadence, ticks.
pub const TRANSFER_EVERY: u64 = 8;
/// Output slots.
pub const OUTPUTS: usize = 4;

/// What the grinder needs to know about items.
pub trait Items {
    fn has_tag(&self, item: &str, tag: &str) -> bool;
    fn max_stack(&self, item: &str) -> u32;
}

/// A stack as the grinder sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
}

impl Stack {
    pub fn new(item: &str, count: u32) -> Stack {
        Stack {
            item: item.to_owned(),
            count,
        }
    }
}

impl Recipe {
    pub fn matches(&self, item: &str, items: &dyn Items) -> bool {
        match &self.ingredient {
            Ingredient::Item(id) => id == item,
            Ingredient::Tag(tag) => items.has_tag(item, tag),
        }
    }

    /// The most of each item it can make (every chance roll kept).
    pub fn max_results(&self) -> Vec<Stack> {
        let mut out: Vec<Stack> = Vec::new();
        for r in &self.results {
            if r.chance <= 0.0 {
                continue;
            }
            match out.iter_mut().find(|s| s.item == r.item) {
                Some(s) => s.count += r.count,
                None => out.push(Stack::new(&r.item, r.count)),
            }
        }
        out
    }

    /// Roll its results; `random` gives numbers in [0, 1).
    pub fn roll(&self, random: &mut dyn FnMut() -> f64) -> Vec<Stack> {
        let mut out: Vec<Stack> = Vec::new();
        for r in &self.results {
            let mut n = 0;
            for _ in 0..r.count {
                if r.chance >= 1.0 || random() < r.chance {
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            match out.iter_mut().find(|s| s.item == r.item) {
                Some(s) => s.count += n,
                None => out.push(Stack::new(&r.item, n)),
            }
        }
        out
    }
}

/// Check a recipe's fields (spec 2.2): 1 to 4 results, chances in range,
/// revolutions above zero.
pub fn validate(recipe: &Recipe) -> Result<(), String> {
    if recipe.results.is_empty() || recipe.results.len() > 4 {
        return Err(format!("{} results (1 to 4)", recipe.results.len()));
    }
    if recipe.count == 0 {
        return Err("count 0".into());
    }
    for r in &recipe.results {
        if !(0.0..=1.0).contains(&r.chance) {
            return Err(format!("chance {} out of 0 to 1", r.chance));
        }
        if r.count == 0 {
            return Err("result count 0".into());
        }
    }
    if recipe.revolutions <= 0.0 || !recipe.revolutions.is_finite() {
        return Err(format!("revolutions {}", recipe.revolutions));
    }
    if recipe.load < 0.0 || !recipe.load.is_finite() {
        return Err(format!("load {}", recipe.load));
    }
    Ok(())
}

/// The recipe for an item: the first by recipe id.
pub fn find<'a>(recipes: &'a [Recipe], item: &str, items: &dyn Items) -> Option<&'a Recipe> {
    recipes
        .iter()
        .filter(|r| r.matches(item, items))
        .min_by(|a, b| a.id.cmp(&b.id))
}

/// Ids of recipes that share an input with an earlier one (by id), for the
/// load-time warning. Only exact item ingredients are compared.
pub fn duplicates(recipes: &[Recipe]) -> Vec<(String, String)> {
    let mut sorted: Vec<&Recipe> = recipes.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out = Vec::new();
    for (i, r) in sorted.iter().enumerate() {
        for earlier in &sorted[..i] {
            if earlier.ingredient == r.ingredient {
                out.push((earlier.id.clone(), r.id.clone()));
                break;
            }
        }
    }
    out
}

/// Whether `results` fit into the output slots.
pub fn fits(results: &[Stack], outputs: &[Option<Stack>], items: &dyn Items) -> bool {
    let mut slots: Vec<Option<Stack>> = outputs.to_vec();
    insert(results, &mut slots, items)
}

/// Put `results` into the output slots, filling matching stacks first.
/// Returns false if they didn't all fit (the slots are left as far as they got).
pub fn insert(results: &[Stack], outputs: &mut [Option<Stack>], items: &dyn Items) -> bool {
    for r in results {
        let max = items.max_stack(&r.item).max(1);
        let mut left = r.count;
        for slot in outputs.iter_mut() {
            if left == 0 {
                break;
            }
            if let Some(s) = slot
                && s.item == r.item
                && s.count < max
            {
                let n = (max - s.count).min(left);
                s.count += n;
                left -= n;
            }
        }
        for slot in outputs.iter_mut() {
            if left == 0 {
                break;
            }
            if slot.is_none() {
                let n = max.min(left);
                *slot = Some(Stack::new(&r.item, n));
                left -= n;
            }
        }
        if left > 0 {
            return false;
        }
    }
    true
}

/// Revolutions done in one tick at `rpm` within a working range (spec 3):
/// none below the minimum, capped at the maximum.
pub fn progress_per_tick(rpm: f64, min: f64, max: f64) -> f64 {
    let w = rpm.abs();
    if w < min { 0.0 } else { w.min(max) / 1200.0 }
}

/// The grinder's working range: cooling water raises the max (cooling spec 6).
pub fn working_range(cooled: bool) -> (f64, f64) {
    if cooled {
        (GRINDER_MIN_RPM, crate::sim::fluid::cool::GRINDER_COOLED_MAX)
    } else {
        (GRINDER_MIN_RPM, GRINDER_MAX_RPM)
    }
}

/// The status line, first match wins (spec 6). Codes are the data slot values.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Jammed = 1,
    Paused = 2,
    Overspeed = 3,
    NoPower = 4,
    TooSlow = 5,
    OutputFull = 6,
    Grinding = 7,
    Idle = 0,
}

impl Status {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Status {
        match code {
            1 => Status::Jammed,
            2 => Status::Paused,
            3 => Status::Overspeed,
            4 => Status::NoPower,
            5 => Status::TooSlow,
            6 => Status::OutputFull,
            7 => Status::Grinding,
            _ => Status::Idle,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Status::Jammed => "jammed",
            Status::Paused => "paused",
            Status::Overspeed => "overspeed",
            Status::NoPower => "no_power",
            Status::TooSlow => "too_slow",
            Status::OutputFull => "output_full",
            Status::Grinding => "grinding",
            Status::Idle => "idle",
        }
    }
}

/// What a grinder is doing this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Situation {
    pub jammed: bool,
    pub paused: bool,
    pub rpm: f64,
    /// Cooling water raises its limit (cooling spec 6).
    pub cooled: bool,
    /// The input holds a recipe's ingredient and count.
    pub has_recipe: bool,
    /// Its results fit.
    pub fits: bool,
}

pub fn status(s: &Situation) -> Status {
    let (min, max) = working_range(s.cooled);
    let w = s.rpm.abs();
    if s.jammed {
        Status::Jammed
    } else if s.paused {
        Status::Paused
    } else if w > max {
        Status::Overspeed
    } else if s.has_recipe && !s.fits {
        Status::OutputFull
    } else if !s.has_recipe {
        Status::Idle
    } else if w == 0.0 {
        Status::NoPower
    } else if w < min {
        Status::TooSlow
    } else {
        Status::Grinding
    }
}

/// The grinder's saved work.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Work {
    /// Revolutions done on the current recipe.
    pub progress: f64,
    /// The recipe they count for.
    pub recipe: Option<String>,
}

/// One tick's result.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Load to report for this tick, Nm.
    pub load: f64,
    pub status: Status,
    /// Items taken from the input.
    pub consumed: u32,
    /// Revolutions needed for the current recipe (for the arrow).
    pub needed: f64,
}

/// One tick of a grinder: its input and outputs, its speed and state.
/// Changes `work`, `outputs` and returns how many inputs to take.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    work: &mut Work,
    input: Option<&Stack>,
    outputs: &mut [Option<Stack>],
    rpm: f64,
    jammed: bool,
    paused: bool,
    cooled: bool,
    recipes: &[Recipe],
    items: &dyn Items,
    random: &mut dyn FnMut() -> f64,
) -> Tick {
    let recipe = input.and_then(|s| find(recipes, &s.item, items).filter(|r| s.count >= r.count));
    // A different recipe (or none) resets the progress.
    let id = input
        .and_then(|s| find(recipes, &s.item, items))
        .map(|r| r.id.clone());
    if id != work.recipe {
        work.recipe = id;
        work.progress = 0.0;
    }
    let fits = recipe.is_some_and(|r| fits(&r.max_results(), outputs, items));
    let situation = Situation {
        jammed,
        paused,
        rpm,
        cooled,
        has_recipe: recipe.is_some(),
        fits,
    };
    let status = status(&situation);
    let needed = recipe.map_or(0.0, |r| r.revolutions);
    let has_work = recipe.is_some() && fits && !jammed;
    let mut result = Tick {
        load: if has_work { recipe.unwrap().load } else { 0.0 },
        status,
        consumed: 0,
        needed,
    };
    if !has_work || paused {
        return result;
    }
    let recipe = recipe.unwrap();
    let (min, max) = working_range(cooled);
    work.progress += progress_per_tick(rpm, min, max);
    if work.progress + 1e-9 >= recipe.revolutions {
        let rolled = recipe.roll(random);
        // The space check used the maximum rolls, so this always fits.
        let _ = insert(&rolled, outputs, items);
        work.progress = 0.0;
        result.consumed = recipe.count;
    }
    result
}

/// The vanilla container signal over `slots` (count and max stack size).
pub fn comparator(slots: &[Option<(u32, u32)>]) -> u8 {
    if slots.is_empty() {
        return 0;
    }
    let mut fullness = 0.0;
    let mut any = false;
    for (count, max) in slots.iter().flatten() {
        if *count > 0 {
            any = true;
            fullness += *count as f64 / (*max).max(1) as f64;
        }
    }
    if !any {
        return 0;
    }
    let f = fullness / slots.len() as f64;
    ((f * 14.0).floor() as u8 + 1).min(15)
}

/// The 0.1 recipes, as the data files say them (for tests and the docs).
pub fn builtin() -> Vec<Recipe> {
    let out = |item: &str, count: u32, chance: f64| Output {
        item: item.into(),
        count,
        chance,
    };
    let recipe =
        |id: &str, ing: Ingredient, count: u32, results: Vec<Output>, rev: f64, load: f64| Recipe {
            id: format!("fras:grinding/{id}"),
            ingredient: ing,
            count,
            results,
            revolutions: rev,
            load,
        };
    let item = |s: &str| Ingredient::Item(s.into());
    let mut v = Vec::new();
    for metal in ["iron", "copper", "gold"] {
        let crushed = format!("fras:crushed_{metal}");
        v.push(recipe(
            &format!("raw_{metal}"),
            item(&format!("minecraft:raw_{metal}")),
            1,
            vec![out(&crushed, 1, 1.0), out(&crushed, 1, 0.5)],
            4.0,
            6.0,
        ));
        v.push(recipe(
            &format!("raw_{metal}_block"),
            item(&format!("minecraft:raw_{metal}_block")),
            1,
            vec![out(&crushed, 9, 1.0), out(&crushed, 9, 0.5)],
            36.0,
            6.0,
        ));
    }
    v.push(recipe(
        "cobblestone",
        item("minecraft:cobblestone"),
        1,
        vec![out("minecraft:gravel", 1, 1.0)],
        2.0,
        6.0,
    ));
    v.push(recipe(
        "cobbled_deepslate",
        item("minecraft:cobbled_deepslate"),
        1,
        vec![out("minecraft:gravel", 1, 1.0)],
        2.0,
        6.0,
    ));
    v.push(recipe(
        "gravel",
        item("minecraft:gravel"),
        1,
        vec![
            out("minecraft:sand", 1, 1.0),
            out("minecraft:flint", 1, 0.1),
        ],
        2.0,
        6.0,
    ));
    v.push(recipe(
        "wool",
        Ingredient::Tag("minecraft:wool".into()),
        1,
        vec![out("minecraft:string", 4, 1.0)],
        2.0,
        3.0,
    ));
    v.push(recipe(
        "bone",
        item("minecraft:bone"),
        1,
        vec![
            out("minecraft:bone_meal", 4, 1.0),
            out("minecraft:bone_meal", 1, 0.5),
        ],
        2.0,
        3.0,
    ));
    // Rust flakes grind into pigment (rust spec 7).
    v.push(recipe(
        "rust_flakes",
        item("fras:rust_flakes"),
        1,
        vec![out("fras:red_oxide_pigment", 1, 1.0)],
        2.0,
        3.0,
    ));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItems;
    impl Items for TestItems {
        fn has_tag(&self, item: &str, tag: &str) -> bool {
            tag == "minecraft:wool" && item.ends_with("_wool")
        }
        fn max_stack(&self, _: &str) -> u32 {
            64
        }
    }

    /// A small deterministic random source.
    fn lcg(seed: u64) -> impl FnMut() -> f64 {
        let mut s = seed;
        move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn average(recipe: &Recipe, item: &str, runs: usize) -> f64 {
        let mut rng = lcg(42);
        let mut total = 0;
        for _ in 0..runs {
            total += recipe
                .roll(&mut rng)
                .iter()
                .filter(|s| s.item == item)
                .map(|s| s.count)
                .sum::<u32>();
        }
        total as f64 / runs as f64
    }

    fn get(id: &str) -> Recipe {
        builtin().into_iter().find(|r| r.id.ends_with(id)).unwrap()
    }

    #[test]
    fn every_builtin_recipe_is_valid() {
        for r in builtin() {
            validate(&r).unwrap();
        }
        assert!(duplicates(&builtin()).is_empty());
    }

    #[test]
    fn yields_average_as_designed() {
        let ore = average(&get("/raw_iron"), "fras:crushed_iron", 20_000);
        assert!((ore - 1.5).abs() < 0.02, "{ore}");
        let block = average(&get("/raw_copper_block"), "fras:crushed_copper", 5_000);
        assert!((block - 13.5).abs() < 0.1, "{block}");
        let bone = average(&get("/bone"), "minecraft:bone_meal", 20_000);
        assert!((bone - 4.5).abs() < 0.02, "{bone}");
        let flint = average(&get("/gravel"), "minecraft:flint", 20_000);
        assert!((flint - 0.1).abs() < 0.01, "{flint}");
    }

    #[test]
    fn revolutions_turn_into_time_by_speed() {
        let r = get("/raw_iron");
        let seconds = |rpm: f64| {
            let per_tick = progress_per_tick(rpm, GRINDER_MIN_RPM, GRINDER_MAX_RPM);
            if per_tick == 0.0 {
                return f64::INFINITY;
            }
            r.revolutions / per_tick / 20.0
        };
        assert!((seconds(19.0) - 12.63).abs() < 0.01);
        assert_eq!(seconds(48.0), 5.0);
        assert_eq!(seconds(96.0), 2.5);
        // Above the max it counts as the max.
        assert_eq!(seconds(150.0), 2.5);
        assert_eq!(seconds(12.0), 20.0);
        assert_eq!(seconds(11.9), f64::INFINITY);
        assert_eq!(get("/raw_gold_block").revolutions, 36.0);
    }

    #[test]
    fn tags_match_every_wool_colour() {
        let recipes = builtin();
        for colour in ["white", "red", "light_blue", "black"] {
            let item = format!("minecraft:{colour}_wool");
            assert!(find(&recipes, &item, &TestItems).is_some_and(|r| r.id.ends_with("/wool")));
        }
        assert!(find(&recipes, "minecraft:iron_ore", &TestItems).is_none());
    }

    #[test]
    fn load_only_while_it_has_work() {
        let recipes = builtin();
        let mut work = Work::default();
        let mut outputs: Vec<Option<Stack>> = vec![None; 4];
        let mut rng = lcg(1);
        let wool = Stack::new("minecraft:red_wool", 3);
        let t = tick(
            &mut work,
            Some(&wool),
            &mut outputs,
            20.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!((t.load, t.status), (3.0, Status::Grinding));
        let t = tick(
            &mut work,
            None,
            &mut outputs,
            20.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!((t.load, t.status), (0.0, Status::Idle));
        // Below the minimum: no progress, full load.
        let cobble = Stack::new("minecraft:cobblestone", 1);
        let before = work.progress;
        let t = tick(
            &mut work,
            Some(&cobble),
            &mut outputs,
            8.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!((t.load, t.status), (6.0, Status::TooSlow));
        assert_eq!(work.progress, before);
        // Jammed: no load.
        let t = tick(
            &mut work,
            Some(&cobble),
            &mut outputs,
            0.0,
            true,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!((t.load, t.status), (0.0, Status::Jammed));
    }

    #[test]
    fn a_full_output_stops_before_starting_and_never_voids() {
        let recipes = builtin();
        let mut work = Work::default();
        let full = Some(Stack::new("minecraft:dirt", 64));
        let mut outputs = vec![
            full.clone(),
            full.clone(),
            full.clone(),
            Some(Stack::new("fras:crushed_iron", 63)),
        ];
        let ore = Stack::new("minecraft:raw_iron", 5);
        let mut rng = lcg(3);
        // Room for 1, the recipe can make 2: it doesn't start.
        let t = tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            96.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!((t.load, t.status), (0.0, Status::OutputFull));
        assert_eq!(work.progress, 0.0);
        outputs[3] = Some(Stack::new("fras:crushed_iron", 62));
        let t = tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            96.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!(t.status, Status::Grinding);
    }

    #[test]
    fn chance_rolls_only_at_completion_and_progress_survives_stops() {
        let recipes = builtin();
        let mut work = Work::default();
        let mut outputs: Vec<Option<Stack>> = vec![None; 4];
        let ore = Stack::new("minecraft:raw_iron", 2);
        let mut calls = 0;
        let mut rng = || {
            calls += 1;
            0.0
        };
        // 96 RPM: 4 revolutions take 50 ticks. Stop halfway (no power, jam,
        // pause); progress is kept.
        for _ in 0..25 {
            tick(
                &mut work,
                Some(&ore),
                &mut outputs,
                96.0,
                false,
                false,
                false,
                &recipes,
                &TestItems,
                &mut rng,
            );
        }
        let half = work.progress;
        assert!((half - 2.0).abs() < 1e-9);
        tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            0.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            96.0,
            true,
            false,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            96.0,
            false,
            true,
            false,
            &recipes,
            &TestItems,
            &mut rng,
        );
        assert_eq!(work.progress, half);
        let mut consumed = 0;
        for _ in 0..25 {
            consumed += tick(
                &mut work,
                Some(&ore),
                &mut outputs,
                96.0,
                false,
                false,
                false,
                &recipes,
                &TestItems,
                &mut rng,
            )
            .consumed;
        }
        assert_eq!(consumed, 1);
        assert_eq!(outputs[0], Some(Stack::new("fras:crushed_iron", 2)));
        assert_eq!(calls, 1, "one roll for the one chance result");
        // A different input resets the progress.
        tick(
            &mut work,
            Some(&ore),
            &mut outputs,
            96.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut || 0.9,
        );
        assert!(work.progress > 0.0);
        let wool = Stack::new("minecraft:white_wool", 1);
        tick(
            &mut work,
            Some(&wool),
            &mut outputs,
            0.0,
            false,
            false,
            false,
            &recipes,
            &TestItems,
            &mut || 0.9,
        );
        assert_eq!(work.progress, 0.0);
    }

    #[test]
    fn the_status_line_keeps_its_order() {
        let s = |jammed, paused, rpm, has_recipe, fits| {
            status(&Situation {
                jammed,
                paused,
                rpm,
                cooled: false,
                has_recipe,
                fits,
            })
        };
        assert_eq!(s(true, true, 200.0, true, false), Status::Jammed);
        assert_eq!(s(false, true, 200.0, true, false), Status::Paused);
        assert_eq!(s(false, false, 200.0, true, false), Status::Overspeed);
        assert_eq!(s(false, false, 50.0, true, false), Status::OutputFull);
        assert_eq!(s(false, false, 0.0, true, true), Status::NoPower);
        assert_eq!(s(false, false, 5.0, true, true), Status::TooSlow);
        assert_eq!(s(false, false, 50.0, true, true), Status::Grinding);
        assert_eq!(s(false, false, 50.0, false, true), Status::Idle);
        for code in 0..8 {
            assert_eq!(Status::from_code(code).code(), code);
        }
    }

    #[test]
    fn the_comparator_matches_the_vanilla_formula() {
        assert_eq!(comparator(&[None, None, None, None, None]), 0);
        assert_eq!(comparator(&[Some((1, 64)), None, None, None, None]), 1);
        let full = Some((64, 64));
        assert_eq!(comparator(&[full, full, full, full, full]), 15);
        // 2.5 of 5 slots full: floor(0.5 * 14) + 1 = 8.
        assert_eq!(comparator(&[full, full, Some((32, 64)), None, None]), 8);
    }
}
