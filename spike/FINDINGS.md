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
- Section 11, milestone 0: the in-game part of this spike (below) closes it.

## Still to do in the game

The code compiles against the pinned SDK, passes clippy with `-D warnings`, and the network tests pass. It has
**not run in the game yet**: that needs the Horizon launcher on your machine.

```sh
cd spike
horizon-launcher native-build . --trust-native-code
```

Then, in a creative world (`/give @s fras_spike:shaft 64`, likewise `crank` and `grinder`):

1. Place a crank, a line of shafts (sideways shafts take the axis you place them along) and a grinder at the end.
   Right-click the crank a few times: the shafts should turn smoothly, faster with each click, and slow down
   when you stop.
2. Put cobblestone in the grinder (right-click it): gravel comes out, and the line turns slower while it works.
3. Lay a line of shafts longer than 32 blocks, crank it, then walk away until its far end leaves the simulation
   distance: the line should stop. Walk back: it turns again at the speed it stopped at.
4. Crank, save and quit, reload: the line comes back at the speed it had, then slows down as nobody cranks.
5. Press F9: the handbook page opens; Escape closes it.
6. With F3, check the frame rate with a few hundred turning shafts in view.
