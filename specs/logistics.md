# Logistics: subsystem spec

Part of spec 6 (items and machines) in the design doc's section 12. Version 1.0 (2026-11-26), scope:
**milestone 0.4 (Factory)**. Builds on `Fire, rust and steam - design.md` v2.1 (section 6.3),
`specs/rotational-network.md` (sections 4, 6, 7.5, 9 and 10, which the belt plugs into), `specs/grinder.md`
(section 5, the machine faces belts connect to), `specs/multiblocks.md` (automatic formation, used by the bucket
elevator) and what 0.1 to 0.3 shipped (`docs/development.md`). Written together with `specs/machines.md` and
`specs/progression.md`, which cover the rest of 0.4, and `specs/copper-golem.md`, the optional prototype. Where
this spec is more precise than the design doc, this spec wins; section 14 lists what changes in the design doc.

**Not yet reviewed.** Written in one go without an interview, like the 0.2 and 0.3 specs. Section 15 lists the
open questions with the assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 13 collects them, with the
recipes. The **0.4 exit criteria** for the whole milestone are in `specs/progression.md`, section 9.

---

## 1. Scope

**In 0.4:** the **conveyor belt** (flat and sloped), the **bucket elevator**, the **diverter**, how items get on
and off them (machines, hoppers, chests, tossed items), processing on the belt (the hook the washer and trip
hammer use, `specs/machines.md` 3), their wrench overlay, looks and sounds.

**Not in 0.4:** the filter sorter (tier 3, design doc 6.3), belt curves (question L2), item extraction from the
middle of a belt (L5), chutes and item pipes (no plans).

---

## 2. What the player gets

Until 0.3 every machine was fed by hand or by a hopper chain, and the grinder dropped its output into a chest
under it. 0.4 moves items between machines, so a line of machines can run without the player:

| Part | Does | Teaches |
| --- | --- | --- |
| **Conveyor belt** | Carries items along a straight line; slopes climb one block per block | Belt speed follows shaft speed; every item is a little load; a struggling engine slows the whole line |
| **Bucket elevator** | Lifts items straight up | Lifting costs more than carrying |
| **Diverter** | A redstone flap that pushes items off to the side | Simple routing with redstone and comparators |

Belts also carry items **through** machines: a trip hammer or washer above a belt works on the items riding
under it (section 7).

---

## 3. Belt lines

### 3.1 Shape

- A **conveyor belt** block is a low slab-height belt (6 texels high) with a **heading**, set on placement to
  the player's horizontal facing. Belts are walkable and placeable on any solid top face.
- Adjacent belts with the **same heading, in line** (the next block along the heading, or one block up or down
  from it) join into one **belt line**. A line is straight in plan. Belts side by side, or with different
  headings, are separate lines.
- **Slopes:** a belt whose next belt along the heading is one block higher becomes a **rising slope**; one
  block lower, a **falling slope**. The shape is automatic, like rails, and updates when neighbours change.
  The wrench doesn't change it. A slope rises one block per block (45°), as design doc 6.3 says.
- **Length:** a line is at most **64 blocks** *(tune)* measured along the belt. A 65th belt doesn't join and the
  wrench says "Belt line too long (64 max)". The cap bounds the item list one block entity syncs and draws
  (section 10).
- **Turning a corner** is a hand-off from one line's end onto the side of another (section 5.2). That's how a
  mill did it, and it keeps a line one straight object (question L2).

### 3.2 Running direction

- Items move along the heading when the drive turns **forward** and back along it when the drive turns the
  other way. Which rotation is forward is fixed per belt by its heading (positive rotation about the drive's
  axis, by the right-hand rule, moves the top surface along the heading). A reversed shaft runs the line
  backwards; arrows on the belt show the way items actually travel.
- **The wrench on a belt** reverses its heading, which flips the whole line (every block in it). That's how a
  player fixes a line that runs the wrong way without touching the shaft.

### 3.3 Drives

- A belt line is a part of the rotational network. A shaft-like part (shaft, bearing's shaft, gearbox side,
  bevel gear output, clutch, pulley axle) whose axis is **horizontal and perpendicular to the heading** and
  which ends against a **side face of a flat belt block** drives the line there. That belt block shows a
  roller with a hub. Slopes can't be driven.
