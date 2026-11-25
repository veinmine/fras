//! Rust (rust spec): which parts rust, exposure from the world, the rust
//! count and its stages, leaking, paint and scraping. Free of the game; the
//! game counts random ticks and reads the world.

/// A part's rust stage, as its block state shows it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    Clean,
    Spotty,
    Rusty,
    Leaking,
    /// Painted: never rusts.
    Painted,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Clean,
        Stage::Spotty,
        Stage::Rusty,
        Stage::Leaking,
        Stage::Painted,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Clean => "clean",
            Stage::Spotty => "spotty",
            Stage::Rusty => "rusty",
            Stage::Leaking => "leaking",
            Stage::Painted => "painted",
        }
    }

    pub fn from_name(name: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn leaking(self) -> bool {
        self == Stage::Leaking
    }

    /// Spotty, rusty or leaking: the wrench scrapes it first.
    pub fn rusted(self) -> bool {
        matches!(self, Stage::Spotty | Stage::Rusty | Stage::Leaking)
    }

    fn next(self) -> Stage {
        match self {
            Stage::Clean => Stage::Spotty,
            Stage::Spotty => Stage::Rusty,
            _ => Stage::Leaking,
        }
    }
}

/// The count per stage at 1×, and the speeds of the setting *(tune)*.
pub const COUNT_PER_STAGE: u32 = 18;
pub const SPEEDS: [(f64, u32); 4] = [(0.5, 36), (1.0, 18), (2.0, 9), (4.0, 5)];
/// Random ticks per in-game day at the default `randomTickSpeed`.
pub const TICKS_PER_DAY: f64 = 17.6;
/// What a leaking block loses, mB/s.
pub const LEAK_RATE: f64 = 10.0;

/// The count a stage needs at a rust speed (index into `SPEEDS`).
pub fn per_stage(speed: usize) -> u32 {
    SPEEDS.get(speed).map_or(COUNT_PER_STAGE, |s| s.1)
}

/// What exposes a part (spec 4), checked when it is visited.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Exposure {
    pub waterlogged: bool,
    pub touching_water: bool,
    /// Raining on the position above it, which sees the sky.
    pub rain: bool,
    /// In an active scald area (a vent's jet, a leak, an exhaust, a cloud).
    pub steam: bool,
}

/// What rusts it most, if anything.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    Steam,
    Waterlogged,
    Water,
    Rain,
}

impl Cause {
    pub fn name(self) -> &'static str {
        match self {
            Cause::Steam => "steam",
            Cause::Waterlogged => "waterlogged",
            Cause::Water => "water",
            Cause::Rain => "rain",
        }
    }
}

impl Exposure {
    /// The highest weight that applies: 2 for steam, 1 for water or rain.
    pub fn weight(&self) -> u32 {
        if self.steam {
            2
        } else if self.waterlogged || self.touching_water || self.rain {
            1
        } else {
            0
        }
    }

    pub fn cause(&self) -> Option<Cause> {
        if self.steam {
            Some(Cause::Steam)
        } else if self.waterlogged {
            Some(Cause::Waterlogged)
        } else if self.touching_water {
            Some(Cause::Water)
        } else if self.rain {
            Some(Cause::Rain)
        } else {
            None
        }
    }
}

/// One random tick of a rusting part: its new stage and count. Painted and
/// leaking parts don't change; with rust off nothing gains.
pub fn tick(stage: Stage, count: u32, exposure: Exposure, on: bool, speed: usize) -> (Stage, u32) {
    if !on || matches!(stage, Stage::Painted | Stage::Leaking) {
        return (stage, count);
    }
    let count = count + exposure.weight();
    let need = per_stage(speed);
    if count >= need {
        (stage.next(), 0)
    } else {
        (stage, count)
    }
}

/// Expected in-game days to the next stage while exposed like this.
pub fn days_to_next(
    count: u32,
    exposure: Exposure,
    speed: usize,
    random_tick_speed: u32,
) -> Option<f64> {
    let w = exposure.weight();
    if w == 0 || random_tick_speed == 0 {
        return None;
    }
    let left = per_stage(speed).saturating_sub(count) as f64;
    let ticks_per_day = TICKS_PER_DAY * random_tick_speed as f64 / 3.0;
    Some((left / w as f64) / ticks_per_day)
}

/// What a leaking part does, by kind (spec 6).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Leak {
    /// Vents 10 mB/s of steam (steam network parts, iron boilers, steam tanks, cylinders).
    Steam,
    /// Drips 10 mB/s of what passes (water, hot water, exhaust networks; water tanks).
    Drip,
    /// Jams, "rusted through" (rotational parts, the pump, the governor's spindle).
    Jam,
    /// Gives no cooling (iron radiators).
    NoCooling,
    /// Sticks where it is (dampers).
    Stick,
}

/// What scraping a part gives (spec 8).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scrape {
    /// Scraped clean: flakes dropped, an iron plate used if it leaked.
    Done { flakes: u32, plate: bool },
    /// Leaking and no iron plate at hand.
    NeedsPlate,
    /// Clean or painted: the wrench does what it always does.
    Nothing,
}

