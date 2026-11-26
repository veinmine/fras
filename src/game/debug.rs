//! Debug tools (agent oriented): `/fras dump`, belt throughput counters,
//! the event log, `/fras scenario`, `/fras run N` with an outcome report,
//! `/fras rust`, `/fras steam pause|resume|step` and `/fras perf`.
//!
//! Every answer is plain text an agent can read from the command's
//! feedback; the long ones (dumps, reports) are JSON, also written under
//! `fras-debug/` in the game directory.

use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

use mod_api::serde_json::{self, Value, json};
use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, CommandSource, Compound, Host, LevelRef,
    ServerHost, Text,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, with, with_level};
use crate::sim::geom::Pos;

/// Events kept in the log.
const LOG_CAP: usize = 4000;

/// One logged event.
#[derive(Clone, Debug)]
pub struct Event {
    pub tick: i64,
    pub dim: Dim,
    pub kind: &'static str,
    pub pos: Pos,
    pub detail: String,
}

/// A fast-forward waiting for its end.
#[derive(Clone, Debug)]
pub struct Run {
    pub until: i64,
    pub ticks: u32,
    pub start: i64,
    pub baseline: Value,
    pub wall: Instant,
}

/// The debug tools' state (in the server's shared state).
#[derive(Default)]
pub struct Debug {
    pub events: VecDeque<Event>,
    /// Totals by kind since the server started (never trimmed).
    pub totals: BTreeMap<&'static str, u64>,
    pub steam_paused: bool,
    pub steam_steps: u32,
    /// Per section: calls, total µs, max µs.
    pub perf: BTreeMap<&'static str, (u64, f64, f64)>,
    pub perf_since: Option<Instant>,
    pub run: Option<Run>,
    pub last_report: Option<String>,
    /// Parts warning at the last check, by level.
    pub warning: std::collections::HashMap<Dim, std::collections::BTreeSet<Pos>>,
}

// ---- Recording ---------------------------------------------------------------------

/// Log an event (jam, overspeed, drop, backup, ...).
pub fn event(dim: Dim, tick: i64, kind: &'static str, pos: Pos, detail: String) {
    with(|s| {
        let d = &mut s.debug;
        *d.totals.entry(kind).or_default() += 1;
        if d.events.len() >= LOG_CAP {
            d.events.pop_front();
        }
        d.events.push_back(Event {
            tick,
            dim,
            kind,
            pos,
            detail,
        });
    });
}

/// Count something without logging it (fuel items burnt).
pub fn count(kind: &'static str, n: u64) {
    with(|s| *s.debug.totals.entry(kind).or_default() += n);
}

/// Time a section of the level tick.
pub fn time(section: &'static str, since: Instant) {
    let us = since.elapsed().as_secs_f64() * 1e6;
    with(|s| {
        let d = &mut s.debug;
        d.perf_since.get_or_insert_with(Instant::now);
        flush_pending(d);
        let e = d.perf.entry(section).or_insert((0, 0.0, 0.0));
        e.0 += 1;
        e.1 += us;
        e.2 = e.2.max(us);
    });
}

/// Time a section from inside `with_level` (the server's state is held:
/// queue it in a thread-local and add it on the next `time`).
pub fn time_local(section: &'static str, since: Instant) {
    let us = since.elapsed().as_secs_f64() * 1e6;
    PENDING.with(|p| p.borrow_mut().push((section, us)));
}

