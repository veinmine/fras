//! The firebox's fuels (steam and heat spec 3.1): each item or tag has a heat
//! rate; burn time is always the game's. The table is data
//! (`data/fras/fras/firebox_fuels.json`), so data packs can add fuels.

/// One row: an item id or a tag (`#minecraft:logs`), and its heat rate in kW.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: String,
    pub heat: f64,
}

/// The fuel table.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub rows: Vec<Row>,
    /// The rate of a fuel the table doesn't list.
    pub default: f64,
}

/// What a fuel that isn't listed gives, kW.
pub const DEFAULT_HEAT: f64 = 750.0;

impl Default for Table {
    fn default() -> Self {
        builtin()
    }
}

impl Table {
    /// The heat rate of `item`: an exact row first, then the first tag row it
    /// is in (`has_tag(item, tag)`), else the default.
    pub fn heat(&self, item: &str, has_tag: &dyn Fn(&str, &str) -> bool) -> f64 {
        if let Some(r) = self.rows.iter().find(|r| r.key == item) {
            return r.heat;
        }
        self.rows
            .iter()
            .filter_map(|r| r.key.strip_prefix('#').map(|t| (t, r.heat)))
            .find(|(tag, _)| has_tag(item, tag))
            .map_or(self.default, |(_, h)| h)
    }

    /// The tags the table names, for looking them up once.
    pub fn tags(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter_map(|r| r.key.strip_prefix('#').map(str::to_string))
            .collect()
    }
}

/// Read the table's JSON: `{"default": 750, "fuels": [{"fuel": "minecraft:coal", "heat": 1000},
/// {"fuel": "#minecraft:logs", "heat": 500}]}`. Rows keep the file's order.
pub fn parse(text: &str) -> Result<Table, String> {
    let json: mod_api::serde_json::Value =
        mod_api::serde_json::from_str(text).map_err(|e| e.to_string())?;
    let default = json["default"].as_f64().unwrap_or(DEFAULT_HEAT);
    let fuels = json["fuels"].as_array().ok_or("no \"fuels\" list")?;
    let mut rows = Vec::new();
    for v in fuels {
        let key = v["fuel"]
            .as_str()
            .ok_or("a row without \"fuel\"")?
            .to_string();
        let heat = v["heat"]
            .as_f64()
            .ok_or_else(|| format!("{key}: no heat"))?;
        if heat < 0.0 {
            return Err(format!("{key}: negative heat"));
        }
        rows.push(Row { key, heat });
    }
    Ok(Table { rows, default })
}

/// The table the mod ships (the same as the data file).
pub fn builtin() -> Table {
    let rows = [
        ("minecraft:coal", 1000.0),
        ("minecraft:charcoal", 1000.0),
        ("minecraft:coal_block", 1000.0),
        ("minecraft:blaze_rod", 2000.0),
        ("minecraft:lava_bucket", 2000.0),
        ("minecraft:dried_kelp_block", 750.0),
        ("minecraft:stick", 500.0),
        ("#minecraft:logs_that_burn", 500.0),
        ("#minecraft:planks", 500.0),
        ("#minecraft:wooden_slabs", 500.0),
        ("#minecraft:wooden_stairs", 500.0),
        ("#minecraft:wooden_fences", 500.0),
        ("#minecraft:fence_gates", 500.0),
        ("#minecraft:wooden_doors", 500.0),
        ("#minecraft:wooden_trapdoors", 500.0),
        ("#minecraft:wooden_pressure_plates", 500.0),
        ("#minecraft:wooden_buttons", 500.0),
        ("#minecraft:saplings", 500.0),
        ("#minecraft:bamboo_blocks", 500.0),
    ];
    Table {
        rows: rows
            .into_iter()
            .map(|(k, h)| Row {
                key: k.into(),
                heat: h,
            })
            .collect(),
        default: DEFAULT_HEAT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_items_then_tags_then_the_default() {
        let t = builtin();
        let tags = |item: &str, tag: &str| {
            item == "minecraft:oak_log" && tag == "minecraft:logs_that_burn"
        };
        assert_eq!(t.heat("minecraft:coal", &tags), 1000.0);
        assert_eq!(t.heat("minecraft:oak_log", &tags), 500.0);
        assert_eq!(t.heat("minecraft:blaze_rod", &tags), 2000.0);
        assert_eq!(t.heat("minecraft:something_else", &tags), 750.0);
    }

    #[test]
    fn the_data_file_matches_the_builtin_table() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/fras/fras/firebox_fuels.json"
        );
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(parse(&text).unwrap(), builtin());
        assert!(parse("{}").is_err());
    }
}
