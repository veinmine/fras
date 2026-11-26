//! `/fras scenario <name>`: ready-made setups for testing, built east of
//! where the command runs (logistics 12, machines 10).
//!
//! - `ore_line`: the 0.4 reference ore line on creative engines at 64 RPM:
//!   a chest of raw iron, a hopper, a belt into a grinder, a belt under it
//!   through a washer in belt mode into a trommel, and chests at the
//!   trommel's three outlets.
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
    // Origin two blocks east, one up, so the belts sit at y+1 and y.
    let o = Pos::new(at.x + 2, at.y + 1, at.z);
    clear(host, dim, o, [-1, -2, -2], [19, 3, 2]);
    run(
        host,
        dim,
        &format!(
            "fill {} {} {} {} {} {} minecraft:stone",
            o.x - 1,
            o.y - 3,
            o.z - 2,
            o.x + 19,
            o.y - 3,
            o.z + 2
        ),
    );
    let belt = "fras:conveyor_belt[facing=east]";
    let engine = "fras:creative_engine[facing=south]";
    // Feed: chest → hopper → belt 1 (y+0, x 0..7) → grinder (x 8).
    set(host, dim, o, 0, 2, 0, "minecraft:chest");
    set(host, dim, o, 0, 1, 0, "minecraft:hopper[facing=down]");
    for x in 0..8 {
        set(host, dim, o, x, 0, 0, belt);
    }
    set(host, dim, o, 3, 0, -1, engine);
    set(host, dim, o, 8, 0, 0, "fras:grinder[facing=north]");
    set(host, dim, o, 8, 0, -1, engine);
    // The grinder pushes down onto belt 2 (y-1, x 8..15); the washer rides
    // over it at x 11.
    for x in 8..16 {
        set(host, dim, o, x, -1, 0, belt);
    }
    set(host, dim, o, 13, -1, -1, engine);
    set(host, dim, o, 11, 0, 0, "fras:washer[facing=north]");
    set(host, dim, o, 11, 0, -1, engine);
    // Water for the washer: a pipe from a creative water source on its
    // south side.
    set(
        host,
        dim,
        o,
        11,
        0,
        1,
        "fras:copper_pipe[north=true,south=true]",
    );
    set(host, dim, o, 11, 0, 2, "fras:creative_water_source");
    // The trommel at x 16-17, its outlets into chests.
    set(
        host,
        dim,
        o,
        16,
        -1,
        0,
        "fras:trommel[facing=east,part=start,socket=north]",
    );
    set(
        host,
        dim,
        o,
        17,
        -1,
        0,
        "fras:trommel[facing=east,part=end,socket=north]",
    );
    set(host, dim, o, 16, -1, -1, engine);
    set(host, dim, o, 16, -2, 0, "minecraft:chest");
    set(host, dim, o, 17, -2, 0, "minecraft:chest");
    set(host, dim, o, 18, -1, 0, "minecraft:chest");
    for e in [
        [3, 0, -1],
        [8, 0, -1],
        [13, -1, -1],
        [11, 0, -1],
        [16, -1, -1],
    ] {
        setting(host, level, o, e, "c_rpm", 64.0);
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
        o.x + 18,
        o.x,
        o.y + 2,
        o.z,
        o.x + 16,
        o.y - 2,
        o.z,
        o.x + 17,
        o.y - 2,
        o.z,
        o.x + 18,
        o.y - 1,
        o.z
    )
}

/// A single-cylinder engine on 4 bar steam driving 32 Nm (expect about
/// 64 RPM: steam spec 9, 64 Nm stall and 128 RPM top at 4 bar).
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
         Expect about 64 RPM: /fras dump {} {} {} part",
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
