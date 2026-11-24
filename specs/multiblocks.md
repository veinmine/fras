# Multiblocks: subsystem spec

Spec 4 of the design doc's section 12. Version 1.1 (2026-11-24), scope: **milestone 0.2 (Steam)**.
Builds on `Fire, rust and steam - design.md` v2.1 (section 4.4), `specs/steam-and-heat.md` (spec 2) and
`specs/fluids-and-pipes.md` (spec 3), `specs/rotational-network.md` (the flywheel is a rotational part) and
`spike/FINDINGS.md`. Where this spec is more precise than the design doc, this spec wins; section 12 lists
what changes in the design doc.

**Reviewed with Marcin** (2026-11-24) after being written without an interview, like specs 2 and 3. Section 13
records the decisions; two of them (M1, M2) changed the first draft.

Numbers marked *(tune)* are starting values for balancing, not commitments.

---

## 1. Scope

**In 0.2:** the boiler (copper and iron, with steam domes, heated by fireboxes), the steam engine (steam
cylinder(s) + flywheel, twin engine), the storage tank; formation, validation, splitting, chunk borders, saving,
rendering as one object, wrench diagnostics.

**Designed for, built later:** heat exchanger with radiators and the condenser (0.3), steel boilers up to
~27 blocks (tier 2), coke oven and blast furnace (tier 2). Section 2's rules are written so they apply to
those unchanged.

---

## 2. Common rules

- **Formation is automatic** (design doc 4.4). Place the parts correctly and they visibly join: seams vanish,
  the engine's rod appears. No hammer, no "form" click.
- **Validation runs** when a block of the multiblock (or a block where one could go) is placed or broken,
  when a neighbour that matters changes (a flywheel's plane, a dome's boiler), and when the last missing block
  of a saved multiblock loads (section 9). Never on a timer.
- **Deterministic:** the same blocks always give the same result, whatever order they were placed in, so a
  world reloads into exactly what was saved and a rebuild never surprises the player.
- **Wrench diagnostics:** sneak-right-click any part of a multiblock, formed or not, shows what it is and, if
  it isn't working, why (section 7). For shape problems the wrench also puts particles on the offending blocks
  for a few seconds.
- **Breaking any block** re-validates what's left. A boiler that loses a block **vents** that block's share of
  steam in a puff (spec 2, 5) that scalds the block around it for a second. The rest re-forms if it is still
  valid.
- A block belongs to **at most one** multiblock. Multiblock blocks refuse pistons; structure rotation rotates
  their saved orientation.

---

## 3. Boiler

### 3.1 The cuboid rule

- A **group** is a set of boiler blocks of the **same material**, connected face to face.
- **Different materials and kinds never join, and may touch.** A copper boiler, an iron boiler and an iron
  tank can stand face to face for a compact build; each forms (or fails) on its own, as if the others weren't
  there. The same will hold for every material and multiblock kind added later.
- A group forms one boiler **if and only if** it fills a complete cuboid (any dimensions, any orientation)
  **within its material's size limit**. A dome is not needed to form, but a boiler without one is sealed
  (3.2).

  | Material | Size limit | Rated | Notes |
  | --- | --- | --- | --- |
  | Copper boiler | 4 blocks | 4 bar | Entry, hand-crafted from ingots |
  | Iron boiler | 8 blocks | 6 bar | From hand-hammered plates; rusts from 0.3 |

  So a copper boiler can be 1×1×1, 1×1×2, 1×1×3, 1×1×4 or 1×2×2 in any orientation; an iron one also 1×1×5 to
  1×1×8, 1×2×3, 1×2×4 and 2×2×2.
- **All or nothing:** a group that isn't a cuboid, or is too big, doesn't form at all. The wrench says which
  ("not a cuboid", "too big: 10 blocks, iron boilers hold 8"). The design doc's "extra blocks simply don't
  join" is replaced by this, because choosing which blocks to leave out can't be made obvious (section 13).
- Two boilers of the same material therefore need a gap between them (a firebox in the gap heats both), or a
  boiler of the other material between them.
- Capacity, steam space and heat capacity follow from the block count (spec 2, section 4).

### 3.2 Steam dome

- A dome sits on the **top face** of a boiler block. More domes give more steam throughput (100 mB/s each,
  spec 3, 4.1).
