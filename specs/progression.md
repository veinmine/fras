# Progression: subsystem spec

Spec 7 of the design doc's section 12, the tier-1 part. Version 1.0 (2026-11-26), scope: **milestone 0.4
(Factory)**. Builds on `Fire, rust and steam - design.md` v2.1 (pillar 4 and section 6.4), every recipe 0.1 to 0.3
shipped (`data/fras/recipe/`, the recipe sections of the earlier specs), and the two companion 0.4 specs,
`specs/logistics.md` and `specs/machines.md`. Where this spec is more precise than the design doc, this spec wins;
section 10 lists what changes in the design doc.

**Reviewed by Marcin on 2026-11-26** (decisions in the last section). Written in one go without an interview. Section 11 lists the open questions with the
assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. This spec also holds the **0.4 exit
criteria** for the whole milestone (section 9).

---

## 1. Scope

**In 0.4:** the **intermediate parts** (plates, rods, gears, rivets), the **tier-1 recipe tree** with the changes to
0.1 to 0.3 recipes that gate the better blocks behind them, the **bootstrap path** from vanilla materials to a working
factory, discovery, and the 0.4 exit criteria.

**Not in 0.4:** the tier-2 gate (steel, coke oven, blast furnace, the assembly table: design doc 6.4), the balance pass
over all of tier 1 (1.0), recipe pages (machines spec, question M9).

---

## 2. The principle

Pillar 4: **the factory makes the factory.** Higher tiers are built from parts earlier machines produce, on production
lines, not at a crafting table. In tier 1 this means three layers:

| Layer | Made from | Examples |
| --- | --- | --- |
| **Entry** | Vanilla materials in the crafting grid | Crank, water wheel, wooden shafts and gears, bearing, pulleys, wrench, hand hammer, grinder, firebox, copper boiler, copper pipe, valves, gauges, conveyor belt |
| **Hand-hammered** | Plates from the hand hammer (one ingot, one plate) | The first engine (steam cylinder), iron pipe, the lathe, the trip hammer |
| **Machine parts** | Rods and gears from the lathe, rivets from the trip hammer, plates from either | Gearbox, iron bevel gear, clutch, governor, damper, iron boiler, storage tank, heat exchanger, tachometer, the washer, sawmill, trommel, bucket elevator |

There is no special crafting block: the gate is the parts, made on a line (design doc 6.4). The hand hammer still makes
plates at the same yield as the trip hammer; making them one by one is the punishment (design doc 6.4), and the first
lathe and trip hammer cost about 11 hand-hammered plates between them and the engine that drives them.

---

## 3. Intermediate parts

| Item | Made on | From | Yield | Used for |
| --- | --- | --- | --- | --- |
| **Iron plate** (exists) | Hand hammer or trip hammer | Iron ingot | 1 | Boilers, cylinders, pipes, casings |
| **Copper plate** (exists) | Hand hammer or trip hammer | Copper ingot | 1 | Lathe → copper pipe; tier 2 |
| **Iron rod** (new) | Lathe | Iron ingot | 2 | Iron shafts, rivets |
| **Iron gear** (new) | Lathe | Iron plate | 1 | Gearbox, bevel gears, clutch, governor, tachometer, machines |
| **Rivet** (new) | Trip hammer | Iron rod | 8 | Boilers, tanks, gearbox, damper, heat exchanger |

All yields *(tune)*. Item tags for datapacks and other mods: `fras:plates/iron`, `fras:plates/copper`, `fras:rods/iron`,
`fras:gears/iron`, `fras:rivets` (with the conventional `c:` tags as well, if Horizon carries them). No storage blocks.
No copper rods or gears in tier 1: nothing would use them before electricity (copper wire, tier 3).

**What an iron part costs** in ingots, for balancing: plate 1, rod 0.5, gear 1, rivet 1/16.

---

## 4. The recipe tree

### 4.1 Entry and hand-hammered recipes: unchanged

Crank, water wheel, wooden shaft, wooden bevel gear, bearing, pulley, large pulley, leather belt, wrench, hand hammer,
grinder, firebox, copper boiler, steam dome, copper pipe, manual valve, redstone valve, steam release, emergency valve,
pressure gauge, sight glass, drain, copper and iron radiators, bellows, red oxide paint and every painted variant keep
their 0.1 to 0.3 recipes. So do the parts of **the first engine**: steam cylinder (hand-hammered plates, a piston, an iron
shaft), flywheel, pump. Iron pipe keeps its recipe (3 iron plates → 6): plates are what the trip hammer makes fast.

### 4.2 Recipes that change in 0.4

