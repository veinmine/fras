//! The recipe-tree test (progression spec 8): reads the generated recipe
//! and tag files and checks that every tier-1 item is craftable from vanilla
//! materials, that the hand-hammered layer needs no lathe or trip hammer
//! part, and that the gated recipes (progression 4.2) need one.

use std::collections::{BTreeMap, BTreeSet};

use mod_api::serde_json::Value;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// A recipe as the tree sees it: what it needs and what it makes.
struct Node {
    _id: String,
    needs: Vec<String>,
    makes: String,
}

fn read_json(path: &std::path::Path) -> Value {
    mod_api::serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

fn ingredient(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => vec![
            a.iter()
                .filter_map(Value::as_str)
                .next()
                .unwrap_or("")
                .to_owned(),
        ],
        Value::Object(o) => o
            .get("item")
            .or_else(|| o.get("id"))
            .and_then(Value::as_str)
            .map(|s| vec![s.to_owned()])
            .or_else(|| {
                o.get("tag")
                    .and_then(Value::as_str)
                    .map(|t| vec![format!("#{t}")])
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn machine_of(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "fras:grinding" => "fras:grinder",
        "fras:washing" => "fras:washer",
        "fras:hammering" => "fras:trip_hammer",
        "fras:turning" => "fras:lathe",
        "fras:sawing" => "fras:sawmill",
        _ => return None,
    })
}

fn nodes() -> Vec<Node> {
    let mut files = Vec::new();
    walk(
        &std::path::Path::new(ROOT).join("data/fras/recipe"),
        &mut files,
    );
    let mut out = Vec::new();
    for f in files {
        let j = read_json(&f);
        let id = f.strip_prefix(ROOT).unwrap().display().to_string();
        let kind = j["type"].as_str().unwrap_or_default();
        let mut needs = Vec::new();
        let results: Vec<String> = match kind {
            "minecraft:crafting_shaped" => {
                for v in j["key"].as_object().unwrap().values() {
                    needs.extend(ingredient(v));
                }
                vec![j["result"]["id"].as_str().unwrap().to_owned()]
            }
            "minecraft:crafting_shapeless" => {
                for v in j["ingredients"].as_array().unwrap() {
                    needs.extend(ingredient(v));
                }
                vec![j["result"]["id"].as_str().unwrap().to_owned()]
            }
            "minecraft:smelting" | "minecraft:blasting" => {
                needs.extend(ingredient(&j["ingredient"]));
                vec![j["result"]["id"].as_str().unwrap().to_owned()]
            }
            k if machine_of(k).is_some() => {
                needs.extend(ingredient(&j["ingredient"]));
                needs.push(machine_of(k).unwrap().to_owned());
                j["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["chance"].as_f64().unwrap_or(1.0) >= 1.0)
                    .map(|r| r["id"].as_str().unwrap().to_owned())
                    .collect()
            }
            _ => continue,
        };
        // A hand hammer used in a recipe comes back: it is a tool.
        for makes in results {
            out.push(Node {
                _id: id.clone(),
                needs: needs.clone(),
                makes,
            });
        }
    }
    out
}

fn tags() -> BTreeMap<String, Vec<String>> {
    let mut files = Vec::new();
    walk(
        &std::path::Path::new(ROOT).join("data/fras/tags/item"),
        &mut files,
    );
    let base = std::path::Path::new(ROOT).join("data/fras/tags/item");
    let mut out = BTreeMap::new();
    for f in files {
        let name = f
            .strip_prefix(&base)
            .unwrap()
            .with_extension("")
            .display()
            .to_string();
        let j = read_json(&f);
        let values = j["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        out.insert(format!("#fras:{name}"), values);
    }
    out
}

/// Everything reachable from vanilla materials, leaving out recipes that
/// make anything in `banned`.
fn reachable(banned: &[&str]) -> BTreeSet<String> {
    let nodes = nodes();
    let tags = tags();
    let mut have: BTreeSet<String> = BTreeSet::new();
    let has = |have: &BTreeSet<String>, need: &str| -> bool {
        if need.starts_with("minecraft:")
            || need.starts_with("#minecraft:")
            || need.starts_with("#c:")
        {
            return true;
        }
        if let Some(values) = tags.get(need) {
            return values.iter().any(|v| {
                v.starts_with("minecraft:")
                    || v.starts_with("#minecraft:")
                    || have.contains(v)
                    || (v.starts_with('#')
                        && tags
                            .get(v)
                            .is_some_and(|vv| vv.iter().any(|x| have.contains(x))))
            });
        }
        have.contains(need)
    };
    loop {
        let before = have.len();
        for n in &nodes {
            if banned.contains(&n.makes.as_str()) || have.contains(&n.makes) {
                continue;
            }
            if n.needs.iter().all(|need| has(&have, need)) {
                have.insert(n.makes.clone());
            }
        }
        if have.len() == before {
            break;
        }
    }
    have
}

#[test]
fn every_tier_one_item_is_craftable() {
    let have = reachable(&[]);
    let creative: BTreeSet<&str> = [
        "creative_engine",
        "load_bank",
        "creative_steam_source",
        "creative_water_source",
        "creative_drain",
        "creative_firebox",
    ]
    .into();
    let mut missing = Vec::new();
    for b in super::blocks::Block::ALL {
        if creative.contains(b.name()) {
            continue;
        }
        let id = format!("fras:{}", b.name());
        if !have.contains(&id) {
            missing.push(id);
        }
    }
    for i in crate::ITEMS {
        let id = format!("fras:{i}");
        if !have.contains(&id) {
            missing.push(id);
        }
    }
    for p in crate::PAINTED {
        let id = format!("fras:painted_{p}");
        if !have.contains(&id) {
            missing.push(id);
        }
    }
    assert!(missing.is_empty(), "not craftable: {missing:?}");
}

#[test]
fn the_factory_gates_the_better_blocks() {
    let parts = ["fras:iron_rod", "fras:iron_gear", "fras:rivet"];
    let without = reachable(&parts);
    // The hand-hammered layer and the first engine need no machine part.
    for id in [
        "fras:lathe",
        "fras:trip_hammer",
        "fras:conveyor_belt",
        "fras:diverter",
        "fras:iron_pulley",
        "fras:large_iron_pulley",
        "fras:iron_shaft",
        "fras:steam_cylinder",
        "fras:flywheel",
        "fras:pump",
        "fras:grinder",
        "fras:copper_boiler",
        "fras:iron_pipe",
    ] {
        assert!(without.contains(id), "{id} needs a machine part");
    }
    // Progression 4.2 and 4.3: these need one.
    for id in [
        "fras:gearbox",
        "fras:iron_bevel_gear",
        "fras:clutch",
        "fras:governor",
        "fras:furnace_damper",
        "fras:iron_boiler",
        "fras:storage_tank",
        "fras:heat_exchanger",
        "fras:tachometer",
        "fras:bucket_elevator",
        "fras:washer",
        "fras:sawmill",
        "fras:trommel",
    ] {
        assert!(
            !without.contains(id),
            "{id} is craftable without a machine part"
        );
    }
}

#[test]
fn the_machine_recipe_files_match_the_built_in_tables() {
    use crate::sim::machine::{self, Machine};
    for m in [
        Machine::Washer,
        Machine::TripHammer,
        Machine::Lathe,
        Machine::Sawmill,
    ] {
        let kind = m.recipe_type().unwrap();
        let dir = std::path::Path::new(ROOT)
            .join("data/fras/recipe")
            .join(kind.trim_start_matches("fras:"));
        let mut found = Vec::new();
        for e in std::fs::read_dir(&dir).unwrap() {
            let path = e.unwrap().path();
            let json = read_json(&path);
            let id = format!("{kind}/{}", path.file_stem().unwrap().to_str().unwrap());
            found.push(
                super::grinder::parse_for(&id, &json, m.default_load(), m.default_water()).unwrap(),
            );
        }
        found.sort_by(|a, b| a.id.cmp(&b.id));
        let mut builtin = machine::builtin(m);
        builtin.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(found, builtin, "{kind}");
    }
}

#[test]
fn the_trommel_tags_match_the_built_in_classes() {
    let tags = tags();
    let mut fine = tags["#fras:trommel/fine"].clone();
    fine.sort();
    assert_eq!(fine, crate::sim::machine::trommel_fine());
    assert_eq!(
        tags["#fras:trommel/medium"],
        crate::sim::machine::trommel_medium()
    );
}

/// A crafting recipe as the clash test sees it: a grid of slots (`None` is
/// empty) or a bag of slots, each slot the set of items that fit it.
enum Grid {
    Shaped(Vec<Vec<Option<BTreeSet<String>>>>),
    Shapeless(Vec<BTreeSet<String>>),
}

/// Every item an ingredient (an item, a tag or a list of them) takes, with
/// tags looked up in `tags` (FRAS's own and vanilla's, already expanded).
fn slot(v: &Value, tags: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let one = |s: &str| -> BTreeSet<String> {
        match s.strip_prefix('#') {
            Some(_) => tags
                .get(s)
                .unwrap_or_else(|| panic!("unknown tag {s}"))
                .clone(),
            None => [s.to_owned()].into(),
        }
    };
    match v {
        Value::Array(a) => a.iter().flat_map(|x| slot(x, tags)).collect(),
        _ => ingredient(v).iter().flat_map(|s| one(s)).collect(),
    }
}

fn grid(j: &Value, tags: &BTreeMap<String, BTreeSet<String>>) -> Option<Grid> {
    match j["type"].as_str() {
        // Vanilla's file has no type; it has a pattern or ingredients.
        Some("minecraft:crafting_shaped") | None if j.get("pattern").is_some() => {
            let key = j["key"].as_object().unwrap();
            let rows: Vec<&str> = j["pattern"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect();
            let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            Some(Grid::Shaped(
                rows.iter()
                    .map(|r| {
                        (0..width)
                            .map(|x| match r.as_bytes().get(x) {
                                None | Some(b' ') => None,
                                Some(&c) => Some(slot(&key[&(c as char).to_string()], tags)),
                            })
                            .collect()
                    })
                    .collect(),
            ))
        }
        Some("minecraft:crafting_shapeless") | None => Some(Grid::Shapeless(
            j["ingredients"]
                .as_array()?
                .iter()
                .map(|v| slot(v, tags))
                .collect(),
        )),
        _ => None,
    }
}

/// Whether every slot of `a` can take a different slot of `b` (a perfect
/// matching, by augmenting paths).
fn bags_overlap(a: &[&BTreeSet<String>], b: &[&BTreeSet<String>]) -> bool {
    fn place(
        i: usize,
        a: &[&BTreeSet<String>],
        b: &[&BTreeSet<String>],
        seen: &mut [bool],
        owner: &mut [Option<usize>],
    ) -> bool {
        for j in 0..b.len() {
            if !seen[j] && !a[i].is_disjoint(b[j]) {
                seen[j] = true;
                if owner[j].is_none_or(|k| place(k, a, b, seen, owner)) {
                    owner[j] = Some(i);
                    return true;
                }
            }
        }
        false
    }
    let mut owner = vec![None; b.len()];
    a.len() == b.len()
        && (0..a.len()).all(|i| place(i, a, b, &mut vec![false; b.len()], &mut owner))
}

/// Whether some filled crafting grid matches both recipes. A shaped recipe
/// also matches mirrored, as in vanilla.
fn clash(a: &Grid, b: &Grid) -> bool {
    let cells = |g: &Grid| -> Vec<BTreeSet<String>> {
        match g {
            Grid::Shaped(rows) => rows.iter().flatten().flatten().cloned().collect(),
            Grid::Shapeless(slots) => slots.clone(),
        }
    };
    match (a, b) {
        (Grid::Shaped(a), Grid::Shaped(b)) => {
            if a.len() != b.len() || a[0].len() != b[0].len() {
                return false;
            }
            let fits = |mirror: bool| {
                a.iter().zip(b).all(|(ra, rb)| {
                    let rb: Vec<_> = if mirror {
                        rb.iter().rev().collect()
                    } else {
                        rb.iter().collect()
                    };
                    ra.iter().zip(rb).all(|(x, y)| match (x, y) {
                        (None, None) => true,
                        (Some(x), Some(y)) => !x.is_disjoint(y),
                        _ => false,
                    })
                })
            };
            fits(false) || fits(true)
        }
        _ => {
            let (ca, cb) = (cells(a), cells(b));
            bags_overlap(
                &ca.iter().collect::<Vec<_>>(),
                &cb.iter().collect::<Vec<_>>(),
            )
        }
    }
}

/// Named crafting recipes.
type Grids = Vec<(String, Grid)>;

/// FRAS's crafting recipes and vanilla's (tools/data/vanilla-crafting.json,
/// written by tools/vanilla_recipes.py), with all tags expanded.
fn crafting_grids() -> (Grids, Grids) {
    let vanilla = read_json(&std::path::Path::new(ROOT).join("tools/data/vanilla-crafting.json"));
    let mut all: BTreeMap<String, BTreeSet<String>> = vanilla["tags"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| {
            let items = v
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_str().unwrap().to_owned());
            (format!("#{k}"), items.collect())
        })
        .collect();
    // FRAS's tags may name vanilla tags and each other: expand until stable.
    let fras = tags();
    loop {
        let mut changed = false;
        for (name, values) in &fras {
            let mut items = BTreeSet::new();
            for v in values {
                match v.strip_prefix('#') {
                    Some(_) => items.extend(all.get(v).cloned().unwrap_or_default()),
                    None => {
                        items.insert(v.clone());
                    }
                }
            }
            if all.get(name) != Some(&items) {
                all.insert(name.clone(), items);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut files = Vec::new();
    walk(
        &std::path::Path::new(ROOT).join("data/fras/recipe"),
        &mut files,
    );
    files.sort();
    let ours = files
        .iter()
        .filter_map(|f| {
            let name = f.file_stem().unwrap().to_str().unwrap();
            grid(&read_json(f), &all).map(|g| (format!("fras:{name}"), g))
        })
        .collect();
    let theirs = vanilla["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| grid(r, &all).map(|g| (r["id"].as_str().unwrap().to_owned(), g)))
        .collect();
    (ours, theirs)
}

#[test]
fn no_crafting_recipe_clashes_with_vanilla_or_another() {
    let (ours, theirs) = crafting_grids();
    assert!(theirs.len() > 1000, "vanilla's recipes didn't load");
    let mut clashes = Vec::new();
    for (i, (a, ga)) in ours.iter().enumerate() {
        for (b, gb) in ours[i + 1..].iter().chain(&theirs) {
            if clash(ga, gb) {
                clashes.push(format!("{a} / {b}"));
            }
        }
    }
    assert!(clashes.is_empty(), "same grid: {clashes:?}");
}