- **One drive per 16 blocks** *(tune)*: a line of length `L` needs `ceil(L / 16)` drives anywhere along it. With
  fewer, the line doesn't move and the overlay says "Needs 2 drives, has 1". The belt can still carry torque
  between its drives, see below.
- **Drives join their networks.** All drives of one line turn together at the same speed (one belt surface), so
  the line is a **1:1 link** between them, like a shaft. If the drives sit on different networks, the line
  joins them into one, with the same ratio and direction rules as a shaft (rotational spec 4.3): two drives
  turning opposite ways are a direction conflict (rotational spec 6.5), and the drive that joined last jams.
- **Torque through the belt** between two drives is capped at **24 Nm** *(tune)*, the leather belt's limit
  (rotational spec 7.5). More shears the line (jam, section 3.5). A belt is a poor way to carry power, as it
  should be.

### 3.4 Speed and load

- **Belt speed** = `|rpm| / 32` blocks per second *(tune)*: 1 block/s at 32 RPM, 2 at 64, 4 at 128.
- **Load** on the network while the line moves *(tune)*:
  - **0.125 Nm per block** of belt (rollers and friction).
  - **0.125 Nm per item** on a flat block, **0.5 Nm per item on a slope** going up; items going down a slope
    count as flat.
  - A 16-block flat line with 32 items draws 6 Nm, a grinder's worth; a full 64-block line with 128 items,
    24 Nm.
- **Inertia** 0.25 per block, **friction `c`** 0.0025 per block, as a shaft.
- **Speed range:** no minimum (a belt creeps at any speed). **Maximum 128 RPM** *(tune)*: above it the line
  overspeeds, warns (rattle, the belt flaps) and jams (rotational spec 6.2). This is why fast lathes and slow
  belts sit on different shafts.

### 3.5 Jam and repair

A jammed line stops, holds its items and stops transmitting torque between its drives (rotational spec 6.4).
Overlay: "Jammed: overspeed" or "Jammed: belt overloaded" (shear). Repair with the wrench and **1 leather or
1 wool** *(tune)* on any block of the line. A jam affects the whole line, since it is one belt.

---

## 4. Items on a belt

### 4.1 Model

- **The belt holds the items**, not the world: there are no item entities on a belt. Each line keeps an ordered
  list of **entries**, one item each, with a position along the line in blocks (0 at the line's start, `L` at
  its end) and a lateral offset of 0 (centred). This is deterministic, cheap and makes processing on the belt
  possible (question L1).
- **Spacing:** entries keep at least **0.5 blocks** *(tune)* apart, so a block holds two items. At 64 RPM a free
  line carries 4 items/s, more than any tier-1 machine produces (the grinder at 96 RPM makes ~0.6 crushed ore/s).
- **Each tick** a moving line advances every entry by `speed / 20` blocks, front to back: an entry stops
  behind the one ahead at the spacing, at a **hold point** (a machine working on it, section 7, or a diverter
  pushing it, section 9), or at the end if it can't leave (section 5.1). Items queue; nothing overlaps or
  vanishes.
- A stopped line (no speed, jammed, under-driven or paused) holds every entry where it is.

### 4.2 Getting on

| Source | Where the item lands | Rule |
| --- | --- | --- |
| **Machine output** pushing down (grinder, washer, trommel outlets, lathe, sawmill) | The block under the machine, at its centre | If there's room within ±0.25 of the centre; otherwise the machine keeps it and retries at its push rate. |
| **Hopper or dropper** pointing into the belt from above or the side | That block's centre | Same room rule; the item capability refuses when there's no room, so the hopper keeps it. |
| **Another line's end** (section 5.2) | The receiving block's centre, or position 0 when fed from behind | Same room rule. |
| **Tossed items** landing on the belt | Where they touch, rounded to the nearest free spot | One item every 4 ticks per block *(tune)*, taken from the item entity; the rest of the stack waits on the belt surface and is carried along with it (section 4.4). |
| **By hand:** right-click a belt with an item | That block's centre | One item per click; sneak-right-click puts the whole stack, one at a time as room appears. |