thread_local! {
    static PENDING: std::cell::RefCell<Vec<(&'static str, f64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn flush_pending(d: &mut Debug) {
    PENDING.with(|p| {
        for (section, us) in p.borrow_mut().drain(..) {
            let e = d.perf.entry(section).or_insert((0, 0.0, 0.0));
            e.0 += 1;
            e.1 += us;
            e.2 = e.2.max(us);
        }
    });
}

/// Every 10 ticks: parts that started warning (overspeed or overload).
pub fn warnings(dim: Dim, tick: i64) {
    let now: Vec<(Pos, bool)> = with_level(dim, |l| {
        l.mech
            .warned()
            .into_iter()
            .map(|(p, over, _)| (p, over))
            .collect()
    });
    with(|s| {
        let d = &mut s.debug;
        let before = d.warning.remove(dim).unwrap_or_default();
        let mut set = std::collections::BTreeSet::new();
        for (pos, over) in now {
            set.insert(pos);
            if !before.contains(&pos) {
                let kind = if over { "overspeed" } else { "overload" };
                *d.totals.entry(kind).or_default() += 1;
                if d.events.len() >= LOG_CAP {
                    d.events.pop_front();
                }
                d.events.push_back(Event {
                    tick,
                    dim,
                    kind,
                    pos,
                    detail: String::new(),
                });
            }
        }
        d.warning.insert(dim, set);
    });
}

/// Whether the steam step runs this tick (`/fras steam pause|step`).
pub fn steam_runs() -> bool {
    with(|s| {
        let d = &mut s.debug;
        if !d.steam_paused {
            return true;
        }
        if d.steam_steps > 0 {
            d.steam_steps -= 1;
            return true;
        }
        false
    })
}

// ---- Commands ----------------------------------------------------------------------

pub fn commands() -> Vec<Command> {
    [
        "fras dump <pos:block_pos> [what:word]",
        "fras counter add <pos:block_pos>",
        "fras counter remove <pos:block_pos>",
        "fras counter list",
        "fras counter clear",
        "fras events [count:integer] [kind:word]",
        "fras events clear",
        "fras scenario <name:word>",
        "fras run <ticks:integer>",
        "fras report",
        "fras rust set <pos:block_pos> <stage:word>",
        "fras rust tick <pos:block_pos> <times:integer>",
        "fras steam pause",
        "fras steam resume",
        "fras steam step [steps:integer]",
        "fras perf",
        "fras perf reset",
    ]
    .into_iter()
    .map(|u| Command::op(u, 2))
    .collect()
}

fn arg_pos(cmd: &CommandInvocation, name: &str) -> Option<Pos> {
    match cmd.argument(name)? {
        CommandArgumentValue::BlockPos(p) => Some(Pos::from_array(*p)),
        _ => None,
    }
}

fn arg_int(cmd: &CommandInvocation, name: &str) -> Option<i32> {
    match cmd.argument(name)? {
        CommandArgumentValue::Integer(n) => Some(*n),
        _ => None,
    }
}

fn arg_str(cmd: &CommandInvocation, name: &str) -> Option<String> {
    match cmd.argument(name)? {
        CommandArgumentValue::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn say(host: &mut dyn ServerHost, line: impl Into<String>) {
    let _ = host.commands().feedback(Text::literal(line.into()));
}

/// Send JSON back: whole if it fits in a few feedback lines, and always to
/// a file under `fras-debug/`.
fn say_json(host: &mut dyn ServerHost, name: &str, value: &Value) {
    let text = serde_json::to_string(value).unwrap_or_default();
    let path = write_file(
        name,
        &serde_json::to_string_pretty(value).unwrap_or_default(),
    );
    // Feedback lines hold up to 4096 bytes; cut at char boundaries.
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let mut cut = rest.len().min(3800);
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        say(host, &rest[..cut]);
        rest = &rest[cut..];
    }
    if let Some(path) = path {
        say(host, format!("(written to {path})"));
    }
}

fn write_file(name: &str, text: &str) -> Option<String> {
    let dir = std::path::Path::new("fras-debug");
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, text).ok()?;
    Some(path.display().to_string())
}

fn level_of(host: &dyn Host, cmd: &CommandInvocation) -> Option<(LevelRef, Dim)> {
    let id = cmd.dimension.id(host)?;
    let level = host.level(&id)?;
    let dim = server::dim_of(host, level)?;
    Some((level, dim))
}

/// Run a `/fras ...` debug command; false if it isn't one.
pub fn run(host: &mut dyn ServerHost, source: &CommandSource, cmd: &CommandInvocation) -> bool {
    let words: Vec<&str> = cmd.literals.iter().map(String::as_str).collect();
    if words.first() != Some(&"fras") {
        return false;
    }
    let _ = source;
    let Some((level, dim)) = level_of(&*host, cmd) else {
        say(host, "fras: no level here");
        return true;
    };
    match words.get(1).copied() {
        Some("dump") => {
            let Some(pos) = arg_pos(cmd, "pos") else {
                return true;
            };
            let what = arg_str(cmd, "what").unwrap_or_else(|| "all".into());
            let v = dump(&*host, level, dim, pos, &what);
            let name = format!("dump-{}-{}-{}-{}", what, pos.x, pos.y, pos.z);
            say_json(host, &name, &v);
        }
        Some("counter") => counter(host, level, dim, &words, cmd),
        Some("events") => {
            if words.get(2) == Some(&"clear") {
                with(|s| s.debug.events.clear());
                say(host, "fras: event log cleared");
            } else {
                let n = arg_int(cmd, "count").unwrap_or(50).max(1) as usize;
                let kind = arg_str(cmd, "kind");
                let v = events_json(n, kind.as_deref());
                say_json(host, "events", &v);
            }
        }
        Some("scenario") => {
            let name = arg_str(cmd, "name").unwrap_or_default();
            let at = Pos::new(
                cmd.position[0].floor() as i32,
                cmd.position[1].floor() as i32,
                cmd.position[2].floor() as i32,
            );
            match super::scenario::build(host, level, dim, &name, at) {
                Ok(text) => say(host, text),
                Err(e) => say(host, format!("fras: {e}")),
            }
        }
        Some("run") => {
            let n = arg_int(cmd, "ticks").unwrap_or(1200).clamp(1, 1_000_000) as u32;
            start_run(host, level, n);
        }
        Some("report") => {
            let report = with(|s| s.debug.last_report.clone());
            match report {
                Some(r) => {
                    let v: Value = serde_json::from_str(&r).unwrap_or(Value::Null);
                    say_json(host, "report", &v);
                }
                None => {
                    let running = with(|s| s.debug.run.as_ref().map(|r| r.until));
                    match running {
                        Some(until) => {
                            let now = overworld_time(&*host);
                            say(
                                host,
                                format!("fras: run in progress, {} ticks left", until - now),
                            );
                        }
                        None => say(host, "fras: no report yet (use /fras run <ticks>)"),
                    }
                }
            }
        }
        Some("rust") => rust(host, level, dim, &words, cmd),
        Some("steam") => {
            match words.get(2).copied() {
                Some("pause") => with(|s| s.debug.steam_paused = true),
                Some("resume") => with(|s| {
                    s.debug.steam_paused = false;
                    s.debug.steam_steps = 0;
                }),
                Some("step") => {
                    let n = arg_int(cmd, "steps").unwrap_or(1).max(1) as u32;
                    // One steam step runs every STEP_TICKS ticks; a step
                    // here lets that many level ticks' worth through.
                    with(|s| {
                        s.debug.steam_paused = true;
                        s.debug.steam_steps += n * crate::sim::steam::STEP_TICKS as u32;
                    });
                }
                _ => {}
            }
            let (p, st) = with(|s| (s.debug.steam_paused, s.debug.steam_steps));
            say(
                host,
                format!(
                    "fras: steam {} (ticks to run: {st})",
                    if p { "paused" } else { "running" }
                ),
            );
        }
        Some("perf") => {
            if words.get(2) == Some(&"reset") {
                with(|s| {
                    s.debug.perf.clear();
                    s.debug.perf_since = None;
                });
                say(host, "fras: perf reset");
            } else {
                let v = perf_json();
                say_json(host, "perf", &v);
            }
        }
        _ => say(host, "fras: unknown debug command"),
    }
    true
}

// ---- Dump ----------------------------------------------------------------------------

fn pos_json(p: Pos) -> Value {
    json!([p.x, p.y, p.z])
}

fn nbt_json(c: &Compound) -> Value {
    Value::Object(
        c.iter()
            .map(|(k, v)| (String::from_utf16_lossy(&k.0), tag_json(v)))
            .collect(),
    )
}

/// An NBT tag as readable JSON: names and strings decoded, floats as
/// numbers (not their bits).
fn tag_json(t: &mod_api::Tag) -> Value {
    use mod_api::Tag;
    match t {
        Tag::End => Value::Null,
        Tag::Byte(v) => json!(v),
        Tag::Short(v) => json!(v),
        Tag::Int(v) => json!(v),
        Tag::Long(v) => json!(v),
        Tag::Float(bits) => json!(f32::from_bits(*bits)),
        Tag::Double(bits) => json!(f64::from_bits(*bits)),
        Tag::ByteArray(v) => json!(v),
        Tag::String(s) => Value::String(String::from_utf16_lossy(&s.0)),
        Tag::List(v) => Value::Array(v.iter().map(tag_json).collect()),
        Tag::Compound(c) => nbt_json(c),
        Tag::IntArray(v) => json!(v),
        Tag::LongArray(v) => json!(v),
    }
}

/// The JSON state at `pos`: its block, rotational part and network, belt
/// line, machine and plant part. `what`: all, part, network, line,
/// machine, plant.
pub fn dump(host: &dyn Host, level: LevelRef, dim: Dim, pos: Pos, what: &str) -> Value {
    let mut out = serde_json::Map::new();
    let block = block_at(host, level, pos);
    out.insert("pos".into(), pos_json(pos));
    out.insert(
        "block".into(),
        Value::String(block.map_or_else(
            || {
                level
                    .block(host, pos.to_array())
                    .map(|b| b.id(host).to_string())
                    .unwrap_or_default()
            },
            |b| b.id(),
        )),
    );
    out.insert(
        "game_time".into(),
        json!(level.game_time(host).unwrap_or(0)),
    );
    let all = what == "all";
    if all || what == "part" || what == "network" {
        out.insert("part".into(), part_json(dim, pos));
    }
    if all || what == "network" {
        out.insert("network".into(), network_json(dim, pos, what == "network"));
    }
    if all || what == "line" {
        out.insert("line".into(), line_json(host, dim, pos));
    }
    if all || what == "machine" {
        out.insert("machine".into(), machine_json(host, level, dim, pos, block));
    }
    if all || what == "plant" {
        let has = with_level(dim, |l| l.steam.works.contains(pos));
        out.insert("plant".into(), json!({ "in_plant": has }));
    }
    Value::Object(out)
}

fn part_json(dim: Dim, pos: Pos) -> Value {
    with_level(dim, |l| {
        let Some(p) = l.mech.part(pos) else {
            return Value::Null;
        };
        let reading = l.mech.reading(pos);
        let speed = l.mech.speed(pos);
        json!({
            "kind": format!("{:?}", p.spec.kind),
            "shape": format!("{:?}", p.spec.shape),
            "rpm": speed.map(|s| s.0),
            "paused": speed.map(|s| s.1),
            "jam": p.jam.map(|j| j.name()),
            "load": p.load,
            "drive": format!("{:?}", p.drive),
            "torque": p.torque,
            "torque_limit": p.torque_limit(),
            "overspeed_warning": p.over_t,
            "shear_warning": p.shear_t,
            "sag": p.sag,
            "ties": l.mech.ties(pos).into_iter().map(pos_json).collect::<Vec<_>>(),
            "reading": reading.map(|r| json!({
                "rpm": r.rpm, "paused": r.paused, "red_line": r.red_line,
                "min_working": r.min_working, "drawn": r.drawn, "give": r.give,
                "stall": r.stall, "parts": r.parts,
            })),
        })
    })
}

fn network_json(dim: Dim, pos: Pos, full: bool) -> Value {
    with_level(dim, |l| {
        let Some((net, paused)) = l.mech.own_pause(pos) else {
            return Value::Null;
        };
        let members: Vec<Pos> = l
            .mech
            .positions()
            .into_iter()
            .filter(|q| l.mech.own_pause(*q).is_some_and(|(n, _)| n == net))
            .collect();
        let parts: Vec<Value> = members
            .iter()
            .map(|q| {
                let p = l.mech.part(*q);
                if full {
                    json!({
                        "pos": pos_json(*q),
                        "kind": p.map(|p| format!("{:?}", p.spec.kind)),
                        "rpm": l.mech.speed(*q).map(|s| s.0),
                        "load": p.map(|p| p.load),
                        "jam": p.and_then(|p| p.jam).map(|j| j.name()),
                    })
                } else {
                    pos_json(*q)
                }
            })
            .collect();
        json!({ "index": net, "paused": paused, "size": members.len(), "parts": parts })
    })
}

fn line_json(host: &dyn Host, dim: Dim, pos: Pos) -> Value {
    let v = with_level(dim, |l| {
        let (ls, i) = l.belts.line_at(pos)?;
        let items: Vec<Value> = ls
            .line
            .entries
            .iter()
            .map(|e| {
                json!({
                    "item": e.item.item().id(host).to_string(),
                    "count": e.item.count(),
                    "x": e.x,
                    "held": e.hold.is_some(),
                })
            })
            .collect();
        let counters: Vec<Value> = ls
            .line
            .blocks
            .iter()
            .filter_map(|b| l.belts.counters.get(b).map(|c| (b, c)))
            .map(|(b, c)| json!({"pos": pos_json(*b), "total": c.total}))
            .collect();
        Some(json!({
            "kind": format!("{:?}", ls.kind),
            "index_here": i,
            "blocks": ls.line.blocks.iter().map(|b| pos_json(*b)).collect::<Vec<_>>(),
            "slopes": ls.line.slopes.iter().map(|s| s.name()).collect::<Vec<_>>(),
            "heading": ls.line.heading.name(),
            "blocks_per_tick": ls.v,
            "blocks_per_second": ls.v * 20.0,
            "stop": ls.stop.map(super::belts::stop_name),
            "drives": ls.drives.iter().map(|b| pos_json(*b)).collect::<Vec<_>>(),
            "drives_needed": crate::sim::belt::drives_needed(ls.line.len()),
            "end": format!("{:?}", ls.end),
            "load": ls.load,
            "count": ls.line.count(),
            "capacity": ls.line.capacity(),
            "items": items,
            "counters": counters,
        }))
    });
    v.unwrap_or(Value::Null)
}

fn machine_json(
    host: &dyn Host,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Option<Block>,
) -> Value {
    // A machine answers from the block that keeps its state: a line
    // machine's top, a formed trommel's middle.
    let pos = block
        .and_then(|b| super::machines::state_pos(host, level, pos, b))
        .unwrap_or(pos);
    let Some(entity) = host.block_entity(level, pos.to_array()) else {
        return Value::Null;
    };
    let nbt = entity.data(host).unwrap_or_default();
    let slots: Vec<Value> = (0..8u32)
        .map_while(|i| entity.item_slot(host, i).ok())
        .map(|s| {
            s.map_or(
                Value::Null,
                |s| json!({"item": s.item().id(host).to_string(), "count": s.count()}),
            )
        })
        .collect();
    let (job, made, signal) = with_level(dim, |l| {
        let station = l.machines.stations.get(&pos).map(|st| {
            json!({
                "items": st.entries.iter().map(|e| json!({
                    "item": e.item.item().id(host).to_string(),
                    "x": e.x,
                    "done": e.done,
                })).collect::<Vec<_>>(),
                "progress": st.work.progress,
                "recipe": st.work.recipe,
                "pending": st.pending.len(),
            })
        });
        let drum = l.machines.drums.get(&pos).map(|d| {
            json!({
                "items": d.items.iter().map(|t| json!({
                    "item": t.item.item().id(host).to_string(),
                    "size": t.size.index(),
                    "progress": t.progress,
                    "from_low": t.from_low,
                })).collect::<Vec<_>>(),
                "outlets": d.outlets.iter().map(Vec::len).collect::<Vec<_>>(),
            })
        });
        (
            station.or(drum),
            l.machines.made.get(&pos).copied(),
            l.machines
                .signals
                .get(&pos)
                .or_else(|| l.grinder_signal.get(&pos))
                .copied(),
        )
    });
    let data = entity.data_slots(host).unwrap_or_default();
    json!({
        "machine": block.and_then(Block::machine).map(|m| m.name()),
        "state_pos": [pos.x, pos.y, pos.z],
        "slots": slots,
        "menu_data": data,
        "status": data.get(2).map(|c| crate::sim::grind::Status::from_code(*c).key()),
        "progress": blocks::get_f64(&nbt, "progress"),
        "recipe": blocks::get_str(&nbt, "recipe"),
        "tank_mb": blocks::get_f64(&nbt, "tank"),
        "line": job,
        "made": made,
        "comparator": signal,
        "nbt": nbt_json(&nbt),
    })
}

// ---- Counters ---------------------------------------------------------------------------

fn counter(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    words: &[&str],
    cmd: &CommandInvocation,
) {
    let _ = level;
    match words.get(2).copied() {
        Some("add") => {
            let Some(pos) = arg_pos(cmd, "pos") else {
                return;
            };
            let ok = with_level(dim, |l| {
                if !l.belts.by_block.contains_key(&pos) {
                    return false;
                }
                let since = l.belts.ticks;
                l.belts.counters.insert(
                    pos,
                    super::belts::Counter {
                        since,
                        ..Default::default()
                    },
                );
                true
            });
            say(
                host,
                if ok {
                    format!(
                        "fras: counting items crossing {} {} {}",
                        pos.x, pos.y, pos.z
                    )
                } else {
                    "fras: not a belt block in a line".to_string()
                },
            );
        }
        Some("remove") => {
            let Some(pos) = arg_pos(cmd, "pos") else {
                return;
            };
            with_level(dim, |l| l.belts.counters.remove(&pos));
            say(host, "fras: counter removed");
        }
        Some("clear") => {
            with_level(dim, |l| l.belts.counters.clear());
            say(host, "fras: counters cleared");
        }
        _ => {
            let v = counters_json(dim);
            say_json(host, "counters", &v);
        }
    }
}

fn counters_json(dim: Dim) -> Value {
    with_level(dim, |l| {
        let now = l.belts.ticks;
        let list: Vec<Value> = l
            .belts
            .counters
            .iter()
            .map(|(p, c)| {
                let window = now.saturating_sub(c.since).clamp(1, 1200);
                let recent = c.recent.iter().filter(|t| now - **t <= 1200).count();
                let per_min = recent as f64 * 1200.0 / window as f64;
                json!({
                    "pos": pos_json(*p),
                    "total": c.total,
                    "per_minute": per_min,
                    "ticks_counted": now.saturating_sub(c.since),
                })
            })
            .collect();
        Value::Array(list)
    })
}

// ---- Events --------------------------------------------------------------------------------

fn events_json(n: usize, kind: Option<&str>) -> Value {
    with(|s| {
        let list: Vec<Value> = s
            .debug
            .events
            .iter()
            .rev()
            .filter(|e| kind.is_none_or(|k| e.kind == k))
            .take(n)
            .map(|e| {
                json!({
                    "tick": e.tick, "dim": e.dim, "kind": e.kind,
                    "pos": pos_json(e.pos), "detail": e.detail,
                })
            })
            .collect();
        json!({ "totals": s.debug.totals, "events": list })
    })
}

// ---- Run and report ------------------------------------------------------------------------

fn overworld_time(host: &dyn Host) -> i64 {
    host.level("minecraft:overworld")
        .and_then(|l| l.game_time(host))
        .unwrap_or(0)
}

/// What the report compares: per level, recipes done per machine, belt
/// totals; event and fuel totals.
fn snapshot(host: &dyn Host) -> Value {
    let levels: Vec<Dim> = with(|s| s.levels.keys().copied().collect());
    let mut per = serde_json::Map::new();
    for dim in levels {
        let v = with_level(dim, |l| {
            let made: BTreeMap<String, u64> = l
                .machines
                .made
                .iter()
                .map(|(p, n)| (format!("{} {} {}", p.x, p.y, p.z), *n))
                .collect();
            let counters: BTreeMap<String, u64> = l
                .belts
                .counters
                .iter()
                .map(|(p, c)| (format!("{} {} {}", p.x, p.y, p.z), c.total))
                .collect();
            json!({
                "made": made,
                "delivered": l.belts.stats.delivered,
                "dropped": l.belts.stats.dropped,
                "diverted": l.belts.stats.diverted,
                "counters": counters,
                "items_on_belts": l.belts.iter().map(|ls| ls.line.count()).sum::<usize>(),
                "jammed_parts": l.mech.positions().into_iter()
                    .filter(|q| l.mech.part(*q).is_some_and(|p| p.jam.is_some())).count(),
            })
        });
        per.insert(dim.to_string(), v);
    }
    let totals = with(|s| s.debug.totals.clone());
    json!({ "time": overworld_time(host), "levels": per, "totals": totals })
}

fn start_run(host: &mut dyn ServerHost, level: LevelRef, n: u32) {
    let _ = level;
    let now = overworld_time(&*host);
    let baseline = snapshot(&*host);
    with(|s| {
        s.debug.run = Some(Run {
            until: now + n as i64,
            ticks: n,
            start: now,
            baseline,
            wall: Instant::now(),
        });
        s.debug.last_report = None;
    });
    let ok = host.commands().run_command(&format!("tick sprint {n}"));
    say(
        host,
        format!(
            "fras: running {n} ticks{}; /fras report when done",
            if ok { " (sprinting)" } else { "" }
        ),
    );
}

/// Each overworld tick: finish a run that reached its end.
pub fn check_run(host: &mut dyn ServerHost) {
    let now = overworld_time(&*host);
    let done = with(|s| match &s.debug.run {
        Some(r) if now >= r.until => s.debug.run.take(),
        _ => None,
    });
    let Some(run) = done else {
        return;
    };
    let end = snapshot(&*host);
    let report = report_json(&run, &end, now);
    let text = serde_json::to_string(&report).unwrap_or_default();
    let _ = write_file(
        "report",
        &serde_json::to_string_pretty(&report).unwrap_or_default(),
    );
    eprintln!("[fras] run report: {text}");
    with(|s| s.debug.last_report = Some(text));
    let line = Text::literal(format!(
        "[FRAS] run of {} ticks done: /fras report",
        run.ticks
    ));
    let levels: Vec<Dim> = with(|s| s.levels.keys().copied().collect());
    let players: Vec<mod_api::PlayerRef> = levels
        .into_iter()
        .filter_map(|d| host.level(d))
        .filter_map(|l| l.online_players(&*host))
        .flatten()
        .collect();
    for p in players {
        if p.permission_level(&*host).is_some_and(|l| l >= 2) {
            let _ = p.tell(host, line.clone());
        }
    }
}

fn diff_map(a: &Value, b: &Value) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(bm) = b.as_object() {
        for (k, v) in bm {
            let before = a.get(k).and_then(Value::as_u64).unwrap_or(0);
            let now = v.as_u64().unwrap_or(0);
            if now != before {
                out.insert(k.clone(), json!(now.saturating_sub(before)));
            }
        }
    }
    Value::Object(out)
}