| Result | Was (0.1 to 0.3) | Now (0.4) |
| --- | --- | --- |
| **Iron shaft** | `I/I` (2 ingots) → 4 | Unchanged by hand; new: `R/R` (2 iron rods, one ingot) → 4 |
| **Gearbox** | `III/S S/III` (6 ingots, 2 iron shafts) | `PTP/G G/PTP` (4 iron plates, 2 rivets, 2 iron gears) |
| **Iron bevel gear** ×2 | `I I/ S /I I` (4 ingots, iron shaft) | `GSG` (2 iron gears, an iron shaft) |
| **Clutch** | `SRS` (2 iron shafts, redstone) | `SGS` / ` D ` (2 iron shafts, an iron gear, redstone dust) |
| **Governor** | `I I/ S /PVP` | `I I` / ` G ` / `PVP` (the gear replaces the iron shaft) |
| **Furnace damper** | `PPP/PRP` (5 plates, redstone) | `PTP` / `PDP` (4 iron plates, a rivet, redstone dust) |
| **Iron boiler** | `PPP/P P/PPP` (8 plates) | `PTP/P P/PTP` (6 iron plates, 2 rivets) |
| **Storage tank** ×2 | `PIP/I I/PIP` | `PTP/I I/PTP` (4 iron plates, 2 rivets, 2 ingots) |
| **Heat exchanger** | `PCP/C C/PCP` | `PTP/C C/PTP` (4 iron plates, 2 rivets, 2 copper pipes) |
| **Tachometer** | `NRN/NGN` (iron nuggets, redstone, glass pane) | `NAN` / `NGN` / ` D ` (4 iron nuggets, a glass pane, an iron gear, redstone dust) |

Letters: P iron plate, T rivets, G iron gear, R iron rod, S iron shaft, I iron ingot, C copper pipe, D redstone dust,
V manual valve, N iron nugget, A glass pane. Every painted variant's recipe (part + paint) is unchanged.

Why these and not others:

- **The design doc's list** (6.4): iron shafts, governor, gearbox, the lathe (from plates), bigger boilers. The iron boiler
  is the bigger boiler; riveting it is period-correct and makes the trip hammer matter.
- **The first engine stays reachable** without a lathe: the lathe's 128 RPM minimum (machines spec 6.3) wants an engine geared up through iron pulleys, so
  the engine can't need lathe parts. Iron shafts keep their hand recipe unchanged, so the steam cylinder and flywheel can
  be made before the lathe; rods make shafts at twice the yield afterwards (P2, decided).
- **Regulation and cooling** (governor, damper, heat exchanger) were 0.3's reward for a running plant; gating them behind
  the lathe moves them after the first production line, which is the order a player meets them anyway.
- **Pipes, valves, gauges, radiators** stay entry: a player needs them for the first boiler, and gating plumbing is tedious.

### 4.3 New 0.4 recipes

From the companion specs, collected here: conveyor belt, bucket elevator, diverter (logistics spec 13.2); trip hammer,
lathe, washer, sawmill, trommel, dust smelting (machines spec 13.2); the intermediate parts are machine recipes (section 3).

| Result | Layer | Needs from the factory |
| --- | --- | --- |
| Conveyor belt ×6 | Entry | — |
| Diverter | Hand-hammered | An iron plate |
| Trip hammer | Hand-hammered | 3 iron plates, a block of iron |
| Lathe | Hand-hammered | 2 iron plates, an iron shaft (hand recipe) |
| Iron pulley, large iron pulley | Entry | — (iron ingots, an iron shaft; 4.4) |
| Bucket elevator ×4 | Machine parts | An iron gear |
| Washer | Machine parts | An iron gear |
| Sawmill | Machine parts | An iron gear, an iron shaft |
| Trommel | Machine parts | An iron gear, an iron shaft |

### 4.4 Iron pulleys

The rotational spec planned them ("wooden in 0.1; iron later", 7.5); 0.4 needs them, because the lathe runs at 128 RPM or
more (machines spec 6.3) and wooden pulleys jam above 96.

- **Iron pulley** and **large iron pulley**: the wooden pulleys' rules (rotational spec 7.5: ratio by diameter, leather belt,
  24 Nm belt limit, 16 blocks), with a **maximum of 192 RPM** *(tune)*, inertia 3 and 6. Wooden and iron pulleys can share a
  belt; each keeps its own limit.
- **Recipes** (hand-made, so they come before the lathe): iron pulley ` I ` / `ISI` / ` I ` (4 iron ingots, an iron shaft);
  large iron pulley `III` / `ISI` / `III` (8 iron ingots, an iron shaft). Iron parts rust (`specs/rust.md` 3), so they get
  painted variants like every other iron part.
- **The step-up to the lathe:** a line shaft at 64 to 96 RPM, a large iron pulley on it, a belt to a small iron pulley on the
  lathe's countershaft: 128 to 192 RPM at the lathe, with the belt's 24 Nm far above the lathe's 4 Nm load. Run the line faster
  than 96 RPM and the small pulley overspeeds: the engineer's problem, with a governor (after the lathe) or a valve (before).