### 4.3 Getting off

- **At the leading end** (section 5).
- **By hand:** right-click an empty-handed belt takes the item nearest the cursor.
- **At a diverter** (section 9).
- **Breaking** a belt block drops the items on it as item entities; the line splits.
- **No extraction mid-line:** a hopper under a belt doesn't pull from it (question L5). The item capability of a
  belt block inserts only.

### 4.4 Entities on the belt

Players, mobs and item entities standing on a moving belt are carried along at the belt's speed (the `block_step_on`
and `entity_inside` hooks set their motion), and up slopes. Sneaking players aren't carried. It's fun, period and
honest: the belt really moves.

---

## 5. Ends and hand-offs

### 5.1 The leading end

The leading end is the end the items move toward (it swaps when the line runs backwards). When an entry reaches
it, the line offers the item to the block **in front** of the end, along the heading (for a slope's top, the block
in front of the top):

- **An inventory** (chest, barrel, hopper, furnace, any block with the item capability, including FRAS machines
  and their sided rules): inserted if accepted.
- **Another belt line** (section 5.2): handed over if it has room.
- **A bucket elevator's boot** (section 8): loaded.
- **Air or anything else:** the item **waits** at the end and the line backs up. Nothing is dropped into the world
  (question L3), consistent with machines (grinder spec 5.1).

A refused item waits and is offered again every 4 ticks *(tune)*; items behind it queue at the spacing. The wrench
overlay says "End blocked: chest full" or "End open: nothing to deliver to".

### 5.2 Line to line

- **Side-loading:** a line whose end faces the side of another line's flat block puts its items onto that block's
  centre. That's a corner, a merge and a T in one rule.
- **In line:** belts in line with the same heading join into one line (3.1). Past the 64-block cap they don't, and
  the full line hands its items onto the next line's position 0, so a long run is several lines end to end.
- **Head to head** (two leading ends facing each other): nothing passes; both back up.

---

## 6. Belts and machines

The grinder spec (5.1) already sends its push and insertion through the item capability, so belts plug in
without changes there. The general rules for every FRAS machine:

- **Machines push down.** A belt under a machine's chute receives its output (4.2).
- **Belts insert into machines** through the end hand-off (5.1). Each machine's sided rules apply (for the grinder,
  any face but its socket). A machine that refuses (wrong item, full input) makes the item wait at the belt's end.
- **Machines above a belt** (the washer and trip hammer) can instead work on the items passing under them
  (section 7).
- **0.1 limitation still applies:** a `ModContainer`'s faces are the same for every side in the pinned Horizon
  build (`docs/development.md`, "Where 0.1 differs"), so a belt's hand-off into a machine reaches its input
  through any face. Insertion through the item capability from a belt end is FRAS's own code, so FRAS checks the
  face itself there; only vanilla hoppers stay unchecked.

---

## 7. Processing on the belt

Some machines (the washer and the trip hammer, `specs/machines.md` 3) sit **directly above a flat belt block** and
work on items riding under them. The belt provides the hook; the machine decides what to do:

1. When an entry reaches the **centre** of a block with a belt machine above it, the line asks the machine whether
   it takes this item (it has a matching belt recipe, isn't jammed, is in range or at least turning, and has room
   for byproducts, below).
2. If yes, the entry **stops at the hold point** and the machine works on it at its normal rate. Entries behind it
   queue.
3. When the recipe completes, the entry's item is **replaced in place** by the main result, and any extra results
   (byproducts, extra rolls) are **inserted right behind it** on the belt, one entry each, at the spacing. A result
   that doesn't fit (the line behind is full) waits inside the machine and is placed as room appears; the machine
   doesn't start the next item until it is empty.
4. The entry is released and moves on. The machine takes the next one.

Items with no recipe for the machine pass under it untouched, so a mixed stream works. A machine that stops
(too slow, jammed, out of water) holds its entry and the line backs up behind it, as a stuck press would. Only
recipes that consume **one** item can run on a belt (counts above 1 need the machine's own input slot,
`specs/machines.md` 2.3).

---

## 8. Bucket elevator

### 8.1 Blocks and formation

