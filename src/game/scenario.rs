//! `/fras scenario <name>`: ready-made setups for testing, built east of
//! where the command runs (logistics 12, machines 10).
//!
//! - `ore_line`: the 0.4 reference ore line on creative engines at 64 RPM,
//!   its belts at eye level: a chest of raw ore, a hopper, a belt through
//!   an inline grinder and a washer into a three-block trommel, and chests
//!   under the trommel's three outlets.
//! - `test_engine`: a single-cylinder steam engine on a creative steam
//!   source at 4 bar, driving a load bank of 32 Nm through an iron shaft.
//!
//! Blocks are set with commands (exact states, no placement rules), then
//! the creative blocks get their settings.

use mod_api::{LevelRef, ServerHost};

use super::blocks;
use super::server::{self, Dim};
use crate::sim::geom::Pos;

pub const NAMES: [&str; 2] = ["ore_line", "test_engine"];

const BELT: &str = "fras:conveyor_belt[facing=east]";
const ENGINE: &str = "fras:creative_engine[facing=south]";

/// The ore line's blocks, relative to its origin: belts and machine tops
/// at y 0, drives at y -1 (machines spec 2.3, 10).
const ORE_LINE: &[([i32; 3], &str)] = &[
    // Feed: chest → hopper → belt 1 (x 0..4) → inline grinder (x 5).
    ([0, 2, 0], "minecraft:chest"),
    ([0, 1, 0], "minecraft:hopper[facing=down]"),
    ([0, 0, 0], BELT),
    ([1, 0, 0], BELT),
    ([2, 0, 0], BELT),
    ([3, 0, 0], BELT),
    ([4, 0, 0], BELT),
    ([2, 0, -1], ENGINE),
    (
        [5, -1, 0],
        "fras:inline_grinder[facing=east,half=lower,socket=north]",
    ),
    (
        [5, 0, 0],
        "fras:inline_grinder[facing=east,half=upper,socket=down]",
    ),
    ([5, -1, -1], ENGINE),
    // Belt 2 (x 6..7) → washer (x 8), water piped into its top's south side.
    ([6, 0, 0], BELT),
    ([7, 0, 0], BELT),
    ([6, 0, -1], ENGINE),
    (
        [8, -1, 0],
        "fras:washer[facing=east,half=lower,socket=north]",
    ),
    ([8, 0, 0], "fras:washer[facing=east,half=upper,socket=down]"),
    ([8, -1, -1], ENGINE),
    ([8, 0, 1], "fras:copper_pipe[north=true,south=true]"),
    ([8, 0, 2], "fras:creative_water_source"),
    // Belt 3 (x 9..10) → the trommel's west end (x 11..13), its outlets
    // into chests; it forms and takes its socket by itself.
    ([9, 0, 0], BELT),
    ([10, 0, 0], BELT),
    ([9, 0, -1], ENGINE),
    ([11, 0, 0], "fras:trommel[axis=x,part=none,socket=none]"),
    ([12, 0, 0], "fras:trommel[axis=x,part=none,socket=none]"),
    ([13, 0, 0], "fras:trommel[axis=x,part=none,socket=none]"),
    ([12, 0, -1], ENGINE),
    ([11, -1, 0], "minecraft:chest"),
    ([12, -1, 0], "minecraft:chest"),
    ([13, -1, 0], "minecraft:chest"),
];

/// The ore line's engines: where, RPM, reversed. Positive rotation about
/// +z runs an east-heading belt west (`belt::drive_sign`), so the belt
/// drives turn the other way.
const ORE_LINE_ENGINES: [([i32; 3], f64, bool); 6] = [
    ([2, 0, -1], 64.0, true),
    ([6, 0, -1], 64.0, true),
    ([9, 0, -1], 64.0, true),
    ([5, -1, -1], 64.0, false),
    ([8, -1, -1], 64.0, false),
    ([12, 0, -1], 64.0, false),
];

fn run(host: &mut dyn ServerHost, dim: Dim, line: &str) -> bool {
    host.commands()
        .run_command(&format!("execute in {dim} run {line}"))
}

fn set(host: &mut dyn ServerHost, dim: Dim, o: Pos, x: i32, y: i32, z: i32, block: &str) {
    let p = Pos::new(o.x + x, o.y + y, o.z + z);
    run(
        host,
        dim,
        &format!("setblock {} {} {} {block}", p.x, p.y, p.z),
    );
}

fn setting(host: &mut dyn ServerHost, level: LevelRef, o: Pos, at: [i32; 3], key: &str, v: f64) {
    let p = Pos::new(o.x + at[0], o.y + at[1], o.z + at[2]);
    let mut c = mod_api::Compound::new();
    blocks::put_f64(&mut c, key, v);
    server::merge(host, level, p, c);
}

/// Build a scenario with its origin at `at`; a line saying what was built.
pub fn build(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    name: &str,
    at: Pos,
) -> Result<String, String> {
    match name {
        "ore_line" => Ok(ore_line(host, level, dim, at)),
        "test_engine" => Ok(test_engine(host, level, dim, at)),
        _ => Err(format!("scenarios: {}", NAMES.join(", "))),
    }
}

fn clear(host: &mut dyn ServerHost, dim: Dim, o: Pos, from: [i32; 3], to: [i32; 3]) {
    run(
        host,
        dim,
        &format!(
            "fill {} {} {} {} {} {} minecraft:air",
            o.x + from[0],
            o.y + from[1],
            o.z + from[2],
            o.x + to[0],
            o.y + to[1],
            o.z + to[2]
        ),
    );
}

