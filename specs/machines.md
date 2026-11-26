# Machines: subsystem spec

Part of spec 6 (items and machines) in the design doc's section 12. Version 1.0 (2026-11-26), scope:
**milestone 0.4 (Factory)**. Builds on `Fire, rust and steam - design.md` v2.1 (sections 6.1, 6.2 and 6.4),
`specs/grinder.md` (the reference machine: this spec reuses its recipe format, work model, inventory rules and
GUI, and only says where a machine differs), `specs/rotational-network.md` (sections 6 and 9, speed ranges,
jams and machine rules), `specs/fluids-and-pipes.md` and `specs/cooling.md` (the washer's water) and what 0.1 to
0.3 shipped (`docs/development.md`). Written together with `specs/logistics.md` (belts, and processing on the
belt, section 7 there) and `specs/progression.md` (what the machines make and what needs it). Where this spec is
more precise than the design doc, this spec wins; section 14 lists what changes in the design doc.

**Reviewed by Marcin on 2026-11-26** (decisions in the last section). Written in one go without an interview. Section 15 lists the open questions with the
assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 13 collects them, with the
recipes. The **0.4 exit criteria** for the whole milestone are in `specs/progression.md`, section 9.

---

## 1. Scope

**In 0.4:** the **washer**, **trip hammer**, **lathe**, **sawmill** and **trommel**: their recipes, speed ranges,
loads, faces, GUIs, belt behaviour, looks, sounds and hazards; the **dust** items; the grinder's belt hookup; a
reference ore line that sizes them against each other.

**In the companion specs:** belts, elevators and the diverter (`specs/logistics.md`); intermediate parts and the
recipe tree (`specs/progression.md`); the copper golem (`specs/copper-golem.md`).

**Designed for, built later:** water cooling for machines other than the grinder (cooling spec 9.3), the stamp mill
and electric pulveriser (tier 2+), recipe pages in the item panel (question M9), Recipes viewer pages (upstream).

---

## 2. Rules every machine follows

The grinder spec is the template. Unless a machine's section says otherwise, it behaves exactly like the grinder
in these respects.

### 2.1 Recipes

- Each machine has its own recipe type, with the **grinder's JSON format** (grinder spec 2.2): `ingredient` (item or
  tag), `count`, `results` (1 to 4, each with `count` and per-item `chance`), `revolutions`, `load`. Types:
  `fras:washing`, `fras:hammering`, `fras:turning` (lathe), `fras:sawing`. The trommel sorts by tags, not recipes
  (section 8).
- Extra fields: washing has `water` (mB per item, default 50). Hammering counts **strikes** in the GUI (2 per
  revolution) but stores revolutions like the rest, so one model serves every machine.
- Recipes are FRAS's own data files; datapacks can add or override them.

### 2.2 Work and load

- Progress in revolutions: `|rpm| / 1200` per tick inside the range, nothing below the minimum (with the load still
  drawn), capped at the maximum while overspeeding (grinder spec 3, rotational spec 9.2).
- Load only while it **has work**, otherwise friction only.
- The working range comes from a function of the machine's state, so cooling can widen it later.

### 2.3 Inventory, belt mode and slot mode

- **Slots:** 1 input, 4 outputs (2×2), as the grinder, except the trommel (section 8). Space check with maximum rolls;
  nothing voided; progress kept across stops and saved; reset only when the input changes.
- **Belt machines** (washer, trip hammer) work in one of two modes, chosen by what is under them:
  - **Belt mode:** directly above a flat belt block, the machine works on items riding under it (logistics spec 7).
    Its input slot refuses items; the GUI says "Working on the belt". Only recipes with `count` 1 run.
  - **Slot mode:** anywhere else, it works from its input slot and pushes down like the grinder. Every recipe runs.
- **Other machines** (lathe, sawmill, trommel) are always fed through their input, by hoppers, belt ends or hand.

### 2.4 Faces, input and output

