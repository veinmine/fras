# Machines: subsystem spec

Part of spec 6 (items and machines) in the design doc's section 12. Version 1.1 (2026-11-26), scope:
**milestone 0.4 (Factory)**. Builds on `Fire, rust and steam - design.md` v2.1 (sections 6.1, 6.2 and 6.4),
`specs/grinder.md` (the reference machine: this spec reuses its recipe format, work model, inventory rules and
GUI, and only says where a machine differs), `specs/rotational-network.md` (sections 6 and 9, speed ranges,
jams and machine rules), `specs/fluids-and-pipes.md` and `specs/cooling.md` (the washer's water) and what 0.1 to
0.3 shipped (`docs/development.md`). Written together with `specs/logistics.md` (belts, and processing on the
belt, section 7 there) and `specs/progression.md` (what the machines make and what needs it). Where this spec is
more precise than the design doc, this spec wins; section 14 lists what changes in the design doc.

**Reviewed by Marcin on 2026-11-26** (decisions in section 15). **Revised as 1.1 the same day** after in-game testing of 1.0: machines are line machines with no inventory slots, two blocks tall with a built-in belt, and the trommel is a three-block drum built like a boiler. Section 16 lists the 1.1 decisions. Written in one go without an interview. Section 15 lists the open questions with the
assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 13 collects them, with the
recipes. The **0.4 exit criteria** for the whole milestone are in `specs/progression.md`, section 9.

---

## 1. Scope

**In 0.4:** the **washer**, **trip hammer**, **lathe**, **sawmill**, **inline grinder** and **trommel**: their recipes,
speed ranges, loads, blocks, built-in belts, info screens, looks, sounds and hazards; the **dust** items; the 0.1 grinder's
belt hookup; a reference ore line that sizes them against each other.

**In the companion specs:** belts, elevators and the diverter (`specs/logistics.md`); intermediate parts and the
recipe tree (`specs/progression.md`); the copper golem (`specs/copper-golem.md`).

**Designed for, built later:** water cooling for machines other than the grinder (cooling spec 9.3), the stamp mill
and electric pulveriser (tier 2+), recipe pages in the item panel (question M9), Recipes viewer pages (upstream).

---

## 2. Rules every machine follows

The grinder spec is the template for recipes and work. Everything else (the block, feeding, output) is new in 1.1: the
0.4 machines are **line machines**. They have no inventory slots and no GUI to put items in; items reach them on belts,
from hoppers or by hand, and leave on the other side.

### 2.1 Recipes

- Each machine has its own recipe type, with the **grinder's JSON format** (grinder spec 2.2): `ingredient` (item or
  tag), `results` (1 to 4, each with `count` and per-item `chance`), `revolutions`, `load`. Types: `fras:washing`,
  `fras:hammering`, `fras:turning` (lathe), `fras:sawing`, and `fras:grinding` for the inline grinder (the 0.1 grinder's
  recipes, shared). The trommel sorts by tags, not recipes (section 8).