---

## 5. The bootstrap path

The route a player takes, which exit criterion 2 walks in game. Every step is reachable from the previous ones; no gated
item is needed before its machine exists.

1. **Hand power:** crank or water wheel, wooden shafts, grinder (crushed ore smelts to ingots), hand hammer.
2. **First steam:** firebox, copper boiler, steam dome, copper pipe, valves, pump, **steam cylinder** (6 hand-hammered
   plates) and **flywheel**: an engine at up to the copper boiler's rated pressure.
3. **First machines:** **lathe** (2 hand-hammered plates) and **trip hammer** (3 hand-hammered plates, a block of iron), on
   the engine's shaft. The lathe needs 128 RPM, which the first engine can't give under load (its top is 128 at 4 bar), so it
   runs off a 2:1 step-up through **iron pulleys** (4.4). Conveyor belts from leather or wool.
4. **First line:** ingots → lathe → rods; rods → trip hammer → rivets; ingots → trip hammer → plates; plates → lathe → gears.
   With a belt between them this is the first production line (design doc 6.4).
5. **Better blocks:** gearbox, iron bevel gears, clutch, governor, damper, tachometer, iron boiler, storage tank, heat
   exchanger, iron shafts at the good yield.
6. **The ore line:** washer, trommel, bucket elevator, diverter, sawmill; the reference ore line (machines spec 10).