/// Scrape a part at `stage`, the player carrying an iron plate or not
/// (creative counts as carrying one).
pub fn scrape(stage: Stage, has_plate: bool) -> Scrape {
    match stage {
        Stage::Spotty => Scrape::Done {
            flakes: 1,
            plate: false,
        },
        Stage::Rusty => Scrape::Done {
            flakes: 2,
            plate: false,
        },
        Stage::Leaking if has_plate => Scrape::Done {
            flakes: 3,
            plate: true,
        },
        Stage::Leaking => Scrape::NeedsPlate,
        _ => Scrape::Nothing,
    }
}

/// Whether paint takes: only on a clean part.
pub fn paint(stage: Stage) -> Result<Stage, &'static str> {
    match stage {
        Stage::Clean => Ok(Stage::Painted),
        Stage::Painted => Err("fras.rust.already_painted"),
        _ => Err("fras.rust.scrape_first"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exp(waterlogged: bool, touching_water: bool, rain: bool, steam: bool) -> Exposure {
        Exposure {
            waterlogged,
            touching_water,
            rain,
            steam,
        }
    }

    #[test]
    fn each_trigger_alone_and_the_highest_weight_wins() {
        assert_eq!(exp(true, false, false, false).weight(), 1);
        assert_eq!(exp(false, true, false, false).weight(), 1);
        assert_eq!(exp(false, false, true, false).weight(), 1);
        assert_eq!(exp(false, false, false, true).weight(), 2);
        assert_eq!(exp(true, true, true, false).weight(), 1);
        assert_eq!(exp(true, true, true, true).weight(), 2);
        assert_eq!(Exposure::default().weight(), 0);
        assert_eq!(
            exp(true, true, false, false).cause(),
            Some(Cause::Waterlogged)
        );
    }

    #[test]
    fn contents_never_count() {
        // Nothing about what flows inside is part of the exposure: a dry
        // part gains nothing however long it runs.
        let (s, c) = (0..1000).fold((Stage::Clean, 0), |(s, c), _| {
            tick(s, c, Exposure::default(), true, 1)
        });
        assert_eq!((s, c), (Stage::Clean, 0));
    }

    #[test]
    fn stages_advance_at_18_and_at_each_speed() {
        let water = exp(false, true, false, false);
        let mut st = (Stage::Clean, 0);
        let mut ticks = 0;
        while st.0 != Stage::Leaking {
            st = tick(st.0, st.1, water, true, 1);
            ticks += 1;
        }
        assert_eq!(ticks, 54, "three stages of 18");
        // Steam counts double.
        let mut st = (Stage::Clean, 0);
        let mut ticks = 0;
        while st.0 != Stage::Leaking {
            st = tick(st.0, st.1, exp(false, false, false, true), true, 1);
            ticks += 1;
        }
        assert_eq!(ticks, 27);
        for (i, need) in [36, 18, 9, 5].into_iter().enumerate() {
            assert_eq!(
                tick(Stage::Clean, need - 1, water, true, i),
                (Stage::Spotty, 0)
            );
            assert_eq!(tick(Stage::Clean, need - 2, water, true, i).0, Stage::Clean);
        }
        // About three days in water at 1×.
        let d = days_to_next(0, water, 1, 3).unwrap() * 3.0;
        assert!((d - 3.07).abs() < 0.05, "{d}");
        assert_eq!(days_to_next(0, Exposure::default(), 1, 3), None);
    }

    #[test]
    fn rust_off_painted_and_leaking_parts_stay() {
        let water = exp(true, false, false, false);
        assert_eq!(tick(Stage::Rusty, 17, water, false, 1), (Stage::Rusty, 17));
        assert_eq!(tick(Stage::Painted, 0, water, true, 1), (Stage::Painted, 0));
        assert_eq!(tick(Stage::Leaking, 3, water, true, 1), (Stage::Leaking, 3));
    }

    #[test]
    fn paint_only_on_clean_and_scraping_drops_and_the_plate() {
        assert_eq!(paint(Stage::Clean), Ok(Stage::Painted));
        assert!(paint(Stage::Spotty).is_err());
        assert!(paint(Stage::Leaking).is_err());
        assert!(paint(Stage::Painted).is_err());
        assert_eq!(
            scrape(Stage::Spotty, false),
            Scrape::Done {
                flakes: 1,
                plate: false
            }
        );
        assert_eq!(
            scrape(Stage::Rusty, false),
            Scrape::Done {
                flakes: 2,
                plate: false
            }
        );
        assert_eq!(scrape(Stage::Leaking, false), Scrape::NeedsPlate);
        assert_eq!(
            scrape(Stage::Leaking, true),
            Scrape::Done {
                flakes: 3,
                plate: true
            }
        );
        assert_eq!(scrape(Stage::Clean, true), Scrape::Nothing);
        assert_eq!(scrape(Stage::Painted, true), Scrape::Nothing);
        for s in Stage::ALL {
            assert_eq!(Stage::from_name(s.name()), Some(s));
        }
    }
}