- **One item in per recipe.** A line machine works on one item at a time, so `count` is always 1; a recipe file with a
  larger count is ignored with a log line. (1.0's 4 sand → sandstone on the trip hammer is gone.)
- Extra fields: washing has `water` (mB per item, default 50). Hammering counts **strikes** on the info screen (2 per
  revolution) but stores revolutions like the rest.
- Recipes are FRAS's own data files; datapacks can add or override them.

### 2.2 Work and load

- Progress in revolutions: `|rpm| / 1200` per tick inside the range, nothing below the minimum (with the load still
  drawn), capped at the maximum while overspeeding (grinder spec 3, rotational spec 9.2).
- Load only while it **has work** (an item with a recipe at the work point), otherwise friction only.
- The working range comes from a function of the machine's state, so cooling can widen it later.

### 2.3 The block: two blocks tall

The washer, trip hammer, lathe, sawmill and inline grinder are **two blocks tall**, placed as one item like a door:

- **The top block is the machine.** A **built-in belt** runs straight through it from the **input face** (back) to the
  **output face** (front), at the height of a conveyor belt laid on top of a floor block: the machine sits at the level of
  belts running one block above the floor (Marcin's intended layout: lines at eye level). The work happens over the middle
  of that belt.
- **The bottom block is the drive.** It takes rotation through **one socket**: any of its four sides or its bottom, never
  its top. On placement the socket faces an adjacent shaft-like part's end if there is one (sides first, then below);
  otherwise it faces the bottom. If the socket face has no shaft and one is later placed against another face, the socket
  moves there by itself. The wrench cycles it through the faces that have a shaft against them (all five if none has).
  One socket means a machine never joins two networks by accident.
- **Placing:** the input faces the player, the output away from them (items run away from you). Placing needs the block
  above free; if it isn't, nothing is placed and the action bar says "Needs 2 free blocks" (checked before placement, so
  the block never blinks in and out). Breaking either half breaks both, drops the machine and the items on its belt.

### 2.4 The built-in belt

- The machine's belt is **driven by the machine's own power**, not by the belt lines around it: speed `|rpm| / 32`
  blocks/s, as a conveyor belt (logistics spec 3.4). It holds **2 items** (spacing 0.5), like one belt block.
- **Items come in** at the input face: from a belt line's leading end facing it (logistics spec 5.1), a hopper or dropper
  facing it or above the machine, an elevator head's outlet, or by hand (2.6). The machine refuses an item when its belt has
  no room at the input end, so the source waits.
- **At the work point** (the middle of the belt) an item the machine has a recipe for **stops** and the machine works on
  it. When the recipe completes, the item is **replaced** by the main result and any extra results are queued right behind
  it on the machine's belt; the machine takes the next item once they're all out. A machine that can't work (too slow,
  jammed, out of water) holds its item, and the line behind backs up.
- **Items with no recipe pass straight through** untouched, so a mixed stream works.
- **Items go out** at the output face with a belt end's rules (logistics spec 5.1): into an inventory, onto a belt line
  (its start or its side), into an elevator boot or a trommel's end, falling out into air, otherwise waiting at the output
  and backing the machine up.
- No extraction: a hopper can't pull from a machine. Comparator: 0 to 15 by how full the built-in belt is (0, 1 or 2 items
  and whether the output is blocked: 0, 7, 11, 15).

### 2.5 Info screen

Right-click with an empty hand, or with an item the machine has no recipe for, opens an **info screen** with no slots: the
item being worked (drawn), a progress bar, the **speed bar** with the working band in green and the red line, the **status
line** ("Working", "Too slow: needs 128 RPM", "No water", "Output blocked", "Jammed: overspeed", …), and the washer's water
gauge. The wrench's sneak-click overlay shows the same lines (information only, as everywhere in FRAS).

### 2.6 By hand

Right-click with an item the machine has a recipe for puts **one** on its built-in belt at the input end, if there's room
(sneak: as many as fit, two). The result comes out of the output face like any other: into whatever is there, or dropping
into the world if there's air. This is how the first lathe and trip hammer are used before the first belt line: stand at
the input, click ingots in, collect rods from a chest at the output.

### 2.7 Overspeed, jam and repair

Overspeed warns, then jams (rotational spec 6.2, 6.4); a jammed machine stops its built-in belt and holds its items. Each
machine has a cheap repair (section 13), with the wrench as before. **Machines don't rust in 0.4** (question M10).

---

## 3. Line layout

A machine takes one belt line in and gives one out. The usual layout: belts at y+1 (one block above the floor) feeding the
machine's input, the machine's drive block on the floor with a line shaft along the floor or under it, the next belt line
starting at the output. Belt lines on the floor ramp up to the machine level with slopes (logistics spec 3.1).

Nothing works on items riding a conveyor belt any more: 1.0's **belt mode** is gone (logistics spec 7 is withdrawn). A
machine is the station; the belts carry between stations.

---

## 4. Washer

### 4.1 Role

Crushed ore → **dust** plus a small byproduct; rusting iron nuggets into **rust flakes** (design doc 5, 6.1).
Water-fed. Everything it makes leaves by the output; the trommel does the sorting (1.1).