- **A boiler without a dome forms, but is sealed:** its steam space is a steam network of its own with no
  outlet (spec 2, 5), and nothing can vent it, since steam only leaves a boiler through a dome (so an
  emergency valve can't be fitted either). Heated, it builds
  pressure, goes through the warning ladder and **ruptures** (spec 2, 7). This is deliberate: heating a vessel
  with no way out is a preventable mistake, and the game punishes bad planning rather than hiding it. The
  wrench warns on every sealed boiler ("No steam dome: steam can't get out, heating it will burst it").
- Domes are part of the boiler but don't count toward its size limit, and take the boiler's rated pressure.
  Any dome works on either material.
- A dome connects to pipes on its top and four sides; these are the boiler's **only steam outlets**. A dome on
  a block that isn't a formed boiler is inert ("not on a boiler").
- Breaking a dome leaves the boiler formed. Its pipes lose that outlet; with no dome left the boiler is sealed
  and keeps its steam (the network splits by capacity, spec 2, 5), so a hot boiler whose last dome is broken
  keeps building pressure until the dome is replaced or the fire goes out.

### 3.3 Fireboxes: the heating surface

- A firebox heats a boiler when it is **directly below** a block of the boiler's bottom layer, or **beside** a
  block of its bottom layer (spec 2, 3.2). Above or higher up the sides it heats nothing.
- So the boiler's footprint and bottom-layer perimeter decide how many fireboxes fit: a 1×1×4 tall boiler takes
  5 (1 below, 4 beside), a 1×2×4 lying boiler up to 20. Fireboxes are not part of the multiblock: a boiler
  forms and holds pressure with none.

### 3.4 Other faces

- **Feed water** comes in through a pipe or pump on any face but the top (spec 3, 3.1). A pipe on the top face
  of a boiler block without a dome connects to nothing.
- Pressure gauges and sight glasses attach to any outer face (spec 2, 10).

### 3.5 Contents when a boiler changes

- Each boiler block holds a **share** of its boiler's water and the boiler's temperature (spec 2, 14.3).
- When a boiler unforms (a block broken, the shape becomes invalid), each remaining
  block keeps its water share and temperature; an unformed block is inert (no heat, no feed, no steam space).
  Its share of the steam vents.
- When boilers form (or re-form after a change), the new boiler sums its blocks' water and takes their
  heat-capacity-weighted temperature. A new, empty block added to a hot boiler cools it a little.
- A broken boiler block's water is lost.

---

## 4. Steam engine

### 4.1 Flywheel

- A **single block with a large render**: a 3×3 vertical iron wheel around its hub, axle out of both hub faces
  (either or both connect to shafts), like the water wheel (rotational spec 8.2). Its axis is horizontal, set
  by placement.
- Placing needs the 8 positions around the hub in its plane to be air or replaceable (grass, snow layers). The
  rim has no collision in 0.2.
- If a solid block appears in the plane later, the flywheel is **blocked**: it jams (rotational spec 6.4) with
  the reason "blocked by Stone at 12 64 -30". Clear the block and repair it with the wrench.
- A flywheel with no cylinder is still a rotational part with inertia 200 (spec 2, 9.4): a player can put one
  on any line to smooth it.

### 4.2 Steam cylinder

- One block. It forms an engine when it sits **in the flywheel's plane, two blocks from the hub horizontally**
  (directly beside the middle of the wheel's left or right edge), **facing the hub**. Placed next to that
  spot it turns to face the hub by itself.
- **Faces:** front: the rod (to the wheel); back: the **steam inlet** (a steam endpoint, spec 3); top: the
  exhaust (spec 2, 9.3); sides and bottom blank.
- A cylinder that isn't part of an engine draws no steam ("no flywheel: place it two blocks beside a
  flywheel's hub, facing it").

### 4.3 Engine and twin engine

- **Engine** = a flywheel that isn't blocked + one cylinder in either of its two positions.
- **Twin engine** = cylinders in both positions. Each draws and pushes on its own (spec 2, 9.1); they share
  the flywheel and its direction.
- The engine is formed or not; there's nothing else to place. Breaking the flywheel unforms the engine;
  breaking one cylinder of a twin leaves a single-cylinder engine.

---

## 5. Storage tank

- A **group** of iron tank blocks, connected face to face, forms one tank **if and only if** it fills a complete
  cuboid of **at most 27 blocks** *(tune)*. All or nothing, as the boiler. Tank blocks never join boiler
  blocks and may touch them (3.1).
- No other parts. Pipes attach to any face of any block; gauges and sight glasses to any outer face.
- Content and capacity: spec 3, section 8. Each block holds a share; splitting keeps shares; a broken block's
  share is lost (water spills nothing in 0.2; steam vents in a puff).
- A steam tank ruptures like a boiler (spec 2, 7.3), with power from its own block count.

---

## 6. Rendering as one object

### 6.1 Boilers and tanks: connected models

- Each boiler and tank block's **block state** carries `formed`, six `joined_<face>` flags (whether the
  neighbour on that face is in the same multiblock) and the multiblock's long `axis`.
- **Unformed:** a plain riveted block with visible seams on every face.
- **Formed:** a model chosen by the flags: inner faces aren't drawn (seams disappear), outer faces get rivet rows
  along the edges, bands around the long axis, and edges parallel to the long axis are chamfered so the whole
  reads as one rounded shell (design doc 4.4). A tall boiler is a vertical drum, a lying one a locomotive
  barrel.
- The long axis is the cuboid's longest dimension; ties go to vertical, then X.
- These are **static block models**, no block entity drawing: 64 flag combinations × 3 axes per material,
  generated by `tools/gen_content.py`. The spike confirmed terrain-quality model drawing for multiblocks.
- Boiler warning effects (shaking, puffs, glow when dry) are particles and a small per-block offset drawn
  only while warning, from the synced warning level (spec 2, 14.4).

### 6.2 Domes, fireboxes, cylinders

Static block models (firebox has a lit state; cylinder has a formed state that shows the rod's gland).

### 6.3 Flywheel and rod

- The flywheel draws with `spin` + `keep`, like the water wheel and shafts (spike), with raised render bounds
  and view distance.
- The **connecting rod and crosshead** reciprocate, which `spin` can't express. Plan: draw them **every frame**
  from the flywheel's phase, from the flywheel's block entity. Engines are few (tens, not hundreds), so this
  costs little; measure it in the 0.2 performance check. Fallback if per-frame drawing proves expensive or
  unavailable: a spinning crank disc on the wheel and a rod that only rocks (a `spin` about the crosshead pin
  with a small angle range, if supported) or a static rod.
- The rod runs just outside the wheel's face to a crank pin on it, so it never passes through the rim.

---

## 7. Wrench diagnostics (content)

| Situation | Message |
| --- | --- |
| Group not a cuboid | "Not a cuboid: fill the gaps or remove the extra blocks" (particles on the blocks outside the largest cuboid and on the holes) |
| Group too big | "Too big: 10 blocks, iron boilers hold 8" |
| Formed boiler with no dome | "No steam dome: steam can't get out, heating it will burst it" (a warning, shown in red) |
| Dome not on a boiler | "Not on a boiler" |
| Touching another material or kind | "Copper and iron boiler blocks don't join" / "Boilers and tanks don't join" (shown on hover, informational) |
| Firebox heating nothing | "Heating nothing: put it below or beside a boiler's bottom layer" |
| Cylinder alone | "No flywheel: place it two blocks beside a flywheel's hub, facing it" |
| Cylinder facing away | "Facing the wrong way" |
| Flywheel blocked | "Blocked by Stone at 12 64 -30" |
| Part of the multiblock unloaded | "Part of this boiler is outside the loaded area" |
| Formed | Size, material, capacity, domes, fireboxes; then the steam and fluid content of spec 2, 11 and spec 3, 9 |

---

## 8. Recipes (0.2)

| Result | Recipe |
| --- | --- |
| Copper boiler | shaped `CCC` / `C C` / `CCC` (8 copper ingots) |
| Iron boiler | shaped `PPP` / `P P` / `PPP` (8 iron plates) |
| Steam dome | shaped `CCC` / `C C` (5 copper ingots) |
| 2 storage tank | shaped `PIP` / `I I` / `PIP` (4 iron plates, 4 iron ingots) |
| Steam cylinder | shaped `PPP` / `K S` / `PPP` (6 iron plates, a piston, an iron shaft) |
| Flywheel | shaped `III` / `ISI` / `III` (8 iron ingots, an iron shaft) |

All *(tune)*. Copper boiler and dome are entry recipes from vanilla materials (design doc 6.4); everything
else needs plates from the hand hammer, so the first iron boiler or engine costs a session at the crafting
grid. The trip hammer (0.4) is what makes that bearable. Each unlocks on picking up an ingredient.

---

## 9. On Horizon

- **Boiler, dome, tank and cylinder blocks don't tick** (spec 2, 14.1). Their block entities hold the saved
  formation (the multiblock's min corner and size, or the engine's flywheel position) and their content
  shares.
- A **level attachment** holds the multiblock registry (position → multiblock id). Validation runs in the
  level tick after block changes, before the steam step (spec 2, 14.1).
- **Chunk borders:** a multiblock with a block in an unloaded chunk is **partly loaded**. It keeps its saved
  formation and is not re-validated; its plant pauses (spec 2, 14.2). When the last block loads, validation
  runs once; if the blocks match what was saved (the normal case) nothing changes and nothing vents.
- **The first tick after a load runs before the registry knows the multiblock** (spike lesson 3): blocks keep
  their saved shares and formation until validation has run.
- The cuboid check is cheap: a group is at most 8 or 27 blocks in tier 1, ~27 in tier 2. The group search
  stops as soon as it exceeds the limit (it is then "too big" and needn't be walked further).

---

## 10. Code layout and tests

- **`sim::multi`**, game-free: `group` (same-material flood fill with a cap), `cuboid` (is it a full cuboid,
  its size and long axis, which blocks are outside or missing for the particles), `boiler` (domes, firebox
  positions, shares on split and merge), `engine` (flywheel plane, cylinder positions and facing), `tank`.
- **Unit tests** at least for: every valid copper and iron shape in every orientation forms; L-shapes, holes,
  and 9 iron blocks don't; mixed materials and boiler next to tank stay separate and each forms on its own; a
  boiler with no dome forms sealed, removing the last dome seals it and keeps its steam; firebox positions (below and
  beside the bottom layer only); water shares and temperature on split and merge; formation independent of
  placement order; engine with one and two cylinders, wrong facing, blocked plane; tank up to 27; partly
  loaded multiblocks keep their formation and don't vent on load; long-axis choice and its ties.

---

## 11. 0.2 exit criteria

In `specs/steam-and-heat.md`, section 16 (items 1, 2, 3, 5, 7, 10, 11 and 13 exercise this spec).

---

## 12. Changes to the design doc

- 4.4: a boiler group is all or nothing (a non-cuboid or oversized group doesn't form) instead of "extra
  blocks don't join"; different materials and kinds never join and may touch; a boiler forms without a dome
  but is then sealed and ruptures if heated; fireboxes heat from below or beside the bottom layer; tier-1
  boilers come in copper (4 blocks) and iron (8 blocks).
- 4.4: the steam cylinder sits two blocks beside the flywheel's hub in its plane, facing it; a twin engine has
  one on each side. The flywheel is one block with a large render, like the water wheel, and works alone as a
  flywheel on any line.
- 4.4: the storage tank is a full cuboid of up to 27 iron tank blocks.

---

## 13. Decisions

Reviewed with Marcin on 2026-11-24. M1 and M2 are his answers; the rest were taken as proposed.

1. **M1. All or nothing,** with different materials side by side. A group that isn't a full cuboid within its
   limit doesn't form; no carving out of a largest cuboid. Boilers and tanks of different materials or kinds
   may touch for compact builds and never join (3.1). Same-material boilers need a gap.
2. **M2. A boiler without a dome forms sealed and ruptures if heated** (3.2). Heating an unconnected boiler is
   a clear, preventable danger; the wrench warns, the game doesn't save the player from it.
3. **M3. Fireboxes only at the bottom layer** (below or beside), so tall boilers aren't strictly better.
4. **M4. Cylinders only left and right of the wheel,** horizontal. Beam-engine positions can come later.
5. **M5. The flywheel rim has no collision** in 0.2, like the water wheel's paddles.
6. **M6. The rod is drawn every frame** (6.3), measured in the 0.2 performance check; the fallback is a crank
   disc with a rocking or static rod.
7. **M7. Tanks:** up to 3×3×3, solid, iron only in tier 1.
