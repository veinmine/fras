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
    // A § code in a parent component doesn't carry into its children: the
    // colour goes on the component's style.
    let mut style = mod_api::text::TextStyle::default();
    style.color = Some("red".into());
    Text::literal("").append(t).with_style(style)
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
        Kind::Governor => "governor",
        Kind::Damper => "furnace_damper",
        Kind::Bellows => "bellows",
        Kind::Exchanger => "heat_exchanger",
        Kind::Radiator { copper: true } => "copper_radiator",
        Kind::Radiator { copper: false } => "iron_radiator",
        Kind::Drain => "drain",
        Kind::Grinder => "grinder",
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
                // Heat with bellows and dampers (regulation spec 6).
                if let Some(f) = w.fire(pos)
                    && f.kw > 0.0
                {
                    out.push(tr(
                        "fras.inspect.fire",
                        vec![
                            num(f.kw),
                            num(1.0 + f.boost),
                            num(f.damper * 100.0),
                            num(f.heat()),
                        ],
                    ));
                    if f.damper <= 0.0 {
                        out.push(tr("fras.inspect.banked", vec![]));
                    } else if f.damper < 1.0 {
                        out.push(tr("fras.inspect.burn_rate", vec![num(f.damper * 100.0)]));
                    }
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
                    let g = w.engine_throttle(pos);
                    if g < 0.995 {
                        out.push(tr("fras.inspect.throttled", vec![num(g * 100.0)]));
                    }
                    if block == Block::SteamCylinder
                        && let Some((vented, piped)) = w.exhaust_at(pos)
                        && vented + piped > 0.05
                    {
                        out.push(tr("fras.inspect.exhaust", vec![num(vented), num(piped)]));
                        // Piped but not all taken: the exchangers are short
                        // (cooling spec 4).
                        if vented > 0.05 && piped > 0.05 {
                            out.push(red(tr("fras.inspect.exhaust_short", vec![])));
                        }
                    }
                }
                None if block == Block::SteamCylinder => {
                    out.push(red(tr("fras.inspect.no_flywheel", vec![])));
                }
                None => {}
            },
            Block::Governor => {
                if let Some((flow, cap)) = w.flow_at(pos) {
                    out.push(tr("fras.inspect.carrying", vec![num(flow), num(cap)]));
                }
                if let Some(g) = w.governor_info(pos) {
                    out.push(tr(
                        "fras.inspect.governor",
                        vec![
                            num(g.target),
                            g.sens.to_string(),
                            num(crate::sim::steam::govern::band(g.sens)),
                        ],
                    ));
                    out.push(tr(
                        "fras.inspect.governor_state",
                        vec![num(g.speed), num(g.ball), num(g.throttle * 100.0)],
                    ));
                    out.push(tr(
                        "fras.inspect.governor_engines",
                        vec![g.engines.to_string()],
                    ));
                    if g.speed < 0.5 {
                        out.push(red(tr("fras.inspect.governor_still", vec![])));
                    } else if g.swing > crate::sim::steam::govern::band(g.sens) / 2.0 {
                        out.push(red(tr("fras.inspect.hunting", vec![num(g.swing / 2.0)])));
                    }
                    if g.vent_behind {
                        out.push(red(tr("fras.inspect.vent_behind", vec![])));
                    }
                    if !g.on_steam {
                        out.push(red(tr("fras.inspect.governor_steam_only", vec![])));
                    }
                }
            }
            Block::FurnaceDamper => {
                if let Some(d) = w.damper_info(pos) {
                    out.push(tr("fras.wrench.damper_set", vec![num(d.set)]));
                    match d.bar {
                        Some(bar) => out.push(tr(
                            "fras.inspect.damper",
                            vec![num(bar), num(d.opening * 100.0)],
                        )),
                        None => out.push(red(tr("fras.inspect.damper_alone", vec![]))),
                    }
                    if d.stuck {
                        out.push(red(tr("fras.inspect.damper_stuck", vec![])));
                    } else if d.powered {
                        out.push(tr("fras.inspect.damper_redstone", vec![]));
                    } else if d.opening <= 0.0 {
                        out.push(tr("fras.inspect.banked", vec![]));
                    }
                }
            }
            Block::Bellows => {
                let rpm = l.mech.speed(pos).map_or(0.0, |s| s.0.abs());
                out.push(tr(
                    "fras.inspect.bellows",
                    vec![
                        num(rpm),
                        num(crate::sim::steam::bellows::MIN_RPM),
                        num(crate::sim::steam::bellows::MAX_RPM),
                        num(crate::sim::steam::bellows::boost(rpm) * 100.0),
                    ],
                ));
                match w.bellows_target(pos) {
                    None => {
                        let facing = server::property(host, level, pos, "facing")
                            .as_deref()
                            .and_then(Dir::from_name)
                            .unwrap_or(Dir::North);
                        let key = if super::server::block_at(host, level, pos.offset(facing))
                            == Some(Block::Firebox)
                        {
                            "fras.inspect.bellows_second"
                        } else {
                            "fras.inspect.bellows_no_firebox"
                        };
                        out.push(red(tr(key, vec![])));
                    }
                    Some(f) => {
                        if w.fire(f).is_none_or(|f| f.kw <= 0.0) {
                            out.push(tr("fras.inspect.bellows_cold", vec![]));
                        }
                    }
                }
            }
            Block::HeatExchanger => {
                if let Some(e) = w.exchanger_info(pos) {
                    out.push(tr(
                        "fras.inspect.exchanger",
                        vec![
                            num(e.power),
                            e.radiators.0.to_string(),
                            e.radiators.1.to_string(),
                        ],
                    ));
                    match e.inlet {
                        Some(c) => out.push(Text::translate(
                            "fras.inspect.exchanger_flow",
                            vec![
                                content_name(Some(c)),
                                Text::literal(num(e.took)),
                                Text::literal(num(e.capacity)),
                            ],
                        )),
                        None => out.push(tr("fras.inspect.exchanger_idle", vec![])),
                    }
                    if e.inlet == Some(crate::sim::fluid::Content::Exhaust)
                        && e.blocked.is_none()
                        && e.capacity > 0.0
                        && e.took >= e.capacity - 1e-3
                    {
                        out.push(red(tr("fras.inspect.exchanger_short", vec![])));
                    }
                    match e.blocked {
                        Some(crate::sim::plant::ExBlock::NoOutlet) => {
                            out.push(red(tr("fras.inspect.no_outlet", vec![])))
                        }
                        Some(crate::sim::plant::ExBlock::OutletFull) => {
                            out.push(red(tr("fras.inspect.outlet_full", vec![])))
                        }
                        Some(crate::sim::plant::ExBlock::OutletCarries(c)) => {
                            out.push(red(Text::translate(
                                "fras.inspect.outlet_carries",
                                vec![content_name(Some(c))],
                            )))
                        }
                        None => {}
                    }
                }
            }
            Block::Drain => match w.drain_flow(pos) {
                Some((c, f)) if f > 1e-6 => out.push(Text::translate(
                    "fras.inspect.drain",
                    vec![Text::literal(num(f)), content_name(Some(c))],
                )),
                _ => out.push(tr("fras.inspect.drain_idle", vec![])),
            },
            Block::Grinder => {
                if let Some(c) = w.cooling_info(pos) {
                    let key = if c.cooled {
                        "fras.inspect.cooled"
                    } else if c.demand > 0.0 && c.received + 1e-6 < c.demand {
                        "fras.inspect.cooling_short"
                    } else {
                        "fras.inspect.not_cooled"
                    };
                    out.push(tr(key, vec![num(c.received), num(c.demand)]));
                    if !c.cooled && w.cool_buffer_full(pos) {
                        out.push(red(tr("fras.inspect.hot_water_stuck", vec![])));
                    }
                }
            }
            _ => {}
        }
        // Leaks on this part's network (rust spec 9).
        let (n, total) = w.net_leaks(pos);
        if n > 0 {
            out.push(red(tr(
                "fras.inspect.leaks",
                vec![n.to_string(), num(total)],
            )));
        }
    });
}