### 4.2 Recipes (`fras:washing`)

| Input | Results | Revolutions | Water | Load |
| --- | --- | --- | --- | --- |
| Crushed iron | 1 iron dust, 1 in 3 a 2nd | 2.5 | 50 mB | 4 Nm |
| Crushed copper | 1 copper dust, 1 in 3 a 2nd; 10% 1 gold nugget | 2.5 | 50 mB | 4 Nm |
| Crushed gold | 1 gold dust, 1 in 3 a 2nd; 10% 1 copper nugget | 2.5 | 50 mB | 4 Nm |
| Iron nugget | 1 rust flakes | 64 | 200 mB | 2 Nm |

All values *(tune)*. Rationale:

- **Grinding and washing double the yield** (M1, decided): 4/3 dust per crushed ore (chance 0.3333 in the recipe file), so
  the grinder's 1.5× becomes **2×**, plus the byproduct for copper and gold. Iron has no byproduct: 1.0's 10% redstone
  is gone (1.1, Marcin: a powder in the fines stream clogs the furnaces).
  Tier 2's stamp mill pushes toward 3× (design doc 6.1). Smelting crushed ore directly keeps working.
- **Dust smelts in half the time** (100 ticks in a furnace, 50 in a blast furnace) to one ingot, with the raw ore's
  XP. A furnace bank keeps up with a washer with fewer furnaces, which matters to an unattended line.
- **Rust flakes without scraping** (design doc 5, paint supply): slow on purpose, one nugget every 64 revolutions
  (2 minutes at 32 RPM), so a paint line is a long-running sideline, not a flood. Flakes → grinder → pigment →
  paint is unchanged.
- **Dust items:** iron dust, copper dust, gold dust, tag `fras:dusts`. No storage blocks.

### 4.3 Water

