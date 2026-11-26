//! 0.4's machines, free of the game (machines spec 1.1): each machine's
//! numbers, the recipes they ship with, the line machines' built-in belt
//! (`station`, machines spec 2.4) and the trommel's drum (machines spec 8).
//!
//! The recipe model, the space check, progress and the status line are the
//! grinder's (`sim::grind`), shared by every recipe machine.

use crate::sim::grind::{
    self, Ingredient, Items, MachineState, Output, Recipe, Status, find, progress_per_tick,
};

/// A machine with one shaft socket and a working range.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Machine {
    Grinder,
    InlineGrinder,
    Washer,
    TripHammer,
    Lathe,
    Sawmill,
    Trommel,
}

impl Machine {
    pub const ALL: [Machine; 7] = [
        Machine::Grinder,
        Machine::InlineGrinder,
        Machine::Washer,
        Machine::TripHammer,
        Machine::Lathe,
        Machine::Sawmill,
        Machine::Trommel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Machine::Grinder => "grinder",
            Machine::InlineGrinder => "inline_grinder",
            Machine::Washer => "washer",
            Machine::TripHammer => "trip_hammer",
            Machine::Lathe => "lathe",
            Machine::Sawmill => "sawmill",
            Machine::Trommel => "trommel",
        }
    }

    /// Its recipe type (`None`: the trommel sorts by tags).
    pub fn recipe_type(self) -> Option<&'static str> {
        Some(match self {
            Machine::Grinder | Machine::InlineGrinder => "fras:grinding",
            Machine::Washer => "fras:washing",
            Machine::TripHammer => "fras:hammering",
            Machine::Lathe => "fras:turning",
            Machine::Sawmill => "fras:sawing",
            Machine::Trommel => return None,
        })
    }

    /// The working range, RPM (machines spec 13.1).
    pub fn range(self) -> (f64, f64) {
        match self {
            Machine::Grinder | Machine::InlineGrinder => grind::working_range(false),
            Machine::Washer => (8.0, 64.0),
            Machine::TripHammer => (16.0, 64.0),
            Machine::Lathe => (128.0, 224.0),
            Machine::Sawmill => (32.0, 128.0),
            Machine::Trommel => (8.0, 96.0),
        }
    }

    pub fn inertia(self) -> f64 {
        match self {
            Machine::Grinder | Machine::InlineGrinder | Machine::Washer | Machine::Sawmill => 4.0,
            Machine::TripHammer | Machine::Trommel => 6.0,
            Machine::Lathe => 2.0,
        }
    }

    /// The load a recipe file without `load` gets, Nm.
    pub fn default_load(self) -> f64 {
        match self {
            Machine::Grinder | Machine::InlineGrinder | Machine::Sawmill => 6.0,
            Machine::Washer | Machine::Lathe => 4.0,
            Machine::TripHammer => 8.0,
            Machine::Trommel => TROMMEL_BASE_LOAD,
        }
    }

    /// The water a recipe file without `water` gets, mB (washing: 50).
    pub fn default_water(self) -> f64 {
        if self == Machine::Washer { 50.0 } else { 0.0 }
    }

    /// What the wrench needs to repair it (machines spec 13.1).
    pub fn repair(self) -> &'static [(&'static str, u32)] {
        match self {
            Machine::Grinder | Machine::InlineGrinder => &[("minecraft:flint", 1)],
            Machine::Washer => &[("minecraft:iron_nugget", 1), ("#minecraft:planks", 1)],
            Machine::Lathe => &[("minecraft:iron_nugget", 1), ("minecraft:flint", 1)],
            Machine::TripHammer | Machine::Sawmill | Machine::Trommel => {
                &[("minecraft:iron_nugget", 2)]
            }
        }
    }

    /// Whether it is a two-block line machine with a built-in belt
    /// (machines spec 2.3): all but the 0.1 grinder and the trommel.
    pub fn is_line(self) -> bool {
        !matches!(self, Machine::Grinder | Machine::Trommel)
    }

    /// Strikes per revolution, for the GUI's count (the trip hammer's cam).
    pub fn strikes_per_rev(self) -> f64 {
        if self == Machine::TripHammer {
            2.0
        } else {
            0.0
        }
    }
}

