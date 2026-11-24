//! Pipe connections and the hand interactions of valves, vents and vessels
//! (fluids spec 3.3, 6 and 8).

use mod_api::{BlockClick, Compound, Host, InteractionResult, ItemStack, LevelRef, ServerHost};

use super::blocks::{self, Block};
use super::server::{self, Dim, block_at, with_level};
use super::steam;
use super::world::set_property;
use crate::sim::fluid::Content;
use crate::sim::fluid::valve;
use crate::sim::geom::{Axis, Dir, Pos};

fn dir_prop(host: &dyn Host, level: LevelRef, pos: Pos, key: &str, default: Dir) -> Dir {
    server::property(host, level, pos, key)
        .as_deref()
        .and_then(Dir::from_name)
        .unwrap_or(default)
}

fn flag(host: &dyn Host, level: LevelRef, pos: Pos, key: &str) -> bool {
    server::property(host, level, pos, key).as_deref() == Some("true")
}

/// What the block at `q` offers a pipe touching its face `face`: `None` if
/// it doesn't connect there, else the content it brings (none for a pipe on
/// an empty network or an empty tank).
pub fn offers(
    host: &dyn Host,
    level: LevelRef,
    dim: Dim,
    q: Pos,
    face: Dir,
) -> Option<Option<Content>> {
    let block = block_at(host, level, q)?;
    let net = || with_level(dim, |l| l.steam.works.content_at(q));
    match block {
        Block::CopperPipe | Block::IronPipe => Some(net()),
        Block::ManualValve | Block::RedstoneValve => {
            let axis = server::property(host, level, q, "axis")
                .as_deref()
                .and_then(Axis::from_name)
                .unwrap_or(Axis::X);
            (face.axis() == axis).then(net)
        }
        Block::SteamDome => (face != Dir::Down).then_some(Some(Content::Steam)),
        Block::CopperBoiler | Block::IronBoiler => {
            (face != Dir::Up).then_some(Some(Content::Water))
        }
        Block::StorageTank => Some(net()),
        Block::SteamCylinder => (face == dir_prop(host, level, q, "facing", Dir::North).opposite())
            .then_some(Some(Content::Steam)),
        Block::SteamRelease | Block::EmergencyValve => {
            (face == dir_prop(host, level, q, "facing", Dir::Down)).then_some(Some(Content::Steam))
        }
        Block::Pump => (face == dir_prop(host, level, q, "facing", Dir::North).opposite())
            .then_some(Some(Content::Water)),
        _ => None,
    }
}

/// Whether two contents may share a network.
fn agree(a: Option<Content>, b: Option<Content>) -> bool {
    a.is_none() || b.is_none() || a == b
}

/// Connect the pipe at `p` through `face` if the neighbour takes it and the
/// contents agree; `content` is what the pipe's network holds so far.
fn try_connect(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    p: Pos,
    face: Dir,
    content: &mut Option<Content>,
) -> bool {
    let q = p.offset(face);
    let Some(theirs) = offers(&*host, level, dim, q, face.opposite()) else {
        return false;
    };
    if !agree(*content, theirs) {
        return false;
    }
    *content = content.or(theirs);
    set_property(host, level, p, face.name(), "true");
    if block_at(&*host, level, q).is_some_and(Block::is_pipe) {
        set_property(host, level, q, face.opposite().name(), "true");
    }
    true
}

/// A block was placed at `pos`: a pipe connects every face it can; any
/// other block gets the pipes around it connected to it.
pub fn connect_around(host: &mut dyn ServerHost, level: LevelRef, dim: Dim, pos: Pos) {
    let Some(block) = block_at(&*host, level, pos) else {
        return;
    };
    if block.is_pipe() {
        let mut content = with_level(dim, |l| l.steam.works.content_at(pos));
        for d in Dir::ALL {
            if !flag(&*host, level, pos, d.name()) {
                try_connect(host, level, dim, pos, d, &mut content);
            }
        }
        return;
    }
    if !block.is_plant() {
        return;
    }
    for d in Dir::ALL {
        let p = pos.offset(d);
        if !block_at(&*host, level, p).is_some_and(Block::is_pipe)
            || flag(&*host, level, p, d.opposite().name())
        {
            continue;
        }
        let mut content = with_level(dim, |l| l.steam.works.content_at(p));
        try_connect(host, level, dim, p, d.opposite(), &mut content);
    }
}