- **A tank of 1,000 mB** *(tune)* filled through the **left and right sides of the top block** (the faces the built-in
  belt doesn't use), from a pipe (fluids spec), or by hand with a water bucket on the top block. **Water or hot water** both work: a closed-loop plant can send its
  surplus hot water here instead of a drain (cooling spec 8). The tank holds one content at a time, like a pipe network.
- Water is used per item at completion (the `water` field) and is lost (it drains away under the machine, with a few
  drip particles; no waste water fluid).
- "Has work" also needs enough water for the recipe. Status "No water"; the item waits at the work point.
- Sight of the level: the info screen shows a gauge; the overlay shows mB.

### 4.4 Machine numbers

Range **8 to 64 RPM** *(tune)*, inertia 4, friction as the grinder, repair **1 iron nugget and 1 planks** *(tune)*.
At 64 RPM a crushed ore takes 2.3 s: a washer keeps up with a grinder on the same shaft (2.5 versus 4 × 2/3 = 2.7
revolutions per crushed ore, section 10).

### 4.5 Look and sound

Top: a wooden tub with a paddle drum turning in it over the built-in belt, iron hoops, a spray bar. Bottom: a timber
frame around the drive. The drum spins with `spin` + `keep`. Splashing loop while working, pitch by RPM; drip particles below; the item's break
particles in the tub.

---

## 5. Trip hammer

### 5.1 Role

Ingots → **plates**; heading rods into **rivets** (design doc 6.2). Loud, period and fun to watch.

### 5.2 Recipes (`fras:hammering`)

| Input | Results | Strikes (revolutions) | Load |
| --- | --- | --- | --- |
| Iron ingot | 1 iron plate | 4 (2) | 8 Nm |
| Copper ingot | 1 copper plate | 4 (2) | 8 Nm |
| Iron rod | 8 rivets | 6 (3) | 8 Nm |

All values *(tune)*. Plates at the **same yield** as the hand hammer (design doc 6.4): the trip hammer is about speed
and automation. Rivets need a rod from the lathe, which makes lathe → hammer the first production line
(`specs/progression.md`).

### 5.3 Strikes

- A **cam** on the shaft lifts the hammer **twice per revolution**; each drop is one strike: a sound, sparks on metal,
  and a step of progress.
- Load: an average of 8 Nm while working. The solver sees a steady load, so the network doesn't jitter, but the
  tachometer readout shows the speed dipping a little at each strike *(tune: 1 Nm of ripple, drawn, not simulated)*.

### 5.4 Machine numbers and hazard

- Range **16 to 64 RPM** *(tune)*: a hammer is slow. At 64 RPM a plate takes 1.9 s. Above 64 the head bounces and the
  hammer jams. Inertia 6 (the head), repair **2 iron nuggets**.
- **Hazard:** a player or mob touching the top block's belt space under the head takes **2 damage per strike** *(tune)*
  while the hammer is working. It warns: the head is visible and every strike is loud. Kid Mode's invulnerability covers it.

### 5.5 Look and sound

Top: a timber frame with a cam shaft and a heavy iron head on a wooden helve, striking an anvil plate set into the built-in
belt. Bottom: the drive, with the cam's gearing. The head's motion is drawn each frame while working
(it isn't a plain spin), kept still otherwise. A heavy **thump** per strike, sparks on metal; the rate tells the speed
from across the room (design doc 7.1).

---

## 6. Lathe

### 6.1 Role

Ingots and plates → **rods, gears and pipes**: the parts later machines are built from (design doc 6.2, 6.4).

### 6.2 Recipes (`fras:turning`)

| Input | Results | Revolutions | Load |
| --- | --- | --- | --- |
| Iron ingot | 2 iron rods | 8 | 4 Nm |
| Iron plate | 1 iron gear | 12 | 4 Nm |
| Copper plate | 3 copper pipes | 8 | 3 Nm |

All values *(tune)*. A machine has one recipe per input (the first by id would win, grinder spec 2.2), which keeps the
list short: an iron ingot is always turned into rods, a plate into a gear (iron) or bored into pipe (copper). Copper pipe from
the lathe gives 3 per ingot against the crafting grid's 2. Iron pipe stays a crafting recipe from plates, which the trip
hammer now makes quickly (`specs/progression.md` 4).

### 6.3 Machine numbers and hazard

- Range **128 to 224 RPM** *(tune)*: the lathe is the fast machine. No tier-1 source reaches 128 RPM under load directly
  (the first engine's top is 128 at 4 bar), so a lathe always runs off a step-up: a 2:1 pair of **iron pulleys** before the
  gearbox exists (`specs/progression.md` 4.4), a gearbox after. Wooden pulleys jam above 96. At 160 RPM a rod takes 3 s, a gear
  4.5 s.
- Inertia 2 (light), repair **1 iron nugget and 1 flint** (a new cutting tool).
- No hazard beyond the rotational spec's overspeed.

### 6.4 Look and sound

Top: a cast-iron bed over the built-in belt, headstock with a stepped pulley, tailstock, tool rest; the item is lifted
between centres at the work point. Bottom: legs and the drive. The workpiece (the input
item, drawn small between centres) spins with `spin` + `keep` while working; curls of swarf as particles. A high whine
whose pitch follows the RPM.

---

## 7. Sawmill

### 7.1 Role

Logs → more planks than by hand (design doc 6.2).

### 7.2 Recipes (`fras:sawing`)

| Input | Results | Revolutions | Load |
| --- | --- | --- | --- |
| Any log or wood of a kind (oak log, stripped oak log, oak wood, …) | 6 planks of that kind | 4 | 6 Nm |
| Crimson / warped stem, hyphae | 6 planks of that kind | 4 | 6 Nm |
| Bamboo block | 3 bamboo planks | 4 | 6 Nm |
| Any planks (`#minecraft:planks`) | 3 sticks | 2 | 3 Nm |

All values *(tune)*. 6 planks per log against the crafting grid's 4 (1.5×, the grinder's theme again); 3 sticks per
plank against the grid's 2. One recipe file per wood kind, generated by `tools/gen_content.py`. The sawmill also takes
any **modded logs** through a datapack recipe; there is no automatic mapping (Horizon can't tell which planks a modded
log makes).

### 7.3 Machine numbers and hazard

- Range **32 to 128 RPM** *(tune)*: a saw wants speed. At 96 RPM a log takes 2.5 s. Inertia 4 (the blade), repair
  **2 iron nuggets**.
- **Hazard:** the blade stands proud of the table. A player or mob touching the top block while it turns above its
  minimum takes **1 damage every half second** *(tune)*, like a hot pipe (0.2 measured that 1 per second was cancelled by
  regeneration). Warning: the whirring and the visible spinning blade. Kid Mode covers it.

### 7.4 Look and sound

Top: a timber table with a circular blade standing out of a slot across the built-in belt. Bottom: the drive. The blade spins
with `spin` + `keep`. A buzz that drops in pitch while cutting; sawdust particles.

---

## 8. Trommel

### 8.1 Role

A rotating sieve that **sorts items by size** into three outlets (design doc 6.2): steam-era sorting. In an ore line it
splits dust from crushed ore that slipped through and from anything that wasn't ground.

### 8.2 Block: three in a row

- **One block type, trommel**, crafted three at a time. **Three trommel blocks in a straight horizontal row form a
  trommel**, automatically, like a boiler or an elevator column (multiblock spec): the row's direction is the drum's axis.
  A row of more than three, or blocks that also touch another row, doesn't form; the sneak-wrench overlay says why
  ("Trommel needs 3 blocks in a row", "Row too long: 4 blocks"). Unformed blocks show a bare frame. Placing can't fail, so
  nothing blinks.
- **Not directional.** Items come in at **either end face** (and the top of either end block), from a belt end, a
  hopper, an elevator head or by hand. Every item is accepted: anything without a size class is oversize.
- **Power:** one socket on any **long side face** of any of the three blocks (six faces), its axis across the drum, driving
  the rollers the drum rests on. On forming, and whenever its socket face loses its shaft, it takes the first long side
  face with a shaft end against it. One socket at a time; the wrench cycles through the faces that have a shaft.
- **Outlets, all underneath, following the item** (Marcin: outputs on the bottom give belt slopes a reason and add some
  verticality): an item moves away from the end it came in. Under the **first** block it crosses, **fines** fall through;
  under the **second**, **mediums**; out of the bottom of the **third**, **oversize**. Fed from the west end, fines drop
  under the west block; fed from the east end, under the east block. Feeding both ends at once mixes the two end outlets,
  so a line feeds one end.
- Each outlet pushes into the inventory, belt or elevator boot below at 1 item every 8 ticks, or drops the item into air.
  A blocked outlet stops only items of its class; the others keep moving through.

### 8.3 Sorting

- **Size classes come from item tags**, so datapacks can extend them: `fras:trommel/fine` and `fras:trommel/medium`.
  Anything in neither is **oversize**.
- Defaults *(tune)*:
  - **Fine:** sand, red sand, all dusts (`fras:dusts`). The ore line's fines are only dust, so they can feed furnaces
    directly. A rock-crushing line (sand) and an ore line are separate lines with their own trommels.
  - **Medium:** crushed ores (`fras:crushed_ores`), raw ores, iron, gold and copper nuggets, gravel, flint, redstone dust,
    bone meal, clay ball, coal, charcoal, rust flakes, red oxide pigment.
  - **Oversize:** everything else (cobblestone, cobbled deepslate, dirt, blocks, tools).
- An item tumbles **1 revolution** *(tune)* from its entry end to the far end, falling out at its outlet on the way. The
  drum holds **up to 8 items** at once (visible tumbling); a full drum refuses input.

### 8.4 Machine numbers

Range **8 to 96 RPM** *(tune)*, load **2 Nm plus 0.25 Nm per item in the drum** (up to 4 Nm), inertia 6, repair **2 iron
nuggets**. At 64 RPM it sorts about 8 items/s. Info screen as section 2.5: speed, status, how many items tumble.

### 8.5 Look and sound

A long iron-mesh drum on rollers across all three blocks, in a timber cradle, chutes under each block; the drum spins with
`spin` + `keep` and the items inside are drawn tumbling (each frame while it has items). A gravelly rattle whose rate
follows the speed.

---

## 9. Grinders in 0.4

- **The 0.1 grinder stays** as it is: GUI, input and output slots, hand-fed, the machine for the crank stage. A belt end
  facing its top or a non-socket side feeds it; a belt under its chute receives its push (logistics spec 6). Tossed items
  are pulled in.
- **The inline grinder** is new: the 0.1 grinder's recipes (`fras:grinding`), range (12 to 96 RPM) and load (6 Nm, 3 for
  wool and bones) as a two-block line machine (section 2). Recipe: shapeless, a grinder, a conveyor belt and a planks.

---

## 10. The reference ore line

This sizes the machines against each other; it is the line exit criterion 1 in `specs/progression.md` builds.

```
chest → hopper → belt → inline grinder → belt → washer → belt → trommel (fed at one end)
                                                                   ├ fines (dust) → belt → hoppers → 3 furnaces → belt → trip hammer → chest
                                                                   ├ mediums (crushed ore that slipped through, nuggets) → chest (or back to the washer)
                                                                   └ oversize → chest
```

On one line shaft at **64 RPM** *(tune)*:

| Machine | Per item | Rate at 64 RPM |
| --- | --- | --- |
| Grinder | 4 rev per raw ore → 1.5 crushed | 16 raw ore/min → 24 crushed/min |
| Washer | 2.5 rev per crushed | 25.6/min (keeps up) → 32 dust/min |
| Furnaces | 5 s per dust | 12/min each: **3 furnaces** keep up (36/min) |
| Trip hammer | 2 rev per plate | 32/min (just keeps up) |

Load: grinder 6 + washer 4 + trommel ~3 + hammer 8 + belts ~6 (four 16-block lines) ≈ **27 Nm** plus friction. A single
steam engine at 4 bar (stall 64 Nm, top 128 RPM) settles near **65 RPM** under it, a twin engine near 95 RPM. The governor
(target 64) holds it; the damper banks the fire when the chest of ore runs out. That is pillar 2's "a well-tuned factory
runs itself", end to end.

---

## 11. On Horizon

- **Block entities:** one type per block (spike lesson 1). A two-block machine keeps its state in the top block's block
  entity; the drive block has none and forwards its socket to the network. A formed trommel keeps its state in its middle
  block.
- **Placement** checks for room before the block is placed (the placement-state hook), so a refused placement never shows.
- **The built-in belt** reuses the belt line code (logistics spec 12) as a one-block line with its own speed and a hold
  point.
- **Recipe loading:** one loader for all four recipe types, generalised from the grinder's.
- **Fluids:** the washer's tank uses `ModFluidTanks` and the `FLUIDS` capability, as boilers and tanks do.
- **Drawing:** spinning parts with `spin` + `keep`; the hammer head and the trommel's tumbling items each frame only while
  working; idle machines are kept.
- **Hazards:** `entity_inside` for the saw blade; the hammer looks up entities in the strike box at each strike.
- **Saved:** the built-in belt's items, progress, the socket face, the washer's tank, the trommel's drum contents.

---

## 12. Code layout and tests

- **`sim::machine`**, generalised from `sim::grind`: the recipe model (item, tag, count, results, chances, revolutions,
  load, water), the space check, progress from RPM per machine range, completion with injectable randomness, kept/reset
  rules, push cadence, status codes. `sim::grind` becomes the grinder's table of numbers on top of it.
- **`sim::trommel`**: size classes, drum capacity, per-class outlet blocking.
- **The built-in belt** in `sim::belt` (logistics spec 12): hold point, in-place replacement, byproducts behind.
- **Unit tests** at least for: each recipe's yield distribution over seeded runs (4/3 dust, 10% byproduct, 8 rivets, 6
  planks); time per item at the bottom, middle and top of each range; no progress below the minimum with load still drawn;
  the washer stops without water and resumes when filled, and refuses a mixed tank; hot water washes like water; recipe files
  with `count` > 1 are refused; non-recipe items pass through; the next item waits until the byproducts are out; a machine
  that can't work holds its item; the drive's socket picks an adjacent shaft and never two; the trommel forms only from
  three in a row, routes each default tag to its outlet counted from the entry end (both ends), a blocked outlet stops only
  its class, and a full drum refuses input; the reference ore line's rates (section 10) in a game-free plant test, including the steady speed
  near 65 RPM on one 4-bar engine.