- **One shaft socket** on a side (placed facing an adjacent shaft's end, else facing the player; the wrench cycles it).
- **Input** through the top and the sides except the socket; **output pushed down** at 1 item every 8 ticks into the
  inventory or belt below, otherwise kept. **Extraction** takes outputs only. Items tossed on top are pulled in,
  which 0.1 left out for the grinder (`docs/development.md`); 0.4 adds it for every machine at once.
- Right-click opens the GUI; the wrench rotates the socket, repairs, and inspects.
- **Comparator:** the vanilla container signal over all slots. **No redstone control:** stop a machine with a clutch.

### 2.5 GUI

The grinder's layout: input left, progress arrow, 2×2 outputs right, player inventory below, a **speed bar** under
the arrow with the machine's working band in green and its maximum in red, and the **status line**. Each machine adds
its own status lines (e.g. "No water"). Data slots as the grinder's (progress, signed RPM × 10, status, max RPM).

### 2.6 Overspeed, jam and repair

Overspeed warns, then jams (rotational spec 6.2, 6.4); a jammed machine keeps its inventory open to hoppers and the GUI
and holds any belt entry it was working on. Each machine has a cheap repair (section 13).
**Machines don't rust in 0.4**: `specs/rust.md` doesn't list the grinder either (question M10).

---

## 3. Belt machines: shared placement

The washer and trip hammer sit **on top of a belt**: the machine block occupies the space directly above a flat belt
block and works into it (the hammer's head drops into the belt's space, the washer sprays down). They are full blocks
in slot mode, and keep the same block in belt mode; only the model shows the lower parts reaching down.

- Both modes need the machine to be **in range** to make progress, as everywhere.
- In belt mode the item under the machine is held at the belt's centre while it works (logistics spec 7). Items
  without a recipe pass under it.
- Byproducts and extra rolls go onto the belt right behind the main result. If the belt behind is full they wait
  inside, and the machine doesn't take the next item until they are out.
- A belt machine over a slope or a diverter is in slot mode.

---

## 4. Washer

### 4.1 Role

Crushed ore → **dust** plus a small byproduct; rusting iron nuggets into **rust flakes** (design doc 5, 6.1).
Water-fed. Works on a belt or from its slot.

### 4.2 Recipes (`fras:washing`)

| Input | Results | Revolutions | Water | Load |
| --- | --- | --- | --- | --- |
| Crushed iron | 1 iron dust, 1 in 3 a 2nd; 10% 1 redstone dust | 2.5 | 50 mB | 4 Nm |
| Crushed copper | 1 copper dust, 1 in 3 a 2nd; 10% 1 gold nugget | 2.5 | 50 mB | 4 Nm |
| Crushed gold | 1 gold dust, 1 in 3 a 2nd; 10% 1 copper nugget | 2.5 | 50 mB | 4 Nm |
| Iron nugget | 1 rust flakes | 64 | 200 mB | 2 Nm |

All values *(tune)*. Rationale:

- **Grinding and washing double the yield** (M1, decided): 4/3 dust per crushed ore (chance 0.3333 in the recipe file), so
  the grinder's 1.5× becomes **2×**, plus the byproduct.
  Tier 2's stamp mill pushes toward 3× (design doc 6.1). Smelting crushed ore directly keeps working.
- **Dust smelts in half the time** (100 ticks in a furnace, 50 in a blast furnace) to one ingot, with the raw ore's
  XP. A furnace bank keeps up with a washer with fewer furnaces, which matters to an unattended line.
- **Rust flakes without scraping** (design doc 5, paint supply): slow on purpose, one nugget every 64 revolutions
  (2 minutes at 32 RPM), so a paint line is a long-running sideline, not a flood. Flakes → grinder → pigment →
  paint is unchanged.
- **Dust items:** iron dust, copper dust, gold dust, tag `fras:dusts`. No storage blocks.

### 4.3 Water

- **A tank of 1,000 mB** *(tune)* filled through a **water port** on any side except the socket, from a pipe
  (fluids spec), or by hand with a water bucket. **Water or hot water** both work: a closed-loop plant can send its
  surplus hot water here instead of a drain (cooling spec 8). The tank holds one content at a time, like a pipe network.
- Water is used per item at completion (the `water` field) and is lost (it drains away under the machine, with a few
  drip particles; no waste water fluid).
- "Has work" also needs enough water for the recipe. Status "No water"; the item waits (in belt mode, held).
- Sight of the level: the GUI shows a small fluid bar; the overlay shows mB.

### 4.4 Machine numbers

Range **8 to 64 RPM** *(tune)*, inertia 4, friction as the grinder, repair **1 iron nugget and 1 planks** *(tune)*.
At 64 RPM a crushed ore takes 2.3 s: a washer keeps up with a grinder on the same shaft (2.5 versus 4 × 2/3 = 2.7
revolutions per crushed ore, section 10).

### 4.5 Look and sound

A wooden tub on legs with a paddle drum turning in it, iron hoops, a spray bar underneath (visible in belt mode). The
drum spins with `spin` + `keep`. Splashing loop while working, pitch by RPM; drip particles below; the item's break
particles in the tub.

---

## 5. Trip hammer

### 5.1 Role

Ingots → **plates**; heading rods into **rivets**; compacting (design doc 6.2). Loud, period and fun to watch. Works on
a belt or from its slot.

### 5.2 Recipes (`fras:hammering`)

| Input | Results | Strikes (revolutions) | Load |
| --- | --- | --- | --- |
| Iron ingot | 1 iron plate | 4 (2) | 8 Nm |
| Copper ingot | 1 copper plate | 4 (2) | 8 Nm |
| Iron rod | 8 rivets | 6 (3) | 8 Nm |
| 4 sand (slot mode only) | 1 sandstone | 2 (1) | 8 Nm |
| 4 red sand (slot mode only) | 1 red sandstone | 2 (1) | 8 Nm |

All values *(tune)*. Plates at the **same yield** as the hand hammer (design doc 6.4): the trip hammer is about speed
and automation. Rivets need a rod from the lathe, which makes lathe → hammer the first production line
(`specs/progression.md`). Compacting sandstone is a small convenience that teaches slot mode.

### 5.3 Strikes

- A **cam** on the shaft lifts the hammer **twice per revolution**; each drop is one strike: a sound, sparks on metal,
  and a step of progress.
- Load: an average of 8 Nm while working. The solver sees a steady load, so the network doesn't jitter, but the
  tachometer readout shows the speed dipping a little at each strike *(tune: 1 Nm of ripple, drawn, not simulated)*.

### 5.4 Machine numbers and hazard

- Range **16 to 64 RPM** *(tune)*: a hammer is slow. At 64 RPM a plate takes 1.9 s. Above 64 the head bounces and the
  hammer jams. Inertia 6 (the head), repair **2 iron nuggets**.
- **Hazard:** in belt mode, a player or mob in the belt's space under the head takes **2 damage per strike** *(tune)*
  while the hammer is working (a rider on the belt gets hit on the way through). In slot mode the head strikes its own
  anvil inside the block and hurts nobody. It warns: the
  head is visible and every strike is loud. Kid Mode's invulnerability covers it.

### 5.5 Look and sound

A timber frame with a cam shaft on top and a heavy iron head on a wooden helve, an anvil block under it in slot mode
(the machine's own lower half, drawn) or the belt in belt mode. The head's motion is drawn each frame while working
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

A cast-iron bed on legs, headstock with the socket and a stepped pulley, tailstock, tool rest. The workpiece (the input
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
- **Hazard:** the blade stands proud of the table. A player or mob touching the blade's block while it turns above its
  minimum takes **1 damage every half second** *(tune)*, like a hot pipe (0.2 measured that 1 per second was cancelled by
  regeneration). Warning: the whirring and the visible spinning blade. Kid Mode covers it.

### 7.4 Look and sound

A timber table with a circular blade standing out of a slot, the socket under the table on one side. The blade spins
with `spin` + `keep`. A buzz that drops in pitch while cutting; sawdust particles.

---

## 8. Trommel

### 8.1 Role

A rotating sieve that **sorts items by size** into three outlets (design doc 6.2): steam-era sorting. In an ore line it
splits crushed ore from gravel and from anything that wasn't ground, so a player can dump a mining haul in one end.

### 8.2 Block

- A **two-block machine** placed horizontally, like a bed: an iron-mesh drum lying along its length. The player places
  it facing away; it fills the block in front too.
- **Input:** the **start face** (the end nearest the player) and the top of the first block, from hoppers, belt ends or
  by hand. Every item is accepted: anything without a size class is oversize.
- **Socket:** on a **long side of the first block**, its axis across the drum: the shaft drives the rollers the drum rests
  on through a girth gear, as real trommels are driven. The wrench swaps it to the other side.
- **Outlets:** **fines** fall through the fine mesh under the **first** block; **mediums** through the coarse mesh under
  the **second** block; **oversize** items roll out of the **far end face**.

Each outlet pushes into the inventory or belt below (in front, for oversize) at 1 item every 8 ticks, like every
machine's push, and waits if blocked.

### 8.3 Sorting

- **Size classes come from item tags**, so datapacks can extend them: `fras:trommel/fine` and `fras:trommel/medium`.
  Anything in neither is **oversize**.
- Defaults *(tune)*:
  - **Fine:** sand, red sand, all dusts (`fras:dusts`), all crushed ores (`fras:crushed_ores`). Fine and smeltable, so
    the fine outlet can feed furnaces directly (section 10).
  - **Medium:** gravel, flint, iron, gold and copper nuggets, redstone dust, bone meal, clay ball, coal, charcoal, rust
    flakes, red oxide pigment, and raw ores (lumps the grinder wants back: route the medium outlet to the grinder, or a
    chest). Redstone and bone meal are powders, but grading them medium keeps the washer's byproducts out of the furnaces.
  - **Oversize:** everything else (cobblestone, cobbled deepslate, dirt, blocks, tools).
- An item tumbles **1 revolution** *(tune)* before it falls. The drum holds **up to 8 items** at once (visible tumbling);
  a full drum refuses input. A blocked outlet stops only items of that class; the others keep moving through.

### 8.4 Machine numbers

Range **8 to 96 RPM** *(tune)*, load **2 Nm plus 0.25 Nm per item in the drum** (up to 4 Nm), inertia 6, repair **2 iron
nuggets**. At 64 RPM it sorts about 8 items/s across its outlets, faster than the belts feeding it. No recipe, no
progress arrow: its GUI shows the 8 drum slots and the three outlet buffers (one slot each).

### 8.5 Look and sound

A long iron-mesh drum on rollers in a timber cradle, chutes under each half; the drum spins with `spin` + `keep` and the
items inside are drawn tumbling (each frame while it has items). A gravelly rattle whose rate follows the speed.

---

## 9. Grinder in 0.4

- **Belts:** a belt under the chute receives its push; a belt end facing its top or a non-socket side feeds it (logistics
  spec 6). Nothing else changes in its rules.
- **Tossed items** are pulled in (grinder spec 5.2), the 0.1 gap closed for every machine together.
- No belt mode: the grinder only works from its input.

---

## 10. The reference ore line

This sizes the machines against each other; it is the line exit criterion 1 in `specs/progression.md` builds.

```
chest → hopper → belt → grinder → belt → washer (belt mode) → belt → trommel
                                                                 ├ fines (dust) → belt → hoppers → 3 furnaces → belt → trip hammer (belt mode) → chest
                                                                 ├ mediums (byproducts, unground raw ore) → chest (or back to the grinder)
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

- **Block entities:** one type per block (spike lesson 1), each ticking (machines tick). The trommel is two blocks with one
  block entity in the first (the second is a dummy that forwards its outlet and drawing).
- **Belt mode** reads the block below each tick it needs it (cheap: a block state read) and registers the machine as a
  hold point with the line (logistics spec 7).
- **Recipe loading:** one loader for all four recipe types, generalised from the grinder's.
- **Fluids:** the washer's tank uses `ModFluidTanks` and the `FLUIDS` capability, as boilers and tanks do.
- **Drawing:** spinning parts with `spin` + `keep`; the hammer head and the trommel's tumbling items each frame only while
  working; idle machines are kept.
- **Hazards:** `entity_inside` for the saw blade; the hammer looks up entities in the strike box at each strike.
- **Saved:** inventory, progress, socket side, mode-independent state (belt-mode holds are re-registered on load), the
  washer's tank, the trommel's drum contents and outlet buffers.

---

## 12. Code layout and tests

- **`sim::machine`**, generalised from `sim::grind`: the recipe model (item, tag, count, results, chances, revolutions,
  load, water), the space check, progress from RPM per machine range, completion with injectable randomness, kept/reset
  rules, push cadence, status codes. `sim::grind` becomes the grinder's table of numbers on top of it.
- **`sim::trommel`**: size classes, drum capacity, per-class outlet blocking.
- **Belt mode** in `sim::belt` (logistics spec 12): hold points, in-place replacement, byproducts behind.
- **Unit tests** at least for: each recipe's yield distribution over seeded runs (4/3 dust, 10% byproduct, 8 rivets, 6
  planks); time per item at the bottom, middle and top of each range; no progress below the minimum with load still drawn;
  the washer stops without water and resumes when filled, and refuses a mixed tank; hot water washes like water; belt mode
  refuses `count` > 1 recipes and passes non-recipe items; byproducts wait inside when the belt behind is full and the next
  item isn't taken; the trommel routes each default tag to its outlet, a blocked outlet stops only its class, and a full
  drum refuses input; the reference ore line's rates (section 10) in a game-free plant test, including the steady speed
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

Friction `c` 0.0125 for all, as the grinder in 0.1's convention. Washer tank 1,000 mB, 50 mB per crushed ore, 200 mB per
nugget. Hammer 2 strikes per revolution, 2 damage per strike. Saw 1 damage per half second. Trommel 8 items, 1 revolution
per item. Dust smelts in 100 ticks (blast furnace 50). Push 1 item per 8 ticks.

### 13.2 Recipes (0.4)

| Result | Recipe |
| --- | --- |
| Trip hammer | shaped ` S ` / `LBL` / `PPP` (wooden shaft, 2 logs, a block of iron as the head, 3 iron plates) |
| Lathe | shaped `PSP` / `IFI` / `LLL` (2 iron plates, an iron shaft, 2 iron ingots, a flint as the cutter, 3 logs) |
| Washer | shaped `PGP` / `CKC` / `PPP` (5 planks, an iron gear, 2 copper pipes, a bucket as the tub) |
| Sawmill | shaped `IGI` / `PSP` / `PPP` (2 iron ingots, an iron gear as the blade, 5 planks, an iron shaft) |
| Trommel | shaped `BBB` / `G S` / `BBB` (6 iron bars, an iron gear, an iron shaft) |
| Dusts → ingots | smelting and blasting, half the raw ore's time, its XP |

The trip hammer and lathe need only hand-made parts (hand-hammered plates), so they are the bootstrap; the washer,
sawmill and trommel need an iron gear from the lathe (`specs/progression.md`).

### 13.3 Terms for the translation dictionary

Proposed Polish names (**mod** terms unless noted): Washer: Płuczka. Trip hammer: Młot ogoniasty (the historical Polish
name). Lathe: Tokarka. Sawmill: Trak (period word) or Piła tarczowa. Trommel: Przesiewacz bębnowy. Iron / copper / gold
dust: Pył żelaza / miedzi / złota. Strike: uderzenie. Belt mode: praca na przenośniku. Fines / mediums / oversize: drobne /
średnie / nadziarno.

---

## 14. Changes to the design doc

- 6.1: washing gives 4/3 dust per crushed ore (grinding and washing together 2×) plus a 10% byproduct; dust smelts in half the time; gold's byproduct is a
  copper nugget.
- 6.2: belt machines (washer, trip hammer) work on a belt when placed over one and from their own slot otherwise; the trip
  hammer also heads rods into rivets and compacts sand; the lathe makes rods, gears and copper pipes; the sawmill gives 6
  planks per log and 3 sticks per plank and hurts on touch; the trommel is a two-block drum sorting by item tags into fine,
  medium and oversize outlets.
- 6.2: machines don't rust in 0.4.

---

## 15. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

**Reviewed by Marcin, 2026-11-26:** M1 and M6 decided as marked; everything else accepted as proposed.

1. **M1. Decided (Marcin, 2026-11-26): grinding and washing double the yield** in total: 4/3 dust per crushed ore plus a
   10% byproduct; dust smelts twice as fast (4.2).
2. **M2. The washer takes hot water as well as water** (4.3), a use for a closed loop's surplus.
3. **M3. Belt machines switch mode by what is under them** (2.3): over a belt they only work on the belt.
   *Alternative:* they always have a slot as well and work on both, which is more flexible but harder to read.
4. **M4. Rivets come from rods on the trip hammer** (8 per rod, 5.2), so a riveted part needs both the lathe and the
   hammer. *Alternative:* rivets from iron nuggets on the hammer (1 each), so the hammer alone makes them.
5. **M5. The machines bite:** the saw 1 damage per half second on touch, the hammer 2 per strike under its head (5.4,
   7.3). *Alternative:* no hazards in tier 1 beyond steam.
6. **M6. Decided (Marcin, 2026-11-26): the lathe runs at 128 to 224 RPM**, reached through a 2:1 step-up on new iron
   pulleys (max 192); wooden pulleys stay at 96 (6.3, `specs/progression.md` 4.4).
7. **M7. The trommel sorts by tags into three outlets** (8.3). Fines are only what a furnace takes (sand, dusts, crushed
   ore), so redstone and bone meal count as medium; raw ores count as medium so they can loop back to the grinder. *Alternative:* a sieve with recipes (gravel → flint, sand → clay) instead of a sorter.
8. **M8. Sawmill recipes are per wood kind**, modded logs only through datapacks (7.2).
9. **M9. Recipe pages in the engine's item panel** (`panel_recipes`, grinder spec 8.2) are left out of 0.4; the tooltip
   lists what each machine takes, the input refuses the rest, and the docs page lists every recipe. *Alternative:* build
   them now that five machines have recipes (about two days).
10. **M10. Machines don't rust in 0.4** (2.6). *Alternative:* the iron machines (lathe, trommel, hammer) rust like shafts,
    jamming when leaking.