fn report_json(run: &Run, end: &Value, now: i64) -> Value {
    let base = &run.baseline;
    let mut levels = serde_json::Map::new();
    if let Some(lm) = end["levels"].as_object() {
        for (dim, e) in lm {
            let b = &base["levels"][dim];
            let n = |k: &str| {
                e[k].as_u64()
                    .unwrap_or(0)
                    .saturating_sub(b[k].as_u64().unwrap_or(0))
            };
            levels.insert(
                dim.clone(),
                json!({
                    "made_by_machine": diff_map(&b["made"], &e["made"]),
                    "items_made": diff_map(&b["made"], &e["made"]).as_object()
                        .map_or(0, |m| m.values().filter_map(Value::as_u64).sum::<u64>()),
                    "delivered": n("delivered"),
                    "dropped": n("dropped"),
                    "diverted": n("diverted"),
                    "counted": diff_map(&b["counters"], &e["counters"]),
                    "items_on_belts": e["items_on_belts"],
                    "jammed_parts_now": e["jammed_parts"],
                }),
            );
        }
    }
    let totals = diff_map(&base["totals"], &end["totals"]);
    let t = |k: &str| totals.get(k).and_then(Value::as_u64).unwrap_or(0);
    json!({
        "ticks": run.ticks,
        "from": run.start,
        "to": now,
        "wall_seconds": run.wall.elapsed().as_secs_f64(),
        "jams": t("jam"),
        "overspeeds": t("overspeed"),
        "drops": t("drop"),
        "backups": t("backup"),
        "fuel_items_used": t("fuel"),
        "events": totals,
        "levels": levels,
    })
}