---

## 13. Starting numbers, recipes and terms *(tune)*

### 13.1 Numbers

| Machine | Range (RPM) | Load | Inertia | Repair |
| --- | --- | --- | --- | --- |
| Washer | 8 to 64 | 4 Nm (2 rusting) | 4 | 1 iron nugget, 1 planks |
| Trip hammer | 16 to 64 | 8 Nm | 6 | 2 iron nuggets |
| Lathe | 128 to 224 | 4 Nm (3 copper) | 2 | 1 iron nugget, 1 flint |
| Sawmill | 32 to 128 | 6 Nm (3 sticks) | 4 | 2 iron nuggets |
| Trommel | 8 to 96 | 2 Nm + 0.25 per item | 6 | 2 iron nuggets |

Inline grinder: as the 0.1 grinder (12 to 96 RPM, 6 Nm, 3 for wool and bones). Friction `c` 0.0125 for all, as the
grinder in 0.1's convention. Washer tank 1,000 mB, 50 mB per crushed ore, 200 mB per
nugget. Hammer 2 strikes per revolution, 2 damage per strike. Saw 1 damage per half second. Trommel 8 items, 1 revolution
per item. Dust smelts in 100 ticks (blast furnace 50). Push 1 item per 8 ticks.

### 13.2 Recipes (0.4)