/// Washer tank size, mB (machines spec 4.3).
pub const WASHER_TANK: f64 = 1000.0;
/// Trip hammer: damage per strike to anything under its head on a belt.
pub const HAMMER_DAMAGE: f32 = 2.0;
/// Sawmill: damage on touch, every this many ticks.
pub const SAW_DAMAGE: f32 = 1.0;
pub const SAW_DAMAGE_EVERY: u64 = 10;
/// Trommel numbers (machines spec 8).
pub const TROMMEL_CAPACITY: usize = 8;
pub const TROMMEL_REVS: f64 = 1.0;
pub const TROMMEL_BASE_LOAD: f64 = 2.0;
pub const TROMMEL_LOAD_PER_ITEM: f64 = 0.25;
pub const TROMMEL_MAX_LOAD: f64 = 4.0;
/// Dust smelts in this many ticks (blast furnace: half).
pub const DUST_SMELT_TICKS: u32 = 100;

// ---- Work on one item -------------------------------------------------------------------

/// A line machine's work on the item at its work point (machines spec 2.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeltWork {
    pub progress: f64,
    pub recipe: Option<String>,
    /// Results waiting to go onto the belt behind the main one; the machine
    /// takes no new item until they are out.
    pub pending: Vec<String>,
}

/// Whether a line machine stops an item at its work point: it has a recipe
/// for it (count 1). A machine that can't work on it yet (jammed, too slow,
/// out of water, byproducts still waiting) holds it all the same and the
/// line backs up (machines spec 2.4); only items with no recipe pass.
pub fn belt_accepts(item: &str, recipes: &[Recipe], items: &dyn Items) -> bool {
    find(recipes, item, items).is_some_and(|r| r.count == 1)
}

/// One tick of belt-mode work on the held item.
#[derive(Clone, Debug, PartialEq)]
pub struct BeltTick {
    pub load: f64,
    pub status: Status,
    pub needed: f64,
    pub water_used: f64,
    /// The recipe finished: the held item becomes this (`None`: nothing
    /// rolled, the item is used up), and the rest went to `pending`.
    pub done: Option<Option<String>>,
}

/// Work on `item` held under the machine.
pub fn belt_tick(
    work: &mut BeltWork,
    item: &str,
    state: &MachineState,
    recipes: &[Recipe],
    items: &dyn Items,
    random: &mut dyn FnMut() -> f64,
) -> BeltTick {
    let recipe = find(recipes, item, items).filter(|r| r.count == 1);
    let id = recipe.map(|r| r.id.clone());
    if id != work.recipe {
        work.recipe = id;
        work.progress = 0.0;
    }
    let water_ok = match (recipe, state.water) {
        (Some(r), Some(w)) => w + 1e-9 >= r.water,
        _ => true,
    };
    let status = grind::status(&grind::Situation {
        jammed: state.jammed,
        paused: state.paused,
        rpm: state.rpm,
        range: state.range,
        has_recipe: recipe.is_some(),
        fits: true,
        water: water_ok,
    });
    let has_work = recipe.is_some() && water_ok && !state.jammed;
    let mut out = BeltTick {
        load: if has_work { recipe.unwrap().load } else { 0.0 },
        status,
        needed: recipe.map_or(0.0, |r| r.revolutions),
        water_used: 0.0,
        done: None,
    };
    if !has_work || state.paused {
        return out;
    }
    let recipe = recipe.unwrap();
    let (min, max) = state.range;
    work.progress += progress_per_tick(state.rpm, min, max);
    if work.progress + 1e-9 >= recipe.revolutions {
        work.progress = 0.0;
        work.recipe = None;
        out.water_used = recipe.water;
        let mut all: Vec<String> = Vec::new();
        for s in recipe.roll(random) {
            for _ in 0..s.count {
                all.push(s.item.clone());
            }
        }
        let mut it = all.into_iter();
        let main = it.next();
        work.pending.extend(it);
        out.done = Some(main);
    }
    out
}

// ---- Trommel ----------------------------------------------------------------------

/// A trommel size class (machines spec 8.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Fine,
    Medium,
    Oversize,
}

impl Size {
    pub const ALL: [Size; 3] = [Size::Fine, Size::Medium, Size::Oversize];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Size::Fine => "fine",
            Size::Medium => "medium",
            Size::Oversize => "oversize",
        }
    }

    /// The class of an item: by the tags `fras:trommel/fine` and `fras:trommel/medium`.
    pub fn of(item: &str, items: &dyn Items) -> Size {
        if items.has_tag(item, TAG_FINE) {
            Size::Fine
        } else if items.has_tag(item, TAG_MEDIUM) {
            Size::Medium
        } else {
            Size::Oversize
        }
    }
}