// ---- Rust ----------------------------------------------------------------------------------

fn rust(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    words: &[&str],
    cmd: &CommandInvocation,
) {
    let Some(pos) = arg_pos(cmd, "pos") else {
        return;
    };
    let Some(block) = block_at(&*host, level, pos).filter(|b| b.rusts()) else {
        say(host, "fras: not a rusting part");
        return;
    };
    match words.get(2).copied() {
        Some("set") => {
            let name = arg_str(cmd, "stage").unwrap_or_default();
            let Some(stage) = crate::sim::rust::Stage::from_name(&name) else {
                say(
                    host,
                    "fras: stages are clean, spotty, rusty, leaking, painted",
                );
                return;
            };
            super::rust::set_stage(host, level, dim, pos, block, stage);
            say(
                host,
                format!("fras: {} set to {}", block.name(), stage.name()),
            );
        }
        Some("tick") => {
            let n = arg_int(cmd, "times").unwrap_or(1).clamp(1, 100_000);
            for _ in 0..n {
                with_level(dim, |l| l.steam.rust_ticks.insert(pos));
                super::rust::ticks(host, level, dim);
            }
            let stage = super::rust::stage(&*host, level, pos, block);
            let count = with_level(dim, |l| l.steam.rust_counts.get(&pos).copied());
            say(
                host,
                format!(
                    "fras: {n} random ticks: stage {}, count {}",
                    stage.name(),
                    count.unwrap_or(0)
                ),
            );
        }
        _ => {}
    }
}