| Result | Recipe |
| --- | --- |
| Trip hammer | shaped `CS ` / `LBL` / `PPP` (a conveyor belt, a wooden shaft, 2 logs, a block of iron as the head, 3 iron plates) |
| Lathe | shaped `PSP` / `IFI` / `LCL` (2 iron plates, an iron shaft, 2 iron ingots, a flint as the cutter, 2 logs, a conveyor belt) |
| Washer | shaped `PGP` / `KUK` / `PCP` (4 planks, an iron gear, 2 copper pipes K, a bucket U as the tub, a conveyor belt C) |
| Sawmill | shaped `IGI` / `PSP` / `PCP` (2 iron ingots, an iron gear as the blade, 4 planks, an iron shaft, a conveyor belt) |
| Inline grinder | shapeless: a grinder, a conveyor belt, a planks |
| Trommel ×3 | shaped `BBB` / `G S` / `BBB` (6 iron bars, an iron gear, an iron shaft): one drum's worth |
| Dusts → ingots | smelting and blasting, half the raw ore's time, its XP |

Every two-block machine's recipe includes the conveyor belt built into it (1.1). Shaped recipes may be mirrored.

The trip hammer and lathe need only hand-made parts (hand-hammered plates), so they are the bootstrap; the washer,
sawmill and trommel need an iron gear from the lathe (`specs/progression.md`).