- One block type, **bucket elevator**, stacked in a **vertical column of 2 to 32 blocks** *(tune)*. A column forms
  automatically, like a boiler (multiblock spec): the bottom block becomes the **boot** (intake), the top block
  the **head** (outlet), the ones between the **casing**. No hammer or click; sneak-right-click with the wrench
  says what's wrong ("Elevator too tall (32 max)", "No head: the top block is covered").
- **Facing:** the head's outlet faces the way the player faced when placing the top block; the wrench turns it to
  any horizontal side. The boot takes items from any side and the top.
- **Drive:** a shaft-like part ending against any of the **head's** three horizontal sides other than its outlet.
  Real elevators are driven from the head. One drive per
  column; the column is one part on the network.

### 8.2 Lifting

- **Lift speed** = `|rpm| / 32` blocks per second, as the belt. **Buckets** are spaced 0.5 blocks: each carries
  one item.
- **Load:** 0.125 Nm per block of column plus **0.5 Nm per item** being lifted *(tune)*. A 16-block column full
  of items (32) draws 18 Nm.
- **Range:** no minimum, **maximum 96 RPM** *(tune)*. Overspeed warns (buckets clatter) and jams; repair with
  **2 iron nuggets** *(tune)*.
- **Direction:** an elevator lifts whichever way it is driven. A real one run backwards would lower its buckets
  instead; that isn't worth teaching (question L8).
- **Delivery:** at the top the head offers the item to the block in front of its outlet, with the same rules as a
  belt's end (5.1): inventory, belt (side-load or a line's start), another elevator's boot, otherwise it waits and
  the column backs up.
- **Breaking** any block drops the items in the column at the broken block and splits it.

---

## 9. Diverter