pub const TAG_FINE: &str = "fras:trommel/fine";
pub const TAG_MEDIUM: &str = "fras:trommel/medium";

/// Trommel blocks in a formed drum (machines spec 8.2).
pub const TROMMEL_BLOCKS: usize = 3;
/// Items an outlet holds while it waits to push them out.
pub const TROMMEL_OUTLET_HOLD: usize = 8;

/// An item tumbling in the drum, moving away from the end it came in.
#[derive(Clone, Debug, PartialEq)]
pub struct Tumbling<T> {
    pub item: T,
    pub size: Size,
    /// Revolutions so far: it crosses the drum in `TROMMEL_REVS`.
    pub progress: f64,
    /// It came in at the low end (the block with the smallest coordinate).
    pub from_low: bool,
}

impl<T> Tumbling<T> {
    /// The block it falls out of (0 = low end): its class counted from the
    /// end it came in, fines first (8.2).
    pub fn outlet(&self) -> usize {
        let k = self.size.index();
        if self.from_low {
            k
        } else {
            TROMMEL_BLOCKS - 1 - k
        }
    }

    /// Where it falls: the middle of its outlet's block, counted from its
    /// entry end, as revolutions.
    pub fn falls_at(&self) -> f64 {
        (self.size.index() as f64 + 0.5) / TROMMEL_BLOCKS as f64 * TROMMEL_REVS
    }

    /// Its position along the drum, blocks from the low end (for drawing).
    pub fn along(&self) -> f64 {
        let x = (self.progress / TROMMEL_REVS).clamp(0.0, 1.0) * TROMMEL_BLOCKS as f64;
        if self.from_low {
            x
        } else {
            TROMMEL_BLOCKS as f64 - x
        }
    }
}

/// The trommel's drum and the outlets under its three blocks.
#[derive(Clone, Debug, PartialEq)]
pub struct Drum<T> {
    pub items: Vec<Tumbling<T>>,
    /// Items waiting under each block (0 = low end), pushed out one at a time.
    pub outlets: [Vec<T>; TROMMEL_BLOCKS],
}

impl<T> Default for Drum<T> {
    fn default() -> Self {
        Drum {
            items: Vec::new(),
            outlets: [Vec::new(), Vec::new(), Vec::new()],
        }
    }
}

/// One tick of the trommel.
#[derive(Clone, Debug, PartialEq)]
pub struct DrumTick {
    pub load: f64,
    pub status: Status,
    /// Items that fell into an outlet this tick.
    pub sorted: usize,
}

impl<T> Drum<T> {
    /// Whether the drum takes another item (8.3: a full drum refuses).
    pub fn has_room(&self) -> bool {
        self.items.len() < TROMMEL_CAPACITY
    }

    /// Put an item in at one end; false when full.
    pub fn insert(
        &mut self,
        item: T,
        id: &str,
        from_low: bool,
        items: &dyn Items,
    ) -> Result<(), T> {
        if !self.has_room() {
            return Err(item);
        }
        self.items.push(Tumbling {
            item,
            size: Size::of(id, items),
            progress: 0.0,
            from_low,
        });
        Ok(())
    }

    pub fn load(&self) -> f64 {
        (TROMMEL_BASE_LOAD + TROMMEL_LOAD_PER_ITEM * self.items.len() as f64).min(TROMMEL_MAX_LOAD)
    }

    /// Tumble every item; those at their outlet fall into it if it has room.
    /// A full outlet stops only the items bound for it (8.2).
    pub fn tick(&mut self, state: &MachineState) -> DrumTick {
        let (min, max) = state.range;
        let has_work = !self.items.is_empty() && !state.jammed;
        let status = if state.jammed {
            Status::Jammed
        } else if state.paused {
            Status::Paused
        } else if state.rpm.abs() > max {
            Status::Overspeed
        } else if self.items.is_empty() {
            Status::Idle
        } else if state.rpm == 0.0 {
            Status::NoPower
        } else if state.rpm.abs() < min {
            Status::TooSlow
        } else {
            Status::Grinding
        };
        let mut out = DrumTick {
            load: if has_work { self.load() } else { 0.0 },
            status,
            sorted: 0,
        };
        if !has_work || state.paused {
            return out;
        }
        let step = progress_per_tick(state.rpm, min, max);
        let mut keep = Vec::with_capacity(self.items.len());
        let mut waiting = 0;
        for mut t in std::mem::take(&mut self.items) {
            let at = t.falls_at();
            t.progress = (t.progress + step).min(at);
            if t.progress + 1e-9 >= at {
                let k = t.outlet();
                if self.outlets[k].len() < TROMMEL_OUTLET_HOLD {
                    self.outlets[k].push(t.item);
                    out.sorted += 1;
                    continue;
                }
                waiting += 1;
            }
            keep.push(t);
        }
        if waiting > 0 && waiting == keep.len() && out.sorted == 0 && out.status == Status::Grinding
        {
            out.status = Status::OutputFull;
        }
        self.items = keep;
        out
    }
}