// ---- Perf ---------------------------------------------------------------------------------

fn perf_json() -> Value {
    with(|s| {
        let secs = s
            .debug
            .perf_since
            .map_or(0.0, |t| t.elapsed().as_secs_f64());
        let sections: serde_json::Map<String, Value> = s
            .debug
            .perf
            .iter()
            .map(|(k, (n, total, max))| {
                (
                    (*k).to_string(),
                    json!({
                        "calls": n,
                        "mean_us": if *n > 0 { total / *n as f64 } else { 0.0 },
                        "max_us": max,
                        "total_ms": total / 1000.0,
                    }),
                )
            })
            .collect();
        let parts: usize = s.levels.values().map(|l| l.mech.len()).sum();
        let lines: usize = s.levels.values().map(|l| l.belts.iter().count()).sum();
        json!({
            "seconds": secs,
            "sections": sections,
            "rot_parts": parts,
            "belt_lines": lines,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::Tag;

    #[test]
    fn nbt_dumps_as_readable_json() {
        let mut c = Compound::new();
        blocks::put_f64(&mut c, "needle", 59.5);
        c.insert("v".into(), Tag::Int(1));
        c.insert("name".into(), Tag::String("hi".into()));
        assert_eq!(nbt_json(&c), json!({"needle": 59.5, "v": 1, "name": "hi"}));
    }
}
