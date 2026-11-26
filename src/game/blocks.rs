//! FRAS's blocks: which network part each is, how its block state says how
//! it sits, and what its block entity saves.

use mod_api::{Compound, Tag};

use crate::sim::fluid::Content;
use crate::sim::geom::{Axis, Dir, Pos};
use crate::sim::multi::shape::Cuboid;
use crate::sim::plant::{Kind as PlantKind, Saved as PlantSaved, Spec as PlantSpec};
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
    // ---- 0.2: steam, fluids, multiblocks ----
    CopperBoiler,
    IronBoiler,
    StorageTank,
    SteamDome,
    Firebox,
    CopperPipe,
    IronPipe,
    ManualValve,
    RedstoneValve,
    SteamRelease,
    EmergencyValve,
    Pump,
    SteamCylinder,
    Flywheel,
    PressureGauge,
    SightGlass,
    // ---- 0.3: regulation and cooling ----
    Governor,
    FurnaceDamper,
    Bellows,
    HeatExchanger,
    CopperRadiator,
    IronRadiator,
    Drain,
    // ---- Creative (debugging) ----
    CreativeEngine,
    LoadBank,
    CreativeSteam,
    CreativeWater,
    CreativeDrain,
    CreativeFirebox,
}

impl Block {
    pub const ALL: [Block; 42] = [
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
        Block::CopperBoiler,
        Block::IronBoiler,
        Block::StorageTank,
        Block::SteamDome,
        Block::Firebox,
        Block::CopperPipe,
        Block::IronPipe,
        Block::ManualValve,
        Block::RedstoneValve,
        Block::SteamRelease,
        Block::EmergencyValve,
        Block::Pump,
        Block::SteamCylinder,
        Block::Flywheel,
        Block::PressureGauge,
        Block::SightGlass,
        Block::Governor,
        Block::FurnaceDamper,
        Block::Bellows,
        Block::HeatExchanger,
        Block::CopperRadiator,
        Block::IronRadiator,
        Block::Drain,
        Block::CreativeEngine,
        Block::LoadBank,
        Block::CreativeSteam,
        Block::CreativeWater,
        Block::CreativeDrain,
        Block::CreativeFirebox,
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
            Block::CopperBoiler => "copper_boiler",
            Block::IronBoiler => "iron_boiler",
            Block::StorageTank => "storage_tank",
            Block::SteamDome => "steam_dome",
            Block::Firebox => "firebox",
            Block::CopperPipe => "copper_pipe",
            Block::IronPipe => "iron_pipe",
            Block::ManualValve => "manual_valve",
            Block::RedstoneValve => "redstone_valve",
            Block::SteamRelease => "steam_release",
            Block::EmergencyValve => "emergency_valve",
            Block::Pump => "pump",
            Block::SteamCylinder => "steam_cylinder",
            Block::Flywheel => "flywheel",
            Block::PressureGauge => "pressure_gauge",
            Block::SightGlass => "sight_glass",
            Block::Governor => "governor",
            Block::FurnaceDamper => "furnace_damper",
            Block::Bellows => "bellows",
            Block::HeatExchanger => "heat_exchanger",
            Block::CopperRadiator => "copper_radiator",
            Block::IronRadiator => "iron_radiator",
            Block::Drain => "drain",
            Block::CreativeEngine => "creative_engine",
            Block::LoadBank => "load_bank",
            Block::CreativeSteam => "creative_steam_source",
            Block::CreativeWater => "creative_water_source",
            Block::CreativeDrain => "creative_drain",
            Block::CreativeFirebox => "creative_firebox",
        }
    }

    pub fn id(self) -> String {
        format!("fras:{}", self.name())
    }

    pub fn from_id(id: &str) -> Option<Block> {
        let name = id.strip_prefix("fras:")?;
        Block::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The rotational network part it is; the tachometer and most steam
    /// blocks are none.
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
            Block::Flywheel => Kind::Flywheel,
            Block::Pump => Kind::Pump,
            Block::Governor => Kind::Governor,
            Block::Bellows => Kind::Bellows,
            Block::CreativeEngine => Kind::Motor,
            Block::LoadBank => Kind::LoadBank,
            _ => return None,
        })
    }

    /// Whether its block entity ticks: sources, machines and the tachometer
    /// only (shafts don't tick: spike risk 1).
    pub fn ticks(self) -> bool {
        matches!(
            self,
            Block::Crank
                | Block::WaterWheel
                | Block::Grinder
                | Block::Tachometer
                | Block::Firebox
                | Block::PressureGauge
                | Block::SightGlass
                | Block::Governor
                | Block::FurnaceDamper
                | Block::EmergencyValve
        ) || self.is_creative()
    }

    /// The creative-only debugging blocks (no recipes).
    pub fn is_creative(self) -> bool {
        matches!(
            self,
            Block::CreativeEngine
                | Block::LoadBank
                | Block::CreativeSteam
                | Block::CreativeWater
                | Block::CreativeDrain
                | Block::CreativeFirebox
        )
    }

    /// Whether right-clicking it (with anything but the wrench) opens its
    /// screen.
    pub fn has_screen(self) -> bool {
        matches!(
            self,
            Block::Grinder
                | Block::Firebox
                | Block::Governor
                | Block::Tachometer
                | Block::PressureGauge
                | Block::SightGlass
                | Block::FurnaceDamper
                | Block::EmergencyValve
        ) || self.is_creative()
    }

    /// Whether it is a part of the steam plant (spec 2 to 4).
    pub fn is_plant(self) -> bool {
        self.plant_kind().is_some()
    }

    /// The steam plant part it is.
    pub fn plant_kind(self) -> Option<PlantKind> {
        use crate::sim::multi::ShellKind;
        use crate::sim::plant::PipeMat;
        Some(match self {
            Block::CopperBoiler => PlantKind::Shell(ShellKind::CopperBoiler),
            Block::IronBoiler => PlantKind::Shell(ShellKind::IronBoiler),
            Block::StorageTank => PlantKind::Shell(ShellKind::Tank),
            Block::SteamDome => PlantKind::Dome,
            Block::Firebox => PlantKind::Firebox,
            Block::CopperPipe => PlantKind::Pipe(PipeMat::Copper),
            Block::IronPipe => PlantKind::Pipe(PipeMat::Iron),
            Block::ManualValve => PlantKind::Valve { redstone: false },
            Block::RedstoneValve => PlantKind::Valve { redstone: true },
            Block::SteamRelease => PlantKind::Vent { emergency: false },
            Block::EmergencyValve => PlantKind::Vent { emergency: true },
            Block::Pump => PlantKind::Pump,
            Block::SteamCylinder => PlantKind::Cylinder,
            Block::Flywheel => PlantKind::Flywheel,
            Block::Governor => PlantKind::Governor,
            Block::FurnaceDamper => PlantKind::Damper,
            Block::Bellows => PlantKind::Bellows,
            Block::HeatExchanger => PlantKind::Exchanger,
            Block::CopperRadiator => PlantKind::Radiator { copper: true },
            Block::IronRadiator => PlantKind::Radiator { copper: false },
            Block::Drain => PlantKind::Drain,
            Block::Grinder => PlantKind::Grinder,
            Block::CreativeSteam | Block::CreativeWater => PlantKind::Source,
            Block::CreativeDrain => PlantKind::Void,
            Block::CreativeFirebox => PlantKind::Firebox,
            _ => return None,
        })
    }

    /// Whether it rusts (rust spec 3): iron parts.
    pub fn rusts(self) -> bool {
        matches!(
            self,
            Block::ShaftIron
                | Block::Bearing
                | Block::Clutch
                | Block::BevelGearIron
                | Block::Gearbox
                | Block::IronPipe
                | Block::ManualValve
                | Block::RedstoneValve
                | Block::SteamRelease
                | Block::EmergencyValve
                | Block::Governor
                | Block::IronBoiler
                | Block::StorageTank
                | Block::SteamCylinder
                | Block::Flywheel
                | Block::Pump
                | Block::HeatExchanger
                | Block::IronRadiator
                | Block::FurnaceDamper
        )
    }

    /// Its rust stage from its block state (clean for parts that don't rust).
    pub fn rust_stage(self, get: &dyn Fn(&str) -> Option<String>) -> crate::sim::rust::Stage {
        use crate::sim::rust::Stage;
        if !self.rusts() {
            return Stage::Clean;
        }
        get("rust")
            .as_deref()
            .and_then(Stage::from_name)
            .unwrap_or(Stage::Clean)
    }

    /// Whether the plant draws it in `steam_draw` (the steam parts that move
    /// or show a reading); the grinder is a plant part but drawn as a machine.
    pub fn steam_drawn(self) -> bool {
        (self.is_plant() && self != Block::Grinder && !self.is_creative())
            || matches!(self, Block::PressureGauge | Block::SightGlass)
    }

    pub fn is_shell(self) -> bool {
        matches!(
            self,
            Block::CopperBoiler | Block::IronBoiler | Block::StorageTank
        )
    }

    pub fn is_pipe(self) -> bool {
        matches!(self, Block::CopperPipe | Block::IronPipe)
    }

    /// The plant part's spec from its block state and saved data.
    pub fn plant_spec(
        self,
        get: &dyn Fn(&str) -> Option<String>,
        nbt: &Compound,
    ) -> Option<PlantSpec> {
        let kind = self.plant_kind()?;
        let flag = |key: &str| get(key).as_deref() == Some("true");
        let dir = |key: &str, default: Dir| {
            get(key)
                .as_deref()
                .and_then(Dir::from_name)
                .unwrap_or(default)
        };
        let axis = || {
            get("axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X)
        };
        let mut spec = PlantSpec::new(kind);
        spec.damaged = flag("damaged");
        match self {
            Block::CopperPipe | Block::IronPipe => {
                let faces: Vec<Dir> = Dir::ALL.into_iter().filter(|d| flag(d.name())).collect();
                spec = spec.with_faces(&faces);
            }
            Block::ManualValve => {
                spec.dir = axis().positive();
                spec.opening = get_i32(nbt, "opening").unwrap_or(10).clamp(0, 10) as f64 / 10.0;
            }
            Block::RedstoneValve => {
                spec.dir = axis().positive();
                spec.opening = if flag("powered") { 1.0 } else { 0.0 };
            }
            Block::SteamRelease => {
                spec.dir = dir("facing", Dir::Down);
                spec.opening = if flag("open") { 1.0 } else { 0.0 };
            }
            Block::EmergencyValve => {
                spec.dir = dir("facing", Dir::Down);
                if let Some(set) = get_f64(nbt, "set") {
                    spec.set = set;
                }
            }
            Block::Pump | Block::SteamCylinder => spec.dir = dir("facing", Dir::North),
            Block::Flywheel => {
                spec.dir = axis().positive();
                spec.reverse = get_bool(nbt, "reverse");
            }
            Block::Governor => {
                spec.dir = axis().positive();
                spec.target = crate::sim::steam::govern::clean_target(
                    get_f64(nbt, "target").unwrap_or(crate::sim::steam::govern::DEFAULT_TARGET),
                );
                spec.sens = get_i32(nbt, "sens")
                    .unwrap_or(crate::sim::steam::govern::DEFAULT_SENS as i32)
                    .clamp(1, 5) as u8;
            }
            Block::FurnaceDamper => {
                spec.dir = dir("facing", Dir::North);
                spec.set = get_f64(nbt, "set").unwrap_or(crate::sim::steam::damper::DEFAULT_SET);
                spec.powered = flag("powered");
            }
            Block::HeatExchanger => {
                spec.dir = dir("facing", Dir::Up);
                spec.wet = flag("waterlogged");
            }
            Block::Bellows | Block::CopperRadiator | Block::IronRadiator | Block::Drain => {
                spec.dir = dir("facing", Dir::North);
            }
            Block::Grinder => spec.dir = dir("facing", Dir::North),
            Block::CreativeSteam => {
                spec.fluid = Some(Content::Steam);
                spec.amount = get_f64(nbt, "c_rate").unwrap_or(super::creative::DEFAULT_STEAM_RATE);
                spec.set = get_f64(nbt, "c_bar").unwrap_or(super::creative::DEFAULT_BAR);
            }
            Block::CreativeFirebox => spec.free = get_bool(nbt, "c_free"),
            Block::CreativeWater => {
                spec.fluid = Some(if get_bool(nbt, "c_hot") {
                    Content::HotWater
                } else {
                    Content::Water
                });
                spec.amount = get_f64(nbt, "c_rate").unwrap_or(super::creative::DEFAULT_WATER_RATE);
            }
            _ => {}
        }
        spec.rust = self.rust_stage(get);
        Some(spec)
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
            Block::Crank
            | Block::Grinder
            | Block::Pump
            | Block::CreativeEngine
            | Block::LoadBank => Shape::Socket(dir("facing", Dir::North)),
            Block::Governor => Shape::Socket(dir("socket", Dir::Up)),
            // The nozzle faces the firebox; the socket is the opposite face.
            Block::Bellows => Shape::Socket(dir("facing", Dir::North).opposite()),
            Block::Flywheel => Shape::Inline(
                get("axis")
                    .as_deref()
                    .and_then(Axis::from_name)
                    .unwrap_or(Axis::X),
            ),
            _ => return None,
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
        if self == Block::Grinder {
            spec.cooled = get_bool(nbt, "cooled");
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

/// What a plant part saved: its share of a multiblock and its steam. `None`
/// when it never saved (`pv` missing).
pub fn plant_saved(nbt: &Compound) -> Option<PlantSaved> {
    get_i32(nbt, "pv")?;
    let formation = match (
        get_i32(nbt, "fx"),
        get_i32(nbt, "fy"),
        get_i32(nbt, "fz"),
        get_i32(nbt, "fsx"),
        get_i32(nbt, "fsy"),
        get_i32(nbt, "fsz"),
    ) {
        (Some(x), Some(y), Some(z), Some(sx), Some(sy), Some(sz)) if sx > 0 => Some(Cuboid {
            min: Pos::new(x, y, z),
            size: [sx, sy, sz],
        }),
        _ => None,
    };
    let d = PlantSaved::default();
    Some(PlantSaved {
        water: get_f64(nbt, "water").unwrap_or(d.water),
        temp: get_f64(nbt, "temp").unwrap_or(d.temp),
        steam: get_f64(nbt, "steam").unwrap_or(d.steam),
        timer: get_f64(nbt, "ptimer").unwrap_or(d.timer),
        content: get_str(nbt, "content")
            .as_deref()
            .and_then(Content::from_name),
        formation,
        // "ball_x", not "ball": the governor keeps its look step as "ball".
        ball: get_f64(nbt, "ball_x").unwrap_or(d.ball),
        ball_v: get_f64(nbt, "ball_v").unwrap_or(d.ball_v),
        buffer: get_f64(nbt, "buffer").unwrap_or(d.buffer),
        opening: get_f64(nbt, "opening").unwrap_or(d.opening),
    })
}

/// The fields a plant part saves.
pub fn plant_fields(s: &PlantSaved) -> Compound {
    let mut c = Compound::new();
    put_i32(&mut c, "pv", 1);
    put_f64(&mut c, "water", s.water);
    put_f64(&mut c, "temp", s.temp);
    put_f64(&mut c, "steam", s.steam);
    put_f64(&mut c, "ptimer", s.timer);
    put_str(&mut c, "content", s.content.map_or("", Content::name));
    put_f64(&mut c, "ball_x", s.ball);
    put_f64(&mut c, "ball_v", s.ball_v);
    put_f64(&mut c, "buffer", s.buffer);
    put_f64(&mut c, "opening", s.opening);
    let (min, size) = s
        .formation
        .map_or((Pos::new(0, 0, 0), [0, 0, 0]), |f| (f.min, f.size));
    put_i32(&mut c, "fx", min.x);
    put_i32(&mut c, "fy", min.y);
    put_i32(&mut c, "fz", min.z);
    put_i32(&mut c, "fsx", size[0]);
    put_i32(&mut c, "fsy", size[1]);
    put_i32(&mut c, "fsz", size[2]);
    c
}

/// The fields a regulation part saves (governor balls, damper opening, a
/// cooled machine's or exchanger's buffer): `plant_fields` without a shell's
/// water and steam, and without keys other blocks use for themselves.
pub fn regulation_fields(s: &PlantSaved) -> Compound {
    let mut c = Compound::new();
    put_i32(&mut c, "pv", 1);
    put_f64(&mut c, "ball_x", s.ball);
    put_f64(&mut c, "ball_v", s.ball_v);
    put_f64(&mut c, "buffer", s.buffer);
    put_f64(&mut c, "opening", s.opening);
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
    // 0.2
    "opening",
    "set",
    "bar",
    "level",
    "lwarn",
    "pwarn",
    "glow",
    "reverse",
    "fill",
    "content",
    // 0.3
    "ball",
    "lever",
    "brpm",
    "cooled",
    "target",
    "sens",
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