/// The reference ore line (machines spec 10), shortened: every machine and
/// belt on its own creative engine at 64 RPM, so the line runs at the
/// spec's speed whatever the engines' load.
fn ore_line(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, at: Pos) -> String {
    // Origin two blocks east, one up: the belts at eye level, the drives
    // and outlet chests on the floor.
    let o = Pos::new(at.x + 2, at.y + 1, at.z);
    clear(host, dim, o, [-1, -1, -2], [15, 3, 2]);
    run(
        host,
        dim,
        &format!(
            "fill {} {} {} {} {} {} minecraft:stone",
            o.x - 1,
            o.y - 2,
            o.z - 2,
            o.x + 15,
            o.y - 2,
            o.z + 2
        ),
    );
    for (at, block) in ORE_LINE {
        set(host, dim, o, at[0], at[1], at[2], block);
    }
    for (at, rpm, rev) in ORE_LINE_ENGINES {
        setting(host, level, o, at, "c_rpm", rpm);
        let p = Pos::new(o.x + at[0], o.y + at[1], o.z + at[2]);
        let mut c = mod_api::Compound::new();
        blocks::put_bool(&mut c, "c_rev", rev);
        server::merge(host, level, p, c);
    }
    run(
        host,
        dim,
        &format!(
            "item replace block {} {} {} container.0 with minecraft:raw_iron 64",
            o.x,
            o.y + 2,
            o.z
        ),
    );
    run(
        host,
        dim,
        &format!(
            "item replace block {} {} {} container.1 with minecraft:raw_copper 64",
            o.x,
            o.y + 2,
            o.z
        ),
    );
    format!(
        "fras: ore_line built from {} {} {} east to x {}: chest of raw ore at {} {} {}; \
         fines chest {} {} {}, mediums {} {} {}, oversize {} {} {}. \
         Try /fras run 1200 then /fras report.",
        o.x,
        o.y - 2,
        o.z,
        o.x + 13,
        o.x,
        o.y + 2,
        o.z,
        o.x + 11,
        o.y - 1,
        o.z,
        o.x + 12,
        o.y - 1,
        o.z,
        o.x + 13,
        o.y - 1,
        o.z
    )
}

/// A single-cylinder engine on 4 bar steam driving 32 Nm (steam spec 9:
/// 64 Nm stall and 128 RPM top at 4 bar, so 64 RPM on the load alone; the
/// shaft and tachometer's friction, about 2 Nm, bring it to about 60).
fn test_engine(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, at: Pos) -> String {
    // The flywheel's hub at o, its plane north-south (axis x).
    let o = Pos::new(at.x + 3, at.y + 2, at.z);
    clear(host, dim, o, [-2, -2, -2], [6, 2, 5]);
    set(host, dim, o, 0, 0, 0, "fras:flywheel[axis=x]");
    // The cylinder two blocks south of the hub, facing it; steam from behind.
    set(host, dim, o, 0, 0, 2, "fras:steam_cylinder[facing=north]");
    set(
        host,
        dim,
        o,
        0,
        0,
        3,
        "fras:iron_pipe[north=true,south=true]",
    );
    set(host, dim, o, 0, 0, 4, "fras:creative_steam_source");
    // The shaft east of the hub to a load bank, with a tachometer on it.
    for x in 1..4 {
        set(host, dim, o, x, 0, 0, "fras:iron_shaft[axis=x]");
    }
    set(host, dim, o, 2, 1, 0, "fras:tachometer[facing=up]");
    set(host, dim, o, 4, 0, 0, "fras:load_bank[facing=west]");
    setting(host, level, o, [4, 0, 0], "c_load", 32.0);
    setting(host, level, o, [0, 0, 4], "c_bar", 4.0);
    format!(
        "fras: test_engine built: flywheel at {} {} {}, load bank (32 Nm) at {} {} {}. \
         Expect about 60 RPM: /fras dump {} {} {} part",
        o.x,
        o.y,
        o.z,
        o.x + 4,
        o.y,
        o.z,
        o.x + 1,
        o.y,
        o.z
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::belt::{BeltBlock, chain, drive_sign};
    use crate::sim::geom::Dir;

    fn belt_at(p: Pos) -> Option<BeltBlock> {
        ORE_LINE
            .iter()
            .find(|(a, b)| *b == BELT && Pos::new(a[0], a[1], a[2]) == p)
            .map(|_| BeltBlock { heading: Dir::East })
    }

    #[test]
    fn ore_line_belts_are_three_lines_between_the_machines() {
        for (start, end) in [(0, 4), (6, 7), (9, 10)] {
            let line = chain(Pos::new(start, 0, 0), &belt_at);
            assert_eq!(line.first(), Some(&Pos::new(start, 0, 0)));
            assert_eq!(line.last(), Some(&Pos::new(end, 0, 0)));
        }
    }

    #[test]
    fn ore_line_machines_stand_on_their_drives() {
        for (a, b) in ORE_LINE {
            if b.contains("half=upper") {
                let below = ORE_LINE
                    .iter()
                    .find(|(q, _)| *q == [a[0], a[1] - 1, a[2]])
                    .map(|(_, k)| *k);
                let lower = b.replace("half=upper,socket=down", "half=lower,socket=north");
                assert_eq!(below, Some(lower.as_str()), "{b} at {a:?}");
            }
        }
    }

    #[test]
    fn ore_line_belt_drives_run_the_belts_east() {
        for (at, rpm, rev) in ORE_LINE_ENGINES {
            let drives_belt = ORE_LINE
                .iter()
                .any(|(b, k)| *k == BELT && b[0] == at[0] && b[1] == at[1] && b[2] == at[2] + 1);
            if drives_belt {
                let w = if rev { -rpm } else { rpm };
                assert!(
                    drive_sign(Dir::East) * w > 0.0,
                    "engine at {at:?} runs its belt west"
                );
            }
        }
    }
}
