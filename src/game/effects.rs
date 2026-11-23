//! Sounds and particles for what the network does (rotational network spec
//! 11.5). Vanilla sounds stand in until FRAS has its own.

use mod_api::{LevelRef, ParticleOptions, ServerHost, SoundSource};

use super::server::{Dim, with_level};
use crate::sim::geom::Pos;
use crate::sim::rot::Event;

fn center(pos: Pos) -> [f64; 3] {
    [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5]
}

pub fn sound(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    id: &str,
    volume: f32,
    pitch: f32,
) {
    level.play_sound(host, id, center(pos), SoundSource::Blocks, volume, pitch);
}

pub fn particles(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    kind: fn(&super::Keys) -> mod_api::ParticleKey,
    count: i32,
) {
    let Some(keys) = super::keys() else {
        return;
    };
    let options = ParticleOptions::simple(kind(keys));
    level.send_particles(
        host,
        &options,
        None,
        center(pos),
        [0.3, 0.3, 0.3],
        0.02,
        count,
    );
}

/// What the network reported this tick.
pub fn event(host: &mut dyn ServerHost, level: LevelRef, event: &Event) {
    match *event {
        // The clunk of a jam, and smoke.
        Event::Jammed(pos, _) => {
            sound(host, level, pos, "minecraft:block.anvil.land", 0.5, 1.3);
            particles(host, level, pos, |k| k.smoke, 12);
        }
        // A belt squeals when it starts slipping.
        Event::Slipped(a, _) => {
            sound(host, level, a, "minecraft:block.grindstone.use", 0.6, 1.6);
        }
        Event::Gripped(a, _) => {
            sound(
                host,
                level,
                a,
                "minecraft:block.wooden_button.click_on",
                0.5,
                0.8,
            );
        }
    }
}

/// Every half second: rattle for overspeed, creak for overload, louder and
/// quicker as the warning timer fills.
pub fn warnings(host: &mut dyn ServerHost, level: LevelRef, dim: Dim) {
    let warned = with_level(dim, |l| l.mech.warned());
    for (pos, overspeed, level_) in warned {
        let volume = 0.2 + 0.15 * level_ as f32;
        if overspeed {
            sound(
                host,
                level,
                pos,
                "minecraft:block.chain.hit",
                volume,
                0.8 + 0.3 * level_ as f32,
            );
            if level_ >= 2 {
                particles(host, level, pos, |k| k.crit, 3 * level_ as i32);
            }
        } else {
            sound(
                host,
                level,
                pos,
                "minecraft:block.wood.hit",
                volume,
                0.5 + 0.1 * level_ as f32,
            );
            if level_ >= 2 {
                particles(host, level, pos, |k| k.cloud, level_ as i32);
            }
        }
    }
}