### 13.3 Terms for the translation dictionary

Proposed Polish names (**mod** terms unless noted): Washer: Płuczka. Trip hammer: Młot ogoniasty (the historical Polish
name). Lathe: Tokarka. Sawmill: Trak (period word) or Piła tarczowa. Trommel: Przesiewacz bębnowy. Iron / copper / gold
dust: Pył żelaza / miedzi / złota. Inline grinder: Kruszarka przelotowa. Strike: uderzenie. Drive (block): napęd.
Fines / mediums / oversize: drobne / średnie / nadziarno.

---

## 14. Changes to the design doc

- 6.1: washing gives 4/3 dust per crushed ore (grinding and washing together 2×) plus a 10% nugget for copper and gold; dust smelts in half the time; gold's byproduct is a
  copper nugget.
- 6.2: the 0.4 machines are line machines with no inventory slots: two blocks tall, the machine on top with a belt
  running through it, a drive below; an inline grinder joins them, and the 0.1 grinder stays for hand feeding. The trip
  hammer also heads rods into rivets; the lathe makes rods, gears and copper pipes; the sawmill gives 6 planks per log and
  3 sticks per plank and hurts on touch; the trommel is three blocks in a row, sorting by item tags into fine, medium and
  oversize outlets underneath, counted from the end an item enters.