// ---- The recipes 0.4 ships (as the data files say them) -----------------------------

fn out(item: &str, count: u32, chance: f64) -> Output {
    Output {
        item: item.into(),
        count,
        chance,
    }
}

#[allow(clippy::too_many_arguments)]
fn recipe(
    kind: &str,
    id: &str,
    ingredient: Ingredient,
    count: u32,
    results: Vec<Output>,
    revolutions: f64,
    load: f64,
    water: f64,
) -> Recipe {
    Recipe {
        id: format!("fras:{kind}/{id}"),
        ingredient,
        count,
        results,
        revolutions,
        load,
        water,
    }
}

fn item(s: &str) -> Ingredient {
    Ingredient::Item(s.into())
}

/// The washer's recipes (machines spec 4.2).
pub fn washing() -> Vec<Recipe> {
    let byproduct = [
        ("iron", "minecraft:iron_nugget"),
        ("copper", "minecraft:gold_nugget"),
        ("gold", "minecraft:copper_nugget"),
    ];
    let mut v: Vec<Recipe> = byproduct
        .iter()
        .map(|(metal, by)| {
            let dust = format!("fras:{metal}_dust");
            recipe(
                "washing",
                &format!("crushed_{metal}"),
                item(&format!("fras:crushed_{metal}")),
                1,
                vec![out(&dust, 1, 1.0), out(&dust, 1, 0.3333), out(by, 1, 0.1)],
                2.5,
                4.0,
                50.0,
            )
        })
        .collect();
    v.push(recipe(
        "washing",
        "iron_nugget",
        item("minecraft:iron_nugget"),
        1,
        vec![out("fras:rust_flakes", 1, 1.0)],
        64.0,
        2.0,
        200.0,
    ));
    v
}

/// The trip hammer's recipes (machines spec 5.2).
pub fn hammering() -> Vec<Recipe> {
    vec![
        recipe(
            "hammering",
            "iron_ingot",
            item("minecraft:iron_ingot"),
            1,
            vec![out("fras:iron_plate", 1, 1.0)],
            2.0,
            8.0,
            0.0,
        ),
        recipe(
            "hammering",
            "copper_ingot",
            item("minecraft:copper_ingot"),
            1,
            vec![out("fras:copper_plate", 1, 1.0)],
            2.0,
            8.0,
            0.0,
        ),
        recipe(
            "hammering",
            "iron_rod",
            item("fras:iron_rod"),
            1,
            vec![out("fras:rivet", 8, 1.0)],
            3.0,
            8.0,
            0.0,
        ),
    ]
}

/// The lathe's recipes (machines spec 6.2).
pub fn turning() -> Vec<Recipe> {
    vec![
        recipe(
            "turning",
            "iron_ingot",
            item("minecraft:iron_ingot"),
            1,
            vec![out("fras:iron_rod", 2, 1.0)],
            8.0,
            4.0,
            0.0,
        ),
        recipe(
            "turning",
            "iron_plate",
            item("fras:iron_plate"),
            1,
            vec![out("fras:iron_gear", 1, 1.0)],
            12.0,
            4.0,
            0.0,
        ),
        recipe(
            "turning",
            "copper_plate",
            item("fras:copper_plate"),
            1,
            vec![out("fras:copper_pipe", 3, 1.0)],
            8.0,
            3.0,
            0.0,
        ),
    ]
}