/// The wrench on a pipe's face: connect it, or disconnect it on both sides.
/// Returns the translation key of what happened.
pub fn toggle(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dim: Dim,
    p: Pos,
    face: Dir,
) -> &'static str {
    let q = p.offset(face);
    if flag(&*host, level, p, face.name()) {
        set_property(host, level, p, face.name(), "false");
        if block_at(&*host, level, q).is_some_and(Block::is_pipe) {
            set_property(host, level, q, face.opposite().name(), "false");
        }
        return "fras.wrench.pipe_off";
    }
    let mut content = with_level(dim, |l| l.steam.works.content_at(p));
    if offers(&*host, level, dim, q, face.opposite()).is_none() {
        return "fras.wrench.pipe_nothing";
    }
    if try_connect(host, level, dim, p, face, &mut content) {
        "fras.wrench.pipe_on"
    } else {
        "fras.wrench.pipe_mix"
    }
}

// ---- Hands on valves, vents and vessels -----------------------------------------

/// Right-click a manual valve: one step open; sneaking, one step closed
/// (fluids spec 6.1).
pub fn manual_valve(
    host: &mut dyn ServerHost,
    click: &BlockClick<'_>,
    close: bool,
) -> InteractionResult {
    let dim = click.dimension.key().id();
    let Some(level) = host.level(dim) else {
        return InteractionResult::Pass;
    };
    let pos = Pos::from_array(click.pos);
    let Some(entity) = host.block_entity(level, click.pos) else {
        return InteractionResult::Pass;
    };
    let nbt = entity.data(&*host).unwrap_or_default();
    let opening = blocks::get_i32(&nbt, "opening").unwrap_or(10).clamp(0, 10);
    let next = (valve::step(opening as f64 / 10.0, !close) * 10.0).round() as i32;
    if next == opening {
        return InteractionResult::Success;
    }
    let mut c = Compound::new();
    blocks::put_i32(&mut c, "opening", next);
    server::merge(host, level, pos, c);
    steam::queue(dim, pos, false);
    super::effects::sound(
        host,
        level,
        pos,
        "minecraft:block.iron_trapdoor.open",
        0.4,
        1.6 + 0.04 * next as f32,
    );
    if let Some(player) = host.player(click.player) {
        let _ = player.action_bar(
            host,
            mod_api::Text::translate(
                "fras.valve.opening",
                vec![mod_api::Text::literal(format!("{}", next * 10))],
            ),
        );
    }
    InteractionResult::Success
}

/// Right-click a steam release: open or close it (fluids spec 6.3).
pub fn steam_release(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> InteractionResult {
    let dim = click.dimension.key().id();
    let Some(level) = host.level(dim) else {
        return InteractionResult::Pass;
    };
    let pos = Pos::from_array(click.pos);
    let open = !flag(&*host, level, pos, "open");
    set_property(
        host,
        level,
        pos,
        "open",
        if open { "true" } else { "false" },
    );
    let sound = if open {
        "minecraft:block.fire.extinguish"
    } else {
        "minecraft:block.iron_trapdoor.close"
    };
    super::effects::sound(host, level, pos, sound, 0.6, 1.2);
    InteractionResult::Success
}

/// A bucket on a boiler or tank: a water bucket pours 1000 mB in if there is
/// room; an empty bucket takes 1000 mB from a water tank (fluids spec 8).
pub fn bucket(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let keys = super::keys()?;
    let stack = click.stack?;
    let item = stack.item();
    let filling = item == keys.water_bucket;
    if !filling && item != keys.bucket {
        return None;
    }
    let dim = click.dimension.key().id();
    let level = host.level(dim)?;
    let pos = Pos::from_array(click.pos);
    let done = with_level(dim, |l| {
        if filling {
            l.steam.works.pour(pos, 1000.0)
        } else {
            l.steam.works.draw_water(pos, 1000.0)
        }
    });
    if !done {
        return Some(InteractionResult::Pass);
    }
    let player = host.player(click.player)?;
    if player.has_infinite_materials(&*host) != Some(true) {
        let back = if filling {
            keys.bucket
        } else {
            keys.water_bucket
        };
        let slot = match click.hand {
            mod_api::Hand::Main => player.selected_slot(&*host).unwrap_or(0) as u32,
            mod_api::Hand::Off => 40,
        };
        let _ = player.set_inventory_slot(host, slot, ItemStack::new(back, 1));
    }
    let sound = if filling {
        "minecraft:item.bucket.empty"
    } else {
        "minecraft:item.bucket.fill"
    };
    super::effects::sound(host, level, pos, sound, 1.0, 1.0);
    Some(InteractionResult::Success)
}
