# FRAS milestone 0: technical spike findings

Date: 2026-11-23. Horizon build `0b14f8a` (game 26.3, 2026-11-18, the same pin as Kid Mode), cross-checked
against the 2026-11-22 Nightly (`f1bb66f`): every hook used here exists in both.

**The spike code in this folder is throwaway.** It was written to answer questions, not to be the start of the
mod. The real code is written after each subsystem's spec (design doc, section 12).

## Verdict

Horizon's mod API covers everything tier 1 needs, and two things turned out better than hoped:

1. **Rotating parts are nearly free to draw.** A block entity renderer can mark geometry as `spin` (axis,
   pivot, degrees per tick, phase) and `keep` it: the game turns it every frame on the client's animation clock
   without calling the mod again until the block entity changes. A shaft costs one call when its speed changes,
   not one per frame. The doc even gives the conversion (`rpm * 0.3` degrees per tick) and the phase formula for
   keeping a part where it was when the speed changes, and for meshing gears.
2. **The chunk-unload rule maps cleanly onto the API.** Block entities only tick where blocks are simulated, the
   level can be asked whether a position is loaded and simulated, and block entities have `on_load` and
   `on_chunk_unloaded` hooks. The "whole network pauses" rule needs no mixin.

**Tested in the game** (offscreen, driven through the Horizon MCP server on archmage, see "In-game results"):
shafts draw and turn smoothly, cranking spins a line up and load sags it, the grinder's menu and grinding work,
the network pauses at the simulation edge and resumes at the same speed, speed survives save and reload, and the
mod screen opens. 440 turning shafts cost about a quarter of a millisecond a frame.

One item is blocked upstream: **the official Recipes viewer** (see below). One is unproven: **the copper golem
crank**.

No mixins were needed. Nothing in the spike asked for one.

## What the spike checks

| Question | Answer | Evidence |
| --- | --- | --- |
| Ticking block entity | **Yes.** `block_entity_tick` with the entity's saved NBT, item slots and menu data slots; `BlockEntityTickReply::Unchanged` skips the write-back when nothing changed. | `src/lib.rs` `block_entity_tick`; compiles against the SDK. |
| Smooth shaft rendering at scale | **Yes, better than expected.** `block_entity_submit` + `spin` + `keep` (see verdict). View distance (64 blocks default) and render bounds are hookable, for long shafts and big flywheels. | `src/lib.rs` `block_entity_submit`. |
| Network that survives chunk unload/reload | **Yes.** Speed is saved in each part's NBT; a network with a part outside the simulated area pauses with its speed; it comes back as one network at the same speed. Rebuilds happen only when something changed. | `src/network.rs`, 9 unit tests (pause/resume across a chunk border, reload from saved speed, split on break, no rebuild when idle). |
| Block GUIs (firebox, grinder) | **Yes.** `block_container` with a vanilla menu type, a screen layout the mod controls (slot positions, hidden slots, own background), `screen_draw` for gauges and arrows, 16-bit data slots, extra screen data (32 KiB), menu buttons, quick-move. Mod menu types exist too. | `src/lib.rs` grinder `screen`/`screen_draw`. |
| Fluids | **Yes.** Mod fluids (hot water) as content declarations, with or without a world form and bucket; block tanks with `ModFluidTanks` and the `FLUIDS` capability, moved in transactions; a renderer helper draws a fluid filling a box (tanks, sight glass). Steam can be a mod fluid with no world form. | Modding book: content mods, block fluids. |
| Chunk tickets | **Yes**, NeoForge-style ticket controllers with validation on load. Not needed in tier 1 (no chunk loader), available if a server setting wants one later. | `TicketController`, `validate_tickets`. |
| Recipes viewer integration | **Blocked upstream.** The official Recipes mod is a legacy `.hnmod` that launchers from 0.2.70 can't load; the registry archived it on 2026-11-18 and its port is tracked in horizon-mods issue #1214. No API for another mod to add a category to it was found. The engine's own item panel can show recipe pages (`panel_recipes`), so FRAS could show its own pages; custom recipe types (`fras:grinding`) are supported. | horizon-mods `native/recipes/README.md`; book: panel style. |
| Custom book screen (handbook) | **Yes.** A mod opens a screen of its own and draws it (fills, sprites, text, buttons, text fields) and takes its input. There is no layout engine: wrapping, pages and images are FRAS's own work, so the handbook is a real subsystem. | `src/lib.rs` F9 screen; book example `widget_screen.rs`. |
| Entity hooks for the copper golem crank | **Plausible, unproven.** Horizon has the copper golem (brain-driven, weathering into a statue, waxing). Mods can add brain behaviours and goals to a species, hear right-clicks on entities and leads, and draw model parts. Holding it at the crank is doable; animating its arm on the vanilla model is the uncertain part. | `EntityPart::register_behaviors`, `entity_interact`, render hooks. |

Also confirmed along the way, for later subsystems: comparator output and redstone hooks, explosion hooks, piston
push reactions (shafts should refuse pistons), structure rotation of saved block entity data, level and chunk
attachments saved with the world, looping sounds from a client-side block entity tick, terrain-quality model
drawing for multiblocks, settings pages.

## Risks and what they mean for the plan

