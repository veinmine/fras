//! FRAS's blocks: which network part each is, how its block state says how
//! it sits, and what its block entity saves.

use mod_api::{Compound, Tag};

use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::rot::{BeltEnd, Jam, Kind, Material, Saved, Shape, Spec};

/// Every block of the mod, each with a block entity type of its own (spike
/// lesson 1: hooks name the type, not the block).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    ShaftWood,
    ShaftIron,
    Bearing,
    Clutch,
    Pulley,
    LargePulley,
    BevelGearWood,
    BevelGearIron,
    Gearbox,
    Crank,
    WaterWheel,
    Grinder,
    Tachometer,
}

impl Block {
    pub const ALL: [Block; 13] = [
        Block::ShaftWood,
        Block::ShaftIron,
        Block::Bearing,
        Block::Clutch,
        Block::Pulley,
        Block::LargePulley,
        Block::BevelGearWood,
        Block::BevelGearIron,
        Block::Gearbox,
        Block::Crank,
        Block::WaterWheel,
        Block::Grinder,
        Block::Tachometer,
    ];

    /// The path of its id (`fras:<name>`).
    pub fn name(self) -> &'static str {
        match self {
            Block::ShaftWood => "wooden_shaft",
            Block::ShaftIron => "iron_shaft",
            Block::Bearing => "bearing",
            Block::Clutch => "clutch",
            Block::Pulley => "pulley",
            Block::LargePulley => "large_pulley",
            Block::BevelGearWood => "wooden_bevel_gear",
            Block::BevelGearIron => "iron_bevel_gear",
            Block::Gearbox => "gearbox",
            Block::Crank => "crank",
            Block::WaterWheel => "water_wheel",
            Block::Grinder => "grinder",
            Block::Tachometer => "tachometer",
        }
    }

    pub fn id(self) -> String {
        format!("fras:{}", self.name())
    }

    pub fn from_id(id: &str) -> Option<Block> {
        let name = id.strip_prefix("fras:")?;
        Block::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The network part it is; the tachometer is none.
    pub fn kind(self) -> Option<Kind> {
        Some(match self {
            Block::ShaftWood => Kind::Shaft(Material::Wood),
            Block::ShaftIron => Kind::Shaft(Material::Iron),
            Block::Bearing => Kind::Bearing,
            Block::Clutch => Kind::Clutch,
            Block::Pulley => Kind::Pulley { large: false },
            Block::LargePulley => Kind::Pulley { large: true },
            Block::BevelGearWood => Kind::BevelGear(Material::Wood),
            Block::BevelGearIron => Kind::BevelGear(Material::Iron),
            Block::Gearbox => Kind::Gearbox,
            Block::Crank => Kind::Crank,
            Block::WaterWheel => Kind::WaterWheel,
            Block::Grinder => Kind::Grinder,
            Block::Tachometer => return None,
        })
    }

    /// Whether its block entity ticks: sources, machines and the tachometer
    /// only (shafts don't tick: spike risk 1).
    pub fn ticks(self) -> bool {
        matches!(
            self,
            Block::Crank | Block::WaterWheel | Block::Grinder | Block::Tachometer
        )
    }

    pub fn is_wooden(self) -> bool {
        matches!(
            self,
            Block::ShaftWood
                | Block::Pulley
                | Block::LargePulley
                | Block::BevelGearWood
                | Block::Crank
                | Block::WaterWheel
                | Block::Grinder
        )
    }

    /// How it sits: from its block state's properties (`get` reads one).
    pub fn shape(self, get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
        let axis = || {
            get("axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::Y)
        };
        let dir = |key: &str, default: Dir| {
            get(key)
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(default)
        };
        Some(match self {
            Block::ShaftWood
            | Block::ShaftIron
            | Block::Bearing
            | Block::Clutch
            | Block::Pulley
            | Block::LargePulley
            | Block::WaterWheel => Shape::Inline(axis()),
            Block::BevelGearWood | Block::BevelGearIron => {
                let input = dir("input", Dir::West);
                let mut output = dir("output", Dir::South);
                if output.axis() == input.axis() {
                    output = input.perpendicular().next().unwrap();
                }
                Shape::Bevel {
                    input,
                    output,
                    tee: get("tee").as_deref() == Some("true"),
                }
            }
            Block::Gearbox => {
                let slow = dir("facing", Dir::North);
                Shape::Gearbox {
                    axis: slow.axis(),
                    slow,
                }
            }
            Block::Crank | Block::Grinder => Shape::Socket(dir("facing", Dir::North)),
            Block::Tachometer => return None,
        })
    }

    /// The part's spec from its block state and saved data.
    pub fn spec(self, get: &dyn Fn(&str) -> Option<String>, nbt: &Compound) -> Option<Spec> {
        let mut spec = Spec::new(self.kind()?, self.shape(get)?);
        if self == Block::Clutch {
            spec.engaged = get("powered").as_deref() != Some("true");
        }
        if matches!(self, Block::Pulley | Block::LargePulley) {
            spec.belt = belt(nbt);
        }
        Some(spec)
    }
}

// ---- Saved data ------------------------------------------------------------

pub fn get_f64(nbt: &Compound, key: &str) -> Option<f64> {
    match nbt.get(&key.into()) {
        Some(Tag::Double(bits)) => Some(f64::from_bits(*bits)),
        Some(Tag::Float(bits)) => Some(f32::from_bits(*bits) as f64),
        Some(Tag::Int(v)) => Some(*v as f64),
        _ => None,
    }
}

pub fn get_i32(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(*v as i32),
        Some(Tag::Short(v)) => Some(*v as i32),
        _ => None,
    }
}

