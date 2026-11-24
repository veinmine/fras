//! The wrench overlay for steam parts (steam and heat spec 11, fluids spec
//! 9, multiblocks spec 7).

use mod_api::{Host, LevelRef, Text};

use super::blocks::Block;
use super::num;
use super::server::{self, Dim, with_level};
use crate::sim::fluid::Content;
use crate::sim::fluid::pump;
use crate::sim::geom::{Dir, Pos};
use crate::sim::multi::ShellKind;
use crate::sim::multi::shape::Refusal;
use crate::sim::plant::Kind;
use crate::sim::steam::fail;

fn tr(key: &str, args: Vec<String>) -> Text {
    Text::translate(key, args.into_iter().map(Text::literal).collect())
}

fn red(t: Text) -> Text {
    // Text styles can't be built from a mod: a § colour code does it.
    Text::literal("§c").append(t)
}

fn kind_name(kind: Kind) -> Text {
    let name = match kind {
        Kind::Shell(ShellKind::CopperBoiler) => "copper_boiler",
        Kind::Shell(ShellKind::IronBoiler) => "iron_boiler",
        Kind::Shell(ShellKind::Tank) => "storage_tank",
        Kind::Pipe(crate::sim::plant::PipeMat::Copper) => "copper_pipe",
        Kind::Pipe(crate::sim::plant::PipeMat::Iron) => "iron_pipe",
        Kind::Valve { redstone: false } => "manual_valve",
        Kind::Valve { redstone: true } => "redstone_valve",
        Kind::Vent { emergency: false } => "steam_release",
        Kind::Vent { emergency: true } => "emergency_valve",
        Kind::Cylinder => "steam_cylinder",
        Kind::Dome => "steam_dome",
        Kind::Firebox => "firebox",
        Kind::Pump => "pump",
        Kind::Flywheel => "flywheel",
    };
    Text::translate(format!("block.fras.{name}"), vec![])
}

fn content_name(c: Option<Content>) -> Text {
    Text::translate(
        format!("fras.content.{}", c.map_or("empty", Content::name)),
        vec![],
    )
}