The **gearbox** moves from step 1 to step 5. Before it, ratios come from **pulleys** (large to small, 2:1, entry recipes),
which is how a mill geared up anyway, and the way to reach the lathe's speed is iron pulleys (4.4); the 0.1 exit criteria that gear a water wheel up 4× still work with two pulley
stages (rotational spec 7.5, 24 Nm belt limit, well above a water wheel's output at speed).

---

## 6. Discovery

- **Recipe book:** each recipe unlocks on picking up one of its ingredients (as since 0.1). Picking up an iron rod unlocks
  the iron shaft from rods; an iron gear unlocks everything that needs one.
- **Tooltips** on intermediate parts say where they come from and one thing they make: "Iron gear. Turned on the lathe from
  an iron plate. Used in gearboxes and machines." Machines' Shift tooltips list what they take (machines spec 2.5).
- **Refusal is feedback:** machine inputs refuse items they have no recipe for.
- **Docs:** `docs/progression.md` with the tree in section 5 as a diagram, and a page per 0.4 machine with its recipes, as
  `docs/grinder.md`.

---

## 7. Existing worlds

FRAS is unreleased, so there are no worlds to migrate. Blocks already placed in development worlds keep working; only the
recipes change. The removed `I/I → 4` iron shaft recipe and the 0.1 to 0.3 recipes in 4.2 are replaced in place (same
recipe ids), so a recipe book that knew them shows the new ones.

---

## 8. Code and content

- Recipes and tags are generated by `tools/gen_content.py`, as since 0.1: new items (iron rod, iron gear, rivets, three
  dusts), new blocks (logistics and machines), changed recipes (4.2), machine recipe files for the four new recipe types,
  tags (`fras:dusts`, `fras:belt_fabrics`, `fras:trommel/fine`, `fras:trommel/medium`, the part tags of section 3).
- Placeholder textures from `tools/gen_textures.py` (own work), credits list unchanged in policy.
- A **recipe-tree test** (game-free, reading the generated JSON): every item in tier 1 is craftable from vanilla materials
  through FRAS's recipes and machine recipes; nothing in the hand-hammered layer needs a rod, gear or rivet; nothing gated in
  4.2 is craftable without one; the steps of section 5 are reachable in order. It fails the build if a future recipe change
  creates a loop or a dead end.
- **Language files:** every new key in English and Polish, terms added to `docs/translations/dictionary.md`.

---

## 9. 0.4 exit criteria (in game)

For the whole milestone, across the four specs. Driven through the Horizon launcher and its MCP server, with `/tick rate`
raised (or `/tick sprint`) while waiting, `/tp` between test sites, and `/give` for raw materials.

**A full ore line runs unattended:**

1. **The reference ore line** (machines spec 10), on one governed steam engine with a damper: a chest of 256 raw iron and a
   chest of coal feeding the furnaces by hopper. Left alone for 20 game minutes at a raised tick rate, every raw iron ends up
   processed: the plate chest holds **between 470 and 555 iron plates** (expected 512 from 256 × 1.5 × 4/3), the medium chest
   about 38 redstone dust (10% of the crushed ore), nothing waits on a belt, and no item is lost (items in chests + on belts +
   in machines and furnaces account for every input). The main shaft stays within the governor's band around 64 RPM while
   the line runs; when the ore runs out the line idles, belts run empty and the damper banks the fire.
2. **Backing up recovers:** filling the plate chest stops the trip hammer's belt, which backs up to the furnaces, the trommel,
   the washer and the grinder in turn, each showing "Output full" or holding its belt entry; emptying the chest restarts
   the whole line without losing an item.

**The factory makes the factory:**

3. **Bootstrap:** in a fresh survival world with the recipe book, starting from vanilla materials given by `/give` (no FRAS
   items), the steps of section 5 are completed in order. Before a lathe and trip hammer exist, the gearbox, governor, iron
   boiler and washer can't be crafted; afterwards they can, from parts the machines made.
4. **Parts:** the lathe turns ingots into 2 rods and plates into gears, the trip hammer heads a rod into 8 rivets and
   hammers plates at one per ingot; iron shafts from rods come out at 4 per 2 rods, by hand still 4 per 2 ingots.

**Logistics:**

5. **Belts:** a 32-block line on one shaft runs at 2 blocks/s at 64 RPM and visibly slows a crank-driven line as items are
   added; with one drive it doesn't move and the overlay says it needs two; a slope lifts items and costs more load; a corner
   by side-loading works; a line into a full chest backs up; a reversed shaft runs the line the other way; overspeed above
   128 RPM jams it and leather repairs it; a player standing on it is carried, sneaking isn't.
6. **Elevator and diverter:** a 16-high bucket elevator lifts items from a belt into a chest and backs up when the chest is
   full; a diverter with a lever sends items off to a side chest, passes them straight when that chest is full, and its
   comparator pulses count the items passing.

**Machines:**

7. **Every recipe** of the washer, trip hammer, lathe, sawmill and trommel works from its input; the washer and trip hammer
   also work over a belt, letting non-recipe items pass; the washer stops without water and runs on hot water; dust smelts
   in 5 s in a furnace.
8. **Ranges and hazards:** each machine below its minimum stops with "Too slow" and its load still drawn; above its maximum it
   warns, jams and is repaired with the wrench; the sawmill blade and the trip hammer hurt a player in survival.

**All of 0.4:**

9. **Pause and reload:** a belt line crossing the simulation distance pauses whole with its items in place; save and reload
   keep every item on belts and in elevators within 1/16 block of where it was, machine progress, the washer's water and the
   trommel's drum; breaking a belt drops its items.
10. **Performance** *(targets)*: 1,000 belt blocks in 16 or more lines carrying 1,000 items, plus 50 machines working, cost
    under **0.15 ms** of the mod's server time per tick on archmage; the client draws 1,000 moving items at under **1.5 ms**
    extra per frame.
11. **Copper golem** (only if the prototype is kept, `specs/copper-golem.md` 7): a golem assigned to a crank turns it, its
    arm moving with the crank, and stops when it oxidises into a statue.

---

## 10. Changes to the design doc

- 6.4: intermediate parts are iron plates, copper plates, iron rods, iron gears and rivets, with the yields in section 3; the
  gated blocks are those in 4.2; the gearbox moves behind the lathe, and pulleys give ratios before it; iron shafts keep their
  hand recipe, rods make them at twice the yield; iron pulleys (max 192 RPM) join the wooden ones.
- 13: materials add iron rods, iron gears, rivets and iron, copper and gold dust.

---

## 11. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

**Reviewed by Marcin, 2026-11-26:** P1 and P2 decided as marked; everything else accepted as proposed.

1. **P1. Decided (Marcin, 2026-11-26): the gearbox needs lathe parts** (4.2); pulleys give ratios before it (5).
2. **P2. Decided (Marcin, 2026-11-26): the hand recipe for iron shafts stays** at 4 per 2 ingots ("making them by hand is
   punishment enough"); rods give 4 per 2 rods, twice the yield per ingot (4.2).
3. **P3. The first engine stays hand-made** (steam cylinder, flywheel and pump unchanged, 4.1), so the lathe can be driven by
   it. *Alternative:* the cylinder needs rods (a piston rod) and the first lathe runs off a water wheel, which needs the lathe's
   range lowered (machines spec, question M6).
4. **P4. Regulation and cooling blocks move behind the lathe** (governor, damper, heat exchanger, 4.2). *Alternative:* only
   0.4's own machines and the gearbox, iron bevel gear, iron boiler and tank are gated; 0.3's blocks keep their recipes.
5. **P5. Plumbing stays entry** (pipes, valves, gauges, radiators, 4.1), apart from the tachometer, which gets a gear.
6. **P6. No copper rods or gears in tier 1** (3); copper plates go to the lathe for copper pipe.
7. **P7. Exit criterion 1 runs 256 raw iron through the reference line** and expects 470 to 555 plates. Fine as the measure of
   "a full ore line runs unattended"?