/// The wood kinds the sawmill knows: (kind, its log-like blocks, its planks).
pub fn wood_kinds() -> Vec<(String, Vec<String>, String)> {
    let mut v = Vec::new();
    for kind in [
        "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
    ] {
        let logs = vec![
            format!("minecraft:{kind}_log"),
            format!("minecraft:stripped_{kind}_log"),
            format!("minecraft:{kind}_wood"),
            format!("minecraft:stripped_{kind}_wood"),
        ];
        v.push((kind.to_string(), logs, format!("minecraft:{kind}_planks")));
    }
    for kind in ["crimson", "warped"] {
        let logs = vec![
            format!("minecraft:{kind}_stem"),
            format!("minecraft:stripped_{kind}_stem"),
            format!("minecraft:{kind}_hyphae"),
            format!("minecraft:stripped_{kind}_hyphae"),
        ];
        v.push((kind.to_string(), logs, format!("minecraft:{kind}_planks")));
    }
    v
}

/// The sawmill's recipes (machines spec 7.2): one per log-like block, so a
/// log gives planks of its own kind.
pub fn sawing() -> Vec<Recipe> {
    let mut v = Vec::new();
    for (_, logs, planks) in wood_kinds() {
        for log in logs {
            let id = log.trim_start_matches("minecraft:").to_string();
            v.push(recipe(
                "sawing",
                &id,
                item(&log),
                1,
                vec![out(&planks, 6, 1.0)],
                4.0,
                6.0,
                0.0,
            ));
        }
    }
    v.push(recipe(
        "sawing",
        "bamboo_block",
        item("minecraft:bamboo_block"),
        1,
        vec![out("minecraft:bamboo_planks", 3, 1.0)],
        4.0,
        6.0,
        0.0,
    ));
    v.push(recipe(
        "sawing",
        "planks",
        Ingredient::Tag("minecraft:planks".into()),
        1,
        vec![out("minecraft:stick", 3, 1.0)],
        2.0,
        3.0,
        0.0,
    ));
    v
}

/// The built-in recipes of a machine.
pub fn builtin(machine: Machine) -> Vec<Recipe> {
    match machine {
        Machine::Grinder | Machine::InlineGrinder => grind::builtin(),
        Machine::Washer => washing(),
        Machine::TripHammer => hammering(),
        Machine::Lathe => turning(),
        Machine::Sawmill => sawing(),
        Machine::Trommel => Vec::new(),
    }
}

/// Default trommel classes (machines spec 8.3), as the tag files list them.
pub fn trommel_fine() -> Vec<String> {
    let mut v: Vec<String> = ["minecraft:sand", "minecraft:red_sand", "#fras:dusts"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    v.sort();
    v
}

pub fn trommel_medium() -> Vec<String> {
    [
        "#fras:crushed_ores",
        "minecraft:gravel",
        "minecraft:flint",
        "minecraft:iron_nugget",
        "minecraft:gold_nugget",
        "minecraft:copper_nugget",
        "minecraft:redstone",
        "minecraft:bone_meal",
        "minecraft:clay_ball",
        "minecraft:coal",
        "minecraft:charcoal",
        "fras:rust_flakes",
        "fras:red_oxide_pigment",
        "minecraft:raw_iron",
        "minecraft:raw_copper",
        "minecraft:raw_gold",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The washer's tank (machines spec 4.3): one content at a time, water or
/// hot water, both wash.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Tank {
    pub content: Option<crate::sim::fluid::Content>,
    pub amount: f64,
}

impl Tank {
    /// Whether a fluid washes.
    pub fn washes(content: crate::sim::fluid::Content) -> bool {
        use crate::sim::fluid::Content;
        matches!(content, Content::Water | Content::HotWater)
    }

    /// Fill up to `amount` of `content`; returns how much went in (none for
    /// a fluid that doesn't wash or differs from what the tank holds).
    pub fn fill(&mut self, content: crate::sim::fluid::Content, amount: f64) -> f64 {
        if !Tank::washes(content) || amount <= 0.0 {
            return 0.0;
        }
        if self.amount > 1e-9 && self.content.is_some_and(|c| c != content) {
            return 0.0;
        }
        let n = amount.min(WASHER_TANK - self.amount).max(0.0);
        if n > 0.0 {
            self.content = Some(content);
            self.amount += n;
        }
        n
    }

    /// Use `amount` (a finished recipe).
    pub fn drain(&mut self, amount: f64) {
        self.amount = (self.amount - amount).max(0.0);
        if self.amount <= 1e-9 {
            self.amount = 0.0;
            self.content = None;
        }
    }

    pub fn space(&self) -> f64 {
        (WASHER_TANK - self.amount).max(0.0)
    }
}

pub mod station;

#[cfg(test)]
mod tests;