/// The steam overlay's lines for a plant block (or the part a gauge is on).
pub fn lines(
    host: &dyn Host,
    level: LevelRef,
    dim: Dim,
    pos: Pos,
    block: Block,
    out: &mut Vec<Text>,
) {
    let damaged = server::property(host, level, pos, "damaged").as_deref() == Some("true");
    with_level(dim, |l| {
        let w = &l.steam.works;
        if damaged {
            let key = if block.is_shell() {
                "fras.inspect.damaged"
            } else {
                "fras.inspect.burst"
            };
            out.push(red(Text::translate(
                key,
                vec![super::tools::cost_text(&super::tools::repair_cost(block))],
            )));
        }
        if !w.contains(pos) && block.is_plant() {
            out.push(tr("fras.inspect.not_connected", vec![]));
            return;
        }
        if w.partial(pos) {
            out.push(red(tr("fras.inspect.partial", vec![])));
        } else if block.is_plant() && w.paused(pos) && w.refusal(pos).is_none() {
            out.push(tr("fras.inspect.plant_paused", vec![]));
        }
        // Why a group doesn't form.
        if let Some(r) = w.refusal(pos) {
            match r {
                Refusal::TooBig(n) => {
                    let kind = block.plant_kind().and_then(|k| match k {
                        Kind::Shell(s) => Some(s),
                        _ => None,
                    });
                    out.push(red(Text::translate(
                        "fras.inspect.too_big",
                        vec![
                            Text::literal(n.to_string()),
                            Text::translate(
                                format!(
                                    "fras.inspect.holds.{}",
                                    kind.map_or("tank", ShellKind::name)
                                ),
                                vec![],
                            ),
                            Text::literal(kind.map_or(27, ShellKind::limit).to_string()),
                        ],
                    )));
                }
                Refusal::NotCuboid { .. } => out.push(red(tr("fras.inspect.not_cuboid", vec![]))),
            }
        }
        // A formed boiler or tank.
        if let Some(s) = w.shell_info(pos) {
            out.push(tr(
                "fras.inspect.shell",
                vec![
                    s.size[0].to_string(),
                    s.size[1].to_string(),
                    s.size[2].to_string(),
                    s.blocks.to_string(),
                ],
            ));
            if s.kind.is_boiler() {
                let pct = s.water / s.water_capacity.max(1.0) * 100.0;
                out.push(tr(
                    "fras.inspect.water",
                    vec![num(s.water), num(s.water_capacity), num(pct)],
                ));
                if s.water <= 1e-6 && s.temp > 100.5 {
                    out.push(red(tr("fras.inspect.dry", vec![num(s.temp)])));
                } else {
                    out.push(tr("fras.inspect.temperature", vec![num(s.temp)]));
                }
                out.push(tr("fras.inspect.boiling", vec![num(s.boiled)]));
                out.push(tr(
                    "fras.inspect.heat_in",
                    vec![num(s.heat), s.fireboxes.to_string()],
                ));
                if s.domes == 0 {
                    out.push(red(tr("fras.inspect.no_dome", vec![])));
                } else {
                    out.push(tr("fras.inspect.domes", vec![s.domes.to_string()]));
                }
            } else {
                out.push(Text::translate(
                    "fras.inspect.tank",
                    vec![content_name(s.content)],
                ));
                if s.content != Some(Content::Steam) {
                    out.push(tr(
                        "fras.inspect.water",
                        vec![
                            num(s.water),
                            num(s.water_capacity),
                            num(s.water / s.water_capacity.max(1.0) * 100.0),
                        ],
                    ));
                }
            }
            if s.timer > 0.0 {
                out.push(red(tr(
                    "fras.inspect.warning_timer",
                    vec![num(((1.0 - s.timer) * fail::WARN_SECONDS).max(0.0))],
                )));
            }
        }
        // The steam network.
        if let Some(n) = w.steam_info(pos) {
            let by = n.red_by.map_or(Text::literal("-"), kind_name);
            out.push(Text::translate(
                "fras.inspect.pressure",
                vec![Text::literal(num(n.bar)), Text::literal(num(n.red)), by],
            ));
            out.push(tr(
                "fras.inspect.steam_flow",
                vec![num(n.capacity), num(n.steam_in), num(n.steam_out)],
            ));
        }
        match block {
            Block::CopperPipe | Block::IronPipe | Block::ManualValve | Block::RedstoneValve => {
                out.push(Text::translate(
                    "fras.inspect.content",
                    vec![content_name(w.content_at(pos))],
                ));
                if let Some((flow, cap)) = w.flow_at(pos) {
                    out.push(tr("fras.inspect.carrying", vec![num(flow), num(cap)]));
                }
                if block == Block::ManualValve
                    && let Some(p) = w.part(pos)
                {
                    out.push(tr(
                        "fras.inspect.opening",
                        vec![num(p.spec.opening * 100.0)],
                    ));
                }
            }
            Block::SteamRelease | Block::EmergencyValve => {
                let v = w.vent_state(pos);
                let key = if v.open {
                    "fras.inspect.vent_open"
                } else if v.simmer {
                    "fras.inspect.vent_simmer"
                } else {
                    "fras.inspect.vent_closed"
                };
                out.push(tr(key, vec![num(v.rate)]));
                if block == Block::EmergencyValve
                    && let Some(p) = w.part(pos)
                {
                    out.push(tr("fras.inspect.set_pressure", vec![num(p.spec.set)]));
                }
            }
            Block::SteamDome => {
                let below = pos.offset(Dir::Down);
                if w.shell_at(below).is_none_or(|s| !s.is_boiler()) {
                    out.push(red(tr("fras.inspect.dome_alone", vec![])));
                }
            }
            Block::Firebox => {
                let n = w.firebox_boilers(pos);
                if n == 0 {
                    out.push(red(tr("fras.inspect.heating_nothing", vec![])));
                } else {
                    out.push(tr("fras.inspect.heating", vec![n.to_string()]));
                }
            }
            Block::Pump => {
                let rpm = l.mech.speed(pos).map_or(0.0, |s| s.0.abs());
                let flow = w.pump_flow(pos);
                out.push(tr(
                    "fras.inspect.pump",
                    vec![num(flow), num(pump::MIN_RPM), num(pump::MAX_RPM)],
                ));
                if rpm < pump::MIN_RPM {
                    out.push(tr("fras.inspect.pump_slow", vec![]));
                } else if flow <= 1e-6 {
                    out.push(tr("fras.inspect.pump_idle", vec![]));
                }
            }
            Block::SteamCylinder | Block::Flywheel => match w.engine_info(pos) {
                Some(e) => {
                    out.push(tr(
                        "fras.inspect.engine",
                        vec![num(e.stall), num(e.top), e.cylinders.to_string()],
                    ));
                    if e.fill < 0.99 {
                        out.push(red(tr(
                            "fras.inspect.starved",
                            vec![num(e.fill * 100.0), num(e.demand)],
                        )));
                    } else {
                        out.push(tr("fras.inspect.demand", vec![num(e.demand)]));
                    }
                    let key = if e.reverse {
                        "fras.inspect.reverse"
                    } else {
                        "fras.inspect.forward"
                    };
                    out.push(tr(key, vec![]));
                }
                None if block == Block::SteamCylinder => {
                    out.push(red(tr("fras.inspect.no_flywheel", vec![])));
                }
                None => {}
            },
            _ => {}
        }
    });
}