- A **diverter** replaces one flat belt block: it is a belt block with an iron flap mounted on one side, part of the
  line like any belt block (it carries items, counts for length and load, and can't be driven).
- **Placing:** the flap goes on the side to the player's left; the wrench swaps it to the other side. Items are
  pushed **away from the flap**, off the opposite side.
- **Redstone:** while the diverter is **powered**, entries reaching its centre are pushed off sideways onto the
  block beside it: an inventory, the start or side of another line, or an elevator boot, with the 5.1 rules. While
  unpowered the flap lies flat and items pass straight.
- **When the side can't take an item** (full, or nothing there) the item **passes straight on** instead of
  waiting. A diverter never stops a line (question L4).
- Timing: the flap swings in 4 ticks (visible); it acts on the entry at its centre when powered at that moment.
- **Comparator:** a diverter gives a signal of 15 for 2 ticks *(tune)* whenever an item passes its centre, either
  way. With a comparator and a little redstone, a player can count items or divert every fourth one. No filter:
  sorting by item comes with electricity (design doc 6.3); sorting by size is the trommel's job.

---

## 10. On Horizon

### 10.1 Block entities and ticking

- **Belt blocks don't tick**, like shafts (rotational spec 10.2). Each line's state lives in a **level attachment**
  (in memory, like the rust counts in 0.3) owned by the game layer; `server::level_tick` steps every moving line
  once per tick, after the rotational network, from the speed of the network its drives are on.
- **Line membership** comes from block placement, breaking and chunk loading (`block_entity_on_load`,
  `on_chunk_unloaded`), as for networks. A line with any block outside the simulated area **pauses** whole
  (design doc 9).
- **The first block of the line** (position 0 end) is its **controller**: its block entity carries what clients need
  (10.3) and draws the whole line (10.4). Its render bounding box covers the line (`block_entity_render_bounding_box`).
- **Bucket elevators** are lines too, vertical, with the boot as controller.

### 10.2 Saving

- Each belt block entity saves the **entries inside its own block** (item stack and position within the block). The
  line's list is written back to its blocks on `level_save` and when a chunk unloads, not on every move: a block
  entity write per item crossing would cost more than the belt itself (0.3 measured ~50 µs per write).
- On load, a line is rebuilt from its blocks' saved entries, sorted by position.
- Breaking a block writes nothing: its entries are dropped (4.3).

### 10.3 Client sync

- The controller sends, in `block_entity_update_tag`: the line's speed (blocks per tick, signed), each entry's
  item and position, which entries are **held** (at a hold point or queued), and the game time of the snapshot.
- **Clients extrapolate.** Between updates, the client moves free entries at the synced speed, stops them at the
  spacing behind held ones and at the line's end, and draws them where they would be. The server sends a new
  snapshot only when something the client can't predict happens: an item gets on or off, an entry is held or
  released, or the speed changes by more than the rotational spec's sync threshold (max(0.5 RPM, 3%)).
- A steadily running line with nothing getting on or off sends nothing.

### 10.4 Drawing

- **Items** are drawn by the controller in `block_entity_submit` with the world item helper (`draw.item`,
  `Ground` transform, as a dropped item lies), one call per entry, at the extrapolated position plus the partial
  tick. Items aren't kept (they move), so a line with items is drawn every frame.
- **The belt surface** scrolls with the belt: its texture's UV offset follows the extrapolated travel. A line that is
  moving is drawn every frame; a **stopped, empty line is drawn once and kept** (`keep`).
- **Rollers** at each end and at each drive spin with `spin` + `keep` like shafts, in their own block entities.
- Beyond 48 blocks *(tune)* from the camera a line draws its items without the surface scroll, to save frame time.

### 10.5 Hooks used

`block_entity_submit`, `block_entity_render_bounding_box`, `block_entity_update_tag`, `block_entity_on_load`,
`on_chunk_unloaded`, `level_tick_post`, `level_save`, `block_step_on` and `entity_inside` (carrying entities and
picking up tossed items), the item capability (insert-only on belts), `analog_output` (diverter), redstone signal
reads (diverter). All confirmed in the pinned SDK; no mixin is expected.

---

## 11. Wrench overlay, tooltips, look and sound

**Overlay** (sneak-right-click a belt): speed (blocks/s and RPM), items on the line / capacity, load (Nm), drives
(has / needs), length, direction ("forward", "backwards"), end state (delivering to, blocked, open), jam reason. On
an elevator: height, items, load, outlet target. On a diverter: powered or not, side target.

**Tooltips:** Conveyor belt: "Carries items. Speed follows the shaft: 2 blocks/s at 64 RPM." Shift: "One drive per
16 blocks. Max 128 RPM. Joins belts in a straight line; slopes rise one block per block." Bucket elevator: "Lifts
items. Stack 2 to 32 high; drive the top." Diverter: "Pushes items off to the side while powered."

**Look:** a dark canvas or leather belt over wooden rollers in a timber frame, iron bracket at each drive; a red
oxide stripe every block so movement reads even on a plain texture. The elevator is a timber shaft with a window
showing iron buckets on a chain. The diverter's flap is a riveted iron plate on a hinge.

**Sound:** a soft rumble and roller squeak whose rate follows the speed (one looping sound per line, at the
controller, volume by length); the elevator's bucket clank; the diverter's flap clack; the rotational spec's rattle,
clunk and clank for overspeed, jam and repair.

---

## 12. Code layout and tests

- **`sim::belt`**, game-free: a line's geometry (blocks, slopes, drive positions, length), entries, the per-tick
  advance with spacing, hold points and end blocking, insertion with the room rule, side-load positions, the
  diverter's decision, the elevator as a vertical line, load and drive count, and the client extrapolation (the
  same function the client uses, so server and client agree). No Horizon types.
- **The rotational network** gains a belt line part: several sockets (its drives), 1:1 between them, torque cap
  24 Nm, max 128 RPM, inertia and friction from length. The solver doesn't change (rotational spec 9.3).
- **Unit tests** at least for: speed from RPM; spacing holds at any speed and tick; a blocked end backs the line up
  and nothing overlaps; insertion refused without room; side-load at a block's centre; slopes cost 0.5 Nm per item
  going up, 0.125 going down; drive count `ceil(L/16)` and an under-driven line doesn't move; two drives on
  different networks join them, opposite directions jam; a reversed line delivers at the other end; the diverter
  passes items straight when its side is full; an elevator of 33 blocks doesn't form; extrapolation matches the
  server's positions over 200 ticks with no events; save and load keep every entry and its position to within
  1/16 block; a 64-block line with 128 items steps in under 5 µs.

---

## 13. Starting numbers and recipes *(tune)*