- 6.2: machines don't rust in 0.4.

---

## 15. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

**Reviewed by Marcin, 2026-11-26:** M1 and M6 decided as marked; everything else accepted as proposed.

1. **M1. Decided (Marcin, 2026-11-26): grinding and washing double the yield** in total: 4/3 dust per crushed ore plus a
   10% byproduct; dust smelts twice as fast (4.2).
2. **M2. The washer takes hot water as well as water** (4.3), a use for a closed loop's surplus.
3. **M3. Superseded in 1.1:** no belt mode and no slots; machines are stations with a built-in belt (section 16).
4. **M4. Rivets come from rods on the trip hammer** (8 per rod, 5.2), so a riveted part needs both the lathe and the
   hammer. *Alternative:* rivets from iron nuggets on the hammer (1 each), so the hammer alone makes them.
5. **M5. The machines bite:** the saw 1 damage per half second on touch, the hammer 2 per strike under its head (5.4,
   7.3). *Alternative:* no hazards in tier 1 beyond steam.
6. **M6. Decided (Marcin, 2026-11-26): the lathe runs at 128 to 224 RPM**, reached through a 2:1 step-up on new iron
   pulleys (max 192); wooden pulleys stay at 96 (6.3, `specs/progression.md` 4.4).
7. **M7. The trommel sorts by tags into three outlets** (8.3). 1.1: fines are sand and dust only; crushed ore, raw ore,
   nuggets, redstone and bone meal are medium. *Alternative:* a sieve with recipes (gravel → flint, sand → clay) instead of a sorter.
8. **M8. Sawmill recipes are per wood kind**, modded logs only through datapacks (7.2).
9. **M9. Recipe pages in the engine's item panel** (`panel_recipes`, grinder spec 8.2) are left out of 0.4; the tooltip
   lists what each machine takes, the input refuses the rest, and the docs page lists every recipe. *Alternative:* build
   them now that five machines have recipes (about two days).
10. **M10. Machines don't rust in 0.4** (2.6). *Alternative:* the iron machines (lathe, trommel, hammer) rust like shafts,
    jamming when leaking.

---

## 16. Revision 1.1 (Marcin, 2026-11-26, after testing 1.0 in game)

1.0's machines were awkward to use and didn't fit the rest: GUI slots, belt mode by what's underneath, a trommel that was
hard to place and let crushed ore and redstone into the fines. Decided in the thread (question numbers from there):

1. The washer, trip hammer, lathe and sawmill (and a new inline grinder) work **only in a line**: no inventory slots. A
   screen shows information only (15A). By hand, right-click puts an item on the built-in belt (5A, 13).
2. **Two blocks tall**, placed as one item like a door (4A): the machine on top with a **built-in belt** running straight
   through (2A) at the level of belts one block above the floor (11A), driven by the machine's own power (1A); the drive
   below with **one socket** on a side or the bottom, set by the wrench (3A).
3. Items with no recipe pass through; byproducts leave by the output too: **no side outlet**, the trommel sorts (17).
4. The **0.1 grinder stays**; an **inline grinder** is added (6A). The sawmill becomes a line machine too (7A).
5. Recipes needing more than one item go. Each machine's recipe takes a conveyor belt (18B).
6. **Washer water** through the top block's left and right sides, or a bucket (16A). **Redstone** is no longer a washing
   result (12).
7. **Trommel:** three blocks in a row form it like a boiler (20B), so placing never fails; not directional, fed at either
   end; outlets all underneath, counted from the end the item came in (19A); power from any long side face, one at a time,
   picked automatically (9). Fines are sand and dust only; crushed ore and nuggets are medium (12).
8. The sneak-wrench click stays information only.
9. 0.4 is unreleased, so machines already placed in test worlds aren't converted.