1. **Every ticking block entity is a hook call with a data snapshot.** Fine for machines, wasteful for shafts:
   a factory has hundreds. Plan: **shafts, bearings and gears don't tick.** Network membership comes from
   `block_entity_on_load`, `on_chunk_unloaded` and block changes; only sources and machines tick. (The spike
   lets shafts tick to keep it short.) Add to the rotational network spec.
2. **"Unloaded" really means "not simulated".** Block entities stop ticking at the simulation distance, which
   is usually smaller than the view distance. A network that crosses it pauses, so a player can see frozen
   shafts at the edge of what they see. The spike shows paused networks as stopped. The rotational spec should
   decide whether a paused network keeps turning visually.
3. **The Recipes viewer can't be planned around yet.** Keep it out of 0.1–0.4. Add a rule for it to the
   Horizon watch (as Kid Mode does) and revisit when #1214 lands. Fallback: FRAS's own recipe pages in the
   engine's item panel, or wrench/tooltip help only.
4. **The handbook is a subsystem, not a screen.** Everything is hand-drawn. It stays a 1.0 goal; give it its own
   spec (text layout, pages, images, cross-links).
5. **Copper golem stays optional.** Prototype it in 0.4 for a day; drop it if the arm animation needs a mixin.
6. **API churn.** Nearly every Nightly changes the mod API. Pin one build per release (as Kid Mode does), copy
   Kid Mode's Horizon watch, and keep the game layer thin so a repin touches little.
7. **Client state.** Block entity Part state is server-only; clients get what `block_entity_update_tag` sends.
   The spike sends speed and axis; real parts will send a small fixed set. Speed updates only when it changes
   noticeably, as the design (section 9) already says.

No milestone has to move. Suggested changes to the design doc:

- Section 9: shafts don't tick; the pause boundary is the simulation distance.
- Section 7.2 / 8: Recipes viewer depends on an upstream port; not before it lands.
- Section 11, milestone 0: done; this report closes it.

## In-game results

Horizon build `0b14f8a`, its own launcher instance ("FRAS spike"), a creative world, offscreen client driven
through the Horizon MCP server. Each `horizon-launcher native-build` took about 1 second (the prebuilt game is
reused; only the mod compiles).

| Check | Result |
| --- | --- |
| Shafts render and turn | **Pass.** Shaft bars join end to end between crank and grinder and visibly rotate (screenshots three ticks apart). |
| Crank spins up the line, it slows when you stop | **Pass.** 8 shafts settle near 17.8 RPM, 40 shafts near 7.2 RPM (more friction); a 40-shaft line stops in about 3 s. |
| Grinder: menu, grinding, load | **Pass.** Right-click opens a 3x3 menu with input and output slots; cobblestone becomes gravel (2 per 15 s at speed); the line sags from 17.4 to about 10.6 RPM while it grinds. |
| Pause at the simulation edge, resume | **Pass.** With the far end outside simulation distance, the rest held its speed (no slow-down) and showed as stopped; on return it carried on from that speed. |
| Save, quit, reload keeps the speed | **Pass** (after a fix): it comes back at the saved speed, then slows as nobody cranks. |
| Mod screen (F9 handbook page) | **Pass.** Opens, Escape closes. |
| Frame rate, 440 shafts, same camera | No shafts 2631 FPS (CPU 0.35 ms, GPU 0.135 ms); 440 idle 1612 FPS (0.58 / 0.193 ms); 440 with a line turning 1436 FPS (0.66 / 0.218 ms). About +0.25 ms CPU and +0.06 ms GPU a frame for 440 shafts, measured with an extra diagnostic quad per shaft. Only one or two lines were turning: a cranked line stops faster than all eleven can be cranked. |
| Server cost | 440 ticking parts added about 0.11 ms of CPU per frame in single player. |
| Mod errors in the log | None. |

### Bugs the game found (all fixed in the spike)

These are lessons for the real code, which is why they're listed:

1. **Hooks name the block entity type, not the block.** One shared type for every part meant a shaft and a grinder
   looked the same to `block_entity_tick` (and to the renderer's check): shafts never drew and the grinder never
   ground. Give each block its own type, or read the block from the level.
2. **Sprites in block entity drawing are named without a namespace.** `"block/stripped_oak_log"` draws;
   `"minecraft:block/stripped_oak_log"` makes `cube` return false and draw nothing, silently. Always check the
   returned bool.
3. **The first tick after a load runs before the network exists.** Writing the network's speed then overwrote the
   saved speed with zero. A part with no network yet keeps what it saved.
4. **The vanilla `minecraft:chain` is `minecraft:iron_chain` in 26.3.** An unknown template stops the game at
   startup.
5. **A container menu didn't open by itself** when the block was used (vanilla's hopper menu named in
   `ModContainer`); the block's part opens it with `player.open_menu`. Worth a closer look in the GUI spec.

Testing notes: an MCP-started client doesn't pass on the mod's stderr (start it with `horizon-launcher play` to
read `[fras_spike]` lines), and the MCP interact call didn't trigger block use; a real look-at plus mouse click did.
Screenshots are on archmage in `~/git/fras-spike/spike/screenshots/` (not committed).

## Running it yourself

```sh
cd spike
horizon-launcher native-build . --instance fras-spike --trust-native-code
```

In a creative world (`/give @s fras_spike:shaft 64`, likewise `crank` and `grinder`): place a crank, a line of
shafts against it (placed against a block's side they lie along that axis) and a grinder at the end, right-click
the crank a few times, and put cobblestone in the grinder. F9 opens the handbook page.