### 13.1 Numbers

Belt: speed `|rpm| / 32` blocks/s; spacing 0.5 blocks; line length 64; one drive per 16 blocks; load 0.125 Nm per
block plus 0.125 Nm per item flat, 0.5 Nm per item rising; inertia 0.25 and friction 0.0025 per block; max 128 RPM;
torque between drives 24 Nm; repair 1 leather or 1 wool; tossed items taken every 4 ticks; blocked end retried every
4 ticks. Elevator: 2 to 32 high; lift `|rpm| / 32` blocks/s; 0.125 Nm per block plus 0.5 Nm per item; max 96 RPM;
repair 2 iron nuggets. Diverter: flap 4 ticks; comparator pulse 2 ticks. Surface scroll drawn within 48 blocks.

### 13.2 Recipes (0.4)

| Result | Recipe |
| --- | --- |
| Conveyor belt ×6 | shaped `FFF` / `PSP` (3 belt fabric: leather or any wool, tag `fras:belt_fabrics`; 2 planks; 1 wooden shaft) |
| Bucket elevator ×4 | shaped `PBP` / `PGP` / `PBP` (6 planks, 2 buckets, 1 iron gear) |
| Diverter | shapeless: 1 conveyor belt, 1 iron plate, 1 redstone dust |

The belt is an entry recipe (no machine parts), so the first lathe and trip hammer can be fed by belts. The elevator
needs an iron gear from the lathe (`specs/progression.md`). Each unlocks on picking up an ingredient.

### 13.3 Terms for the translation dictionary

Proposed Polish names (**mod** terms): Conveyor belt: Przenośnik taśmowy. Belt line: ciąg przenośnika. Slope:
pochylnia. Drive: napęd. Bucket elevator: Przenośnik kubełkowy. Boot / head: stopa / głowica (przenośnika).
Diverter: Zwrotnica.

---

## 14. Changes to the design doc

- 6.3: belt lines are straight in plan (corners are hand-offs onto another line's side); a line is at most 64 blocks
  and needs a drive per 16; belts hold their items (no item entities) at two per block; an end with nowhere to
  deliver backs up rather than dropping; belts carry players and mobs.
- 6.3: the bucket elevator is a column of one block type, 2 to 32 high, driven at the head.
- 6.3 / 7.1: the diverter pushes items off its side while powered, passes them straight when the side can't take
  them, and pulses a comparator for each item.
- 9: belt blocks don't tick; lines step from the level tick and save their items at chunk save.

---

## 15. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

1. **L1. Belts hold their items** in a list per line (4.1), drawn by the line, with no item entities. *Alternative:*
   real item entities pushed along by the belt (simpler to write, but item entities merge, despawn after 5 minutes,
   cost a tick each and can't be held under a machine).
2. **L2. Lines are straight; corners are side-loads** onto another line (3.1, 5.2). *Alternative:* curved belt blocks,
   which need curved rendering and a 2D path per line.
3. **L3. An end with nowhere to deliver waits** and the line backs up (5.1), as machines never drop items.
   *Alternative:* items fall off an open end as item entities, which some players use to feed a pit or a water stream.
4. **L4. A diverter that can't push passes the item straight on** (9). *Alternative:* it holds the item until the side
   frees up, which makes it a gate as well.
5. **L5. No extraction mid-line:** a hopper under a belt doesn't pull from it (4.3). *Alternative:* hoppers under a belt
   pull the item at that block's centre, the Minecraft-ish way to tap a line, which overlaps with the diverter.
6. **L6. One drive per 16 blocks, and drives join their networks** through the belt with a 24 Nm cap (3.3).
   *Alternative:* all drives must already be on one network, and a line with drives on two networks refuses to move.
7. **L7. Speed `rpm / 32` blocks per second, two items per block** (3.4, 4.1): 4 items/s at 64 RPM. Faster or slower?
   Vanilla hoppers move 2.5 items/s.
8. **L8. The bucket elevator lifts whichever way it is driven** (8.2). *Alternative:* it only lifts one way, and runs
   backwards (dropping nothing, just not lifting) the other.
9. **L9. Players and mobs ride belts** (4.4), sneaking players don't.
