//! The copper golem crank (`specs/copper-golem.md`), a time-boxed prototype.
//!
//! - **Assigning** (spec 3): a player leading a copper golem uses the lead on
//!   a crank; the lead goes into the crank (it holds the golem now) and the
//!   golem walks to the working spot on the crank's handle side. A lead used
//!   on a crank that holds a golem takes the golem back on the lead. Breaking
//!   the crank lets it go, with its lead dropped.
//! - **Holding** (spec 3, 6): a brain behaviour on `minecraft:copper_golem`
//!   walks an assigned golem to its spot. At the spot the golem is snapped
//!   into place, turned to the crank, and its AI paused, which also stops its
//!   own errands (sorting items into chests).
//! - **Turning** (spec 4): a golem at its spot drives the crank with 8 Nm and
//!   16 RPM. When vanilla's weathering turns it into a statue the entity is
//!   gone and the crank stops.
//! - **Drawing** (spec 5): the crank's update tag names the golem; on the
//!   client `render_living_pre` hides its right arm and draws a copy of it
//!   pointing at the crank's grip.
//!
//! Saved: the golem's UUID on the crank (`golem`). The golem keeps its paused
//! AI with it (Horizon saves it), so both survive a reload.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use mod_api::{
    Activity, Behavior, BehaviorCx, BehaviorRegistrar, ClientHost, Compound, EntityPart, EntityRef,
    EntityUuid, Host, InteractionResult, ItemStack, LeashHolder, LevelRef, ModelPart,
    RenderLivingPreEvent, ServerHost,
};

use super::blocks::{self, Block};
use super::server::{self, Dim, with, with_level};
use crate::sim::geom::{Dir, Pos};
use crate::sim::golem::{self as rules, ARRIVE, GOLEM_STALL, GOLEM_TOP, LEASH_RANGE};
use crate::sim::rot::Drive;

pub const KIND: &str = "minecraft:copper_golem";

/// What the crank knows of its golem.
#[derive(Clone, Debug, Default)]
pub struct Link {
    pub uuid: u128,
    /// The golem's entity id while it is loaded.
    pub entity: Option<u32>,
    /// At its spot, turning the crank.
    pub working: bool,
    /// Vanilla's weathering stage, 0 to 3.
    pub stage: u8,
    /// Ticks the golem has been missing with its crank loaded.
    pub missing: u32,
    /// The spot holds a copper golem statue: the golem oxidised.
    pub statue: bool,
    /// What the crank's block entity last got: entity id, stage, working.
    pub synced: Option<(i32, u8, bool)>,
}

impl Link {
    pub fn status(&self) -> &'static str {
        if self.statue {
            "oxidised"
        } else if self.entity.is_none() {
            "missing"
        } else if self.working {
            "working"
        } else {
            "walking"
        }
    }
}

// ---- The brain behaviour ------------------------------------------------------

/// Assigned golems' spots by entity id, for the behaviour: the level tick
/// fills it and the brain reads it.
static SPOTS: Mutex<Option<HashMap<u32, [f64; 3]>>> = Mutex::new(None);
/// Brain ticks the behaviour has run, for the debug log: whether the brain
/// behaviour or the fallback walks the golem.
static BRAIN_TICKS: AtomicU64 = AtomicU64::new(0);

fn spot_of(id: u32) -> Option<[f64; 3]> {
    SPOTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()?
        .get(&id)
        .copied()
}

fn set_spot(id: u32, at: Option<[f64; 3]>) {
    let mut guard = SPOTS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    match at {
        Some(at) => {
            map.insert(id, at);
        }
        None => {
            map.remove(&id);
        }
    }
}