pub fn get_str(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}

pub fn get_bool(nbt: &Compound, key: &str) -> bool {
    get_i32(nbt, key).is_some_and(|v| v != 0)
}

pub fn put_f64(nbt: &mut Compound, key: &str, v: f64) {
    nbt.insert(key.into(), Tag::Double(v.to_bits()));
}

pub fn put_i32(nbt: &mut Compound, key: &str, v: i32) {
    nbt.insert(key.into(), Tag::Int(v));
}

pub fn put_str(nbt: &mut Compound, key: &str, v: &str) {
    nbt.insert(key.into(), Tag::String(v.into()));
}

pub fn put_bool(nbt: &mut Compound, key: &str, v: bool) {
    nbt.insert(key.into(), Tag::Byte(v as i8));
}

/// A pulley's belt: its partner's position and whether it is crossed.
pub fn belt(nbt: &Compound) -> Option<BeltEnd> {
    let x = get_i32(nbt, "belt_x")?;
    let y = get_i32(nbt, "belt_y")?;
    let z = get_i32(nbt, "belt_z")?;
    Some(BeltEnd {
        partner: Pos::new(x, y, z),
        crossed: get_bool(nbt, "belt_crossed"),
    })
}

pub fn put_belt(nbt: &mut Compound, end: Option<BeltEnd>) {
    match end {
        Some(end) => {
            put_i32(nbt, "belt_x", end.partner.x);
            put_i32(nbt, "belt_y", end.partner.y);
            put_i32(nbt, "belt_z", end.partner.z);
            put_bool(nbt, "belt_crossed", end.crossed);
        }
        None => {
            for key in [
                "belt_x",
                "belt_y",
                "belt_z",
                "belt_crossed",
                "belt_big",
                "belt_cost",
            ] {
                nbt.remove(&key.into());
            }
        }
    }
}

/// What a part saved: speed, jam and timers. `None` for a freshly placed
/// block (it has never been written: no `v`).
pub fn saved(nbt: &Compound) -> Option<Saved> {
    get_i32(nbt, "v")?;
    Some(Saved {
        omega: get_f64(nbt, "rpm").unwrap_or(0.0),
        jam: get_str(nbt, "jam").as_deref().and_then(Jam::from_name),
        over_t: get_f64(nbt, "over_t").unwrap_or(0.0),
        shear_t: get_f64(nbt, "shear_t").unwrap_or(0.0),
    })
}

/// The fields a part saves and clients read (spec 10.5, 10.6).
pub fn part_fields(
    rpm: f64,
    jam: Option<Jam>,
    warn: u8,
    sag: bool,
    over_t: f64,
    shear_t: f64,
) -> Compound {
    let mut c = Compound::new();
    put_i32(&mut c, "v", 1);
    put_f64(&mut c, "rpm", rpm);
    put_str(&mut c, "jam", jam.map_or("", Jam::name));
    put_i32(&mut c, "warn", warn as i32);
    put_bool(&mut c, "sag", sag);
    put_f64(&mut c, "over_t", over_t);
    put_f64(&mut c, "shear_t", shear_t);
    c
}

/// The keys clients get (`block_entity_update_tag`).
pub const CLIENT_KEYS: &[&str] = &[
    "rpm",
    "jam",
    "warn",
    "sag",
    "belt_x",
    "belt_y",
    "belt_z",
    "belt_crossed",
    "belt_big",
    "grinding",
    "input",
    "needle",
    "red",
    "yellow",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for b in Block::ALL {
            assert_eq!(Block::from_id(&b.id()), Some(b));
        }
        assert_eq!(Block::from_id("minecraft:stone"), None);
    }

    #[test]
    fn shapes_come_from_the_block_state() {
        let props = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let get = props(&[("axis", "x")]);
        assert_eq!(Block::ShaftWood.shape(&get), Some(Shape::Inline(Axis::X)));
        let get = props(&[("facing", "east")]);
        assert_eq!(
            Block::Gearbox.shape(&get),
            Some(Shape::Gearbox {
                axis: Axis::X,
                slow: Dir::East
            })
        );
        let get = props(&[("powered", "true"), ("axis", "z")]);
        let spec = Block::Clutch.spec(&get, &Compound::new()).unwrap();
        assert!(!spec.engaged);
        // A bevel gear whose output lies on its input's axis is corrected.
        let get = props(&[("input", "up"), ("output", "down")]);
        match Block::BevelGearWood.shape(&get) {
            Some(Shape::Bevel { input, output, .. }) => assert_ne!(input.axis(), output.axis()),
            other => panic!("{other:?}"),
        }
        assert_eq!(Block::Tachometer.shape(&get), None);
    }

    #[test]
    fn saved_data_round_trips() {
        let mut nbt = part_fields(12.5, Some(Jam::Shear), 2, true, 0.25, 0.5);
        let s = saved(&nbt).unwrap();
        assert_eq!(s.omega, 12.5);
        assert_eq!(s.jam, Some(Jam::Shear));
        assert_eq!(s.shear_t, 0.5);
        assert_eq!(saved(&Compound::new()), None);
        put_belt(
            &mut nbt,
            Some(BeltEnd {
                partner: Pos::new(1, -2, 3),
                crossed: true,
            }),
        );
        assert_eq!(
            belt(&nbt),
            Some(BeltEnd {
                partner: Pos::new(1, -2, 3),
                crossed: true
            })
        );
        put_belt(&mut nbt, None);
        assert_eq!(belt(&nbt), None);
    }
}