pub fn clear() {
    *SPOTS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The copper golem's part: one brain behaviour.
pub struct GolemPart;

#[mod_api::horizon_part]
impl EntityPart for GolemPart {
    fn register_behaviors(&self, behaviors: &mut BehaviorRegistrar<'_>) {
        // Core runs whatever else the brain does; priority 0 starts it first.
        if let Err(e) = behaviors.add(Activity::Core, 0, 40, 80, &[], WalkToCrank) {
            eprintln!("[fras] golem behaviour (core): {e}");
            if let Err(e) = behaviors.add(Activity::Idle, 0, 40, 80, &[], WalkToCrank) {
                eprintln!("[fras] golem behaviour (idle): {e}");
            }
        }
    }
}

/// Walks an assigned golem to its spot at the crank (spec 3).
struct WalkToCrank;

impl WalkToCrank {
    fn walk(cx: &mut BehaviorCx<'_>) -> bool {
        let id = cx.mob().entity().id();
        let Some(at) = spot_of(id) else {
            return false;
        };
        BRAIN_TICKS.fetch_add(1, Ordering::Relaxed);
        cx.set_walk_target(
            [
                at[0].floor() as i32,
                at[1].floor() as i32,
                at[2].floor() as i32,
            ],
            1.0,
            0,
        );
        true
    }
}

impl Behavior for WalkToCrank {
    fn name(&self) -> &'static str {
        "FrasWalkToCrank"
    }
    fn check_extra_start_conditions(&mut self, cx: &mut BehaviorCx<'_>) -> bool {
        spot_of(cx.mob().entity().id()).is_some()
    }
    fn start(&mut self, cx: &mut BehaviorCx<'_>) {
        Self::walk(cx);
    }
    fn can_still_use(&mut self, cx: &mut BehaviorCx<'_>) -> bool {
        spot_of(cx.mob().entity().id()).is_some()
    }
    fn tick(&mut self, cx: &mut BehaviorCx<'_>) {
        Self::walk(cx);
    }
}

// ---- Server ---------------------------------------------------------------------

fn uuid_str(uuid: u128) -> String {
    format!("{uuid:032x}")
}

fn parse_uuid(s: &str) -> Option<u128> {
    u128::from_str_radix(s, 16).ok()
}

/// The crank's block entity loaded: pick up its golem.
pub fn loaded(dim: Dim, pos: Pos, nbt: &Compound) {
    let Some(uuid) = blocks::get_str(nbt, "golem").and_then(|s| parse_uuid(&s)) else {
        return;
    };
    with_level(dim, |l| {
        if l.golems.get(&pos).is_none_or(|g| g.uuid != uuid) {
            l.golems.insert(
                pos,
                Link {
                    uuid,
                    ..Link::default()
                },
            );
        }
    });
}

/// The crank's chunk unloaded: forget the link until it loads again.
pub fn unloaded(dim: Dim, pos: Pos) {
    let gone = with_level(dim, |l| l.golems.remove(&pos));
    if let Some(Link {
        entity: Some(id), ..
    }) = gone
    {
        set_spot(id, None);
    }
}

fn is_golem(host: &dyn Host, e: EntityRef) -> bool {
    e.kind(host).is_some_and(|k| k.id(host) == KIND)
}

fn socket(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<Dir> {
    server::property(host, level, pos, "facing")
        .as_deref()
        .and_then(Dir::from_name)
}

/// A lead used on a crank (spec 3): hitch the golem the player leads to it,
/// or take the crank's golem back on the lead. None leaves the use to the
/// crank (no golem either way).
pub fn lead_on_crank(
    host: &mut dyn ServerHost,
    click: &mod_api::BlockClick<'_>,
) -> Option<InteractionResult> {
    let player = host.player(click.player)?;
    let level = player.level(&*host)?;
    let dim = server::dim_of(&*host, level)?;
    let pos = Pos::from_array(click.pos);
    let held = with_level(dim, |l| l.golems.get(&pos).cloned());
    if let Some(link) = held {
        // Take it back on the lead.
        if let Some(e) = host.entity_by_uuid(EntityUuid(link.uuid)) {
            release(host, level, dim, pos, false);
            let _ = e.leash_to(host, player.entity());
        } else {
            release(host, level, dim, pos, false);
        }
        super::tools::bar(
            host,
            player,
            super::tools::tr("fras.golem.unhitched", vec![]),
        );
        return Some(InteractionResult::Success);
    }
    let me = player.entity();
    let c = [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5];
    let r = LEASH_RANGE;
    let near = level.entities_in(
        &*host,
        [c[0] - r, c[1] - r, c[2] - r],
        [c[0] + r, c[1] + r, c[2] + r],
    )?;
    let golem = near.into_iter().find(|e| {
        is_golem(&*host, *e) && e.leash_holder(&*host) == Some(LeashHolder::Entity(me))
    })?;
    let Some(sock) = socket(&*host, level, pos) else {
        return Some(InteractionResult::Pass);
    };
    if rules::spot(pos, sock).is_none() {
        super::tools::bar(host, player, super::tools::tr("fras.golem.no_spot", vec![]));
        return Some(InteractionResult::Success);
    }
    let Some(uuid) = golem.uuid(host) else {
        return Some(InteractionResult::Pass);
    };
    // A golem held by another crank leaves it.
    let other: Option<Pos> = with_level(dim, |l| {
        l.golems
            .iter()
            .find(|(_, g)| g.uuid == uuid)
            .map(|(p, _)| *p)
    });
    if let Some(other) = other {
        release(host, level, dim, other, false);
    }
    // The lead goes into the crank: it holds the golem now.
    let _ = golem.detach_leash(host, false);
    let mut fields = Compound::new();
    blocks::put_str(&mut fields, "golem", &uuid_str(uuid));
    server::merge(host, level, pos, fields);
    with_level(dim, |l| {
        l.golems.insert(
            pos,
            Link {
                uuid,
                entity: Some(golem.id()),
                ..Link::default()
            },
        );
    });
    let time = level.game_time(&*host).unwrap_or(0);
    super::debug::event(dim, time, "golem", pos, "assigned".into());
    super::tools::bar(host, player, super::tools::tr("fras.golem.hitched", vec![]));
    Some(InteractionResult::Success)
}

/// Let the crank's golem go: its AI back on, its spot forgotten, the crank's
/// record cleared, and with `drop_lead` its lead dropped where it stands.
pub fn release(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, pos: Pos, drop_lead: bool) {
    let Some(link) = with_level(dim, |l| l.golems.remove(&pos)) else {
        return;
    };
    let entity = link
        .entity
        .and_then(|id| host.entity(id))
        .or_else(|| host.entity_by_uuid(EntityUuid(link.uuid)));
    if let Some(e) = entity {
        set_spot(e.id(), None);
        if let Some(mob) = host.mob(e) {
            let _ = mob.set_ai_paused(host, false);
        }
        if drop_lead && let Some(lead) = super::keys().and_then(|k| ItemStack::new(k.lead, 1)) {
            let _ = e.spawn_at_location(host, lead, 0.5);
        }
    }
    with_level(dim, |l| {
        if !l.cranks.contains_key(&pos) {
            l.mech.set_drive(pos, Drive::default());
        }
    });
    if level.is_loaded(&*host, pos.to_array())
        && server::block_at(&*host, level, pos) == Some(Block::Crank)
    {
        let mut fields = Compound::new();
        blocks::put_str(&mut fields, "golem", "");
        blocks::put_i32(&mut fields, "golem_id", -1);
        blocks::put_bool(&mut fields, "golem_on", false);
        server::merge(host, level, pos, fields);
    }
    let time = level.game_time(&*host).unwrap_or(0);
    super::debug::event(dim, time, "golem", pos, "released".into());
}

/// Ticks a golem may be missing with its crank loaded before the crank lets
/// it go (killed, or carried off by something).
const MISSING_TICKS: u32 = 200;

/// Every level tick, before the networks: keep each assigned golem at its
/// crank and note which turn.
pub fn tick(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let links: Vec<(Pos, Link)> = with_level(dim, |l| {
        l.golems.iter().map(|(p, g)| (*p, g.clone())).collect()
    });
    if links.is_empty() {
        return;
    }
    let ticks = with(|s| s.ticks);
    let time = level.game_time(&*host).unwrap_or(0);
    for (pos, mut link) in links {
        if !level.is_loaded(&*host, pos.to_array()) {
            continue;
        }
        if server::block_at(&*host, level, pos) != Some(Block::Crank) {
            release(host, level, dim, pos, true);
            continue;
        }
        let Some((at, yaw)) = socket(&*host, level, pos).and_then(|s| rules::stand(pos, s)) else {
            release(host, level, dim, pos, true);
            continue;
        };
        let entity = host
            .entity_by_uuid(EntityUuid(link.uuid))
            .filter(|e| is_golem(&*host, *e) && e.level(&*host) == Some(level));
        let was_working = link.working;
        match entity {
            None => {
                if let Some(id) = link.entity.take() {
                    set_spot(id, None);
                }
                link.working = false;
                link.missing += 1;
                let spot = Pos::new(
                    at[0].floor() as i32,
                    at[1].floor() as i32,
                    at[2].floor() as i32,
                );
                let statue = level
                    .block(&*host, spot.to_array())
                    .is_some_and(|b| b.id(&*host).contains("copper_golem_statue"));
                if statue && !link.statue {
                    super::debug::event(dim, time, "golem", pos, "statue".into());
                }
                link.statue = statue;
                if !statue && link.missing > MISSING_TICKS {
                    release(host, level, dim, pos, false);
                    continue;
                }
            }
            Some(e) => {
                link.missing = 0;
                link.statue = false;
                link.entity = Some(e.id());
                let p = e.position(&*host).unwrap_or(at);
                let flat = ((p[0] - at[0]).powi(2) + (p[2] - at[2]).powi(2)).sqrt();
                if flat > LEASH_RANGE + 2.0 || (p[1] - at[1]).abs() > 4.0 {
                    super::debug::event(dim, time, "golem", pos, "too far".into());
                    release(host, level, dim, pos, true);
                    continue;
                }
                if (ticks % 20 == 0 || link.synced.is_none())
                    && let Some(nbt) = e.nbt(host)
                {
                    link.stage =
                        blocks::get_str(&nbt, "weather_state").map_or(0, |s| rules::stage(&s));
                }
                let mob = host.mob(e);
                if flat <= ARRIVE && (p[1] - at[1]).abs() < 0.6 {
                    set_spot(e.id(), None);
                    if !link.working {
                        if let Some(mob) = mob {
                            let _ = mob.set_ai_paused(host, true);
                        }
                        link.working = true;
                        super::debug::event(dim, time, "golem", pos, "working".into());
                    }
                    let r = e.rotation(&*host).unwrap_or([yaw, 0.0]);
                    if flat > 0.01 || (r[0] - yaw).abs() > 0.5 {
                        let _ = e.teleport_to(host, at, Some([yaw, 0.0]));
                    }
                    let _ = e.set_head_yaw(host, yaw);
                } else {
                    if link.working {
                        if let Some(mob) = mob {
                            let _ = mob.set_ai_paused(host, false);
                        }
                        link.working = false;
                        super::debug::event(dim, time, "golem", pos, "pushed off".into());
                    }
                    set_spot(e.id(), Some(at));
                    // If the brain behaviour hasn't walked it lately, walk it
                    // with the mob's navigation instead (and say so).
                    if ticks % 40 == 0 {
                        let brain = BRAIN_TICKS.swap(0, Ordering::Relaxed);
                        if brain == 0
                            && let Some(mob) = mob
                        {
                            let _ = mob.move_to(host, at, 1.0);
                            super::debug::event(dim, time, "golem", pos, "fallback walk".into());
                        }
                    }
                }
            }
        }
        if was_working != link.working {
            with_level(dim, |l| {
                if !link.working && !l.cranks.contains_key(&pos) {
                    l.mech.set_drive(pos, Drive::default());
                }
            });
        }
        let sync = (
            link.entity.map_or(-1, |id| id as i32),
            link.stage,
            link.working,
        );
        if link.synced != Some(sync) {
            let mut fields = Compound::new();
            blocks::put_i32(&mut fields, "golem_id", sync.0);
            blocks::put_i32(&mut fields, "golem_stage", sync.1 as i32);
            blocks::put_bool(&mut fields, "golem_on", sync.2);
            server::merge(host, level, pos, fields);
            link.synced = Some(sync);
        }
        with_level(dim, |l| {
            if l.golems.contains_key(&pos) {
                l.golems.insert(pos, link);
            }
        });
    }
}

/// Inside the network step: working golems drive their cranks, unless a
/// player is turning it (the player's push wins).
pub fn drives(l: &mut server::Level) {
    for (pos, g) in &l.golems {
        if g.working && !l.cranks.contains_key(pos) {
            l.mech.set_drive(
                *pos,
                Drive {
                    stall: GOLEM_STALL,
                    top: GOLEM_TOP,
                    dir: 1.0,
                },
            );
        }
    }
}

/// The inspection overlay's line for a crank with a golem.
pub fn line(dim: Dim, pos: Pos) -> Option<mod_api::Text> {
    let link = with_level(dim, |l| l.golems.get(&pos).cloned())?;
    Some(super::tools::tr(
        &format!("fras.golem.{}", link.status()),
        vec![],
    ))
}

/// The debug dump's view of the crank's golem.
pub fn debug(dim: Dim, pos: Pos) -> Option<String> {
    let link = with_level(dim, |l| l.golems.get(&pos).cloned())?;
    Some(format!(
        "{{\"uuid\":\"{}\",\"entity\":{},\"status\":\"{}\",\"stage\":{}}}",
        uuid_str(link.uuid),
        link.entity.map_or(-1, |e| e as i64),
        link.status(),
        link.stage
    ))
}

// ---- Client ---------------------------------------------------------------------

/// A golem a crank shows working: the crank, its socket and the golem's
/// weathering stage.
#[derive(Copy, Clone, Debug)]
struct Shown {
    crank: Pos,
    socket: Dir,
    stage: u8,
}

static SHOWN: Mutex<Option<HashMap<u32, Shown>>> = Mutex::new(None);

/// The crank drew: note the golem it shows working, if any.
pub fn note(pos: Pos, socket: Dir, nbt: &Compound) {
    let id = blocks::get_i32(nbt, "golem_id").unwrap_or(-1);
    let on = blocks::get_bool(nbt, "golem_on");
    let stage = blocks::get_i32(nbt, "golem_stage").unwrap_or(0).clamp(0, 3) as u8;
    let mut guard = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.retain(|_, s| s.crank != pos);
    if id >= 0 && on {
        map.insert(
            id as u32,
            Shown {
                crank: pos,
                socket,
                stage,
            },
        );
    }
}

/// The copper golem's right arm from vanilla's model: the part, its
/// shoulder (model texels, its parents' offsets added), the middle of the
/// arm across, and its length to the hand.
struct Arm {
    part: ModelPart,
    pivot: [f32; 3],
    across: [f64; 2],
    length: f64,
}

static ARM: Mutex<Option<Option<Arm>>> = Mutex::new(None);

fn find(part: &ModelPart, name: &str, at: [f32; 3]) -> Option<(ModelPart, [f32; 3])> {
    for (n, child) in part.children() {
        let here = [at[0] + child.x, at[1] + child.y, at[2] + child.z];
        if n == name {
            return Some((child.clone(), here));
        }
        if let Some(found) = find(child, name, here) {
            return Some(found);
        }
    }
    None
}

fn bake_arm(host: &dyn ClientHost) -> Option<Arm> {
    let root = host.bake_layer("minecraft:copper_golem#main")?;
    let (part, pivot) = find(&root, "right_arm", [root.x, root.y, root.z])?;
    let verts: Vec<[f32; 3]> = part
        .polygons
        .iter()
        .flat_map(|p| p.vertices.iter().map(|v| v.pos))
        .collect();
    if verts.is_empty() {
        return None;
    }
    let n = verts.len() as f64;
    let mid = |i: usize| verts.iter().map(|v| v[i] as f64).sum::<f64>() / n;
    let bottom = verts.iter().map(|v| v[1] as f64).fold(f64::MIN, f64::max);
    eprintln!(
        "[fras] copper golem right_arm: pivot {pivot:?}, across ({:.2}, {:.2}), to {bottom:.2}",
        mid(0),
        mid(2)
    );
    Some(Arm {
        part,
        pivot,
        across: [mid(0), mid(2)],
        // The hand's middle, a texel and a half above the arm's end.
        length: (bottom - 1.5).max(1.0),
    })
}

/// Before a copper golem is drawn: if it turns a crank, hide its right arm
/// and draw it reaching for the crank's grip (spec 5).
pub fn render(host: &dyn ClientHost, event: &mut RenderLivingPreEvent<'_>) {
    if event.entity.kind != KIND {
        return;
    }
    let shown = SHOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(&event.entity.id).copied());
    let Some(shown) = shown else {
        return;
    };
    let mut guard = ARM.lock().unwrap_or_else(|e| e.into_inner());
    let Some(arm) = guard.get_or_insert_with(|| bake_arm(host)) else {
        return;
    };
    let now = event.draw.animation_time();
    let angle = super::draw::angle(shown.crank, 0, now).unwrap_or(0.0);
    let target = rules::to_model(rules::grip(shown.socket, angle));
    let line = [
        arm.pivot[0] as f64 + arm.across[0],
        arm.pivot[1] as f64,
        arm.pivot[2] as f64 + arm.across[1],
    ];
    let pose = rules::arm_pose(line, arm.length, target);
    if event.hidden.hide_named("right_arm").is_err() {
        return;
    }
    let mut part = arm.part.clone();
    part.x = arm.pivot[0];
    part.y = arm.pivot[1];
    part.z = arm.pivot[2];
    part.x_rot = pose.x_rot;
    part.y_rot = 0.0;
    part.z_rot = pose.z_rot;
    part.y_scale = pose.stretch;
    let draw = &mut *event.draw;
    draw.push();
    draw.scale([1.0, -1.0, -1.0]);
    draw.translate([0.0, -1.501, 0.0]);
    if !draw.model_part(&part, rules::texture(shown.stage), 0xFF_FFFF) {
        eprintln!(
            "[fras] copper golem arm: no sprite {}",
            rules::texture(shown.stage)
        );
    }
    draw.pop();
}
