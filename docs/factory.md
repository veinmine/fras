# The factory (0.4)

Specs: [logistics](../specs/logistics.md), [machines](../specs/machines.md),
[progression](../specs/progression.md). Every number here is a starting value (marked *tune* in the specs).

## Conveyor belts

- A belt takes the player's facing. Belts in a straight line with the same heading join into one **belt
  line** of up to **64 blocks**; a belt one block higher or lower along the heading makes a **slope**
  (automatic, like rails).
- **Drives:** a shaft, bearing, gearbox side, bevel output, clutch or pulley axle whose axis is horizontal and
  across the belt, ending against the side of a flat belt block, drives the line (the block shows a roller).
  A line needs one drive per 16 blocks. Drives on different networks join them, 1:1; at most **24 Nm** passes
  through the belt between drives, more shears it.
- **Speed** `|rpm| / 32` blocks/s (2 blocks/s at 64 RPM), two items per block. Load 0.125 Nm per block plus
  0.125 Nm per item (0.5 per item going up a slope). Above **128 RPM** a line overspeeds and jams.
- **Items** get on from machines pushing down, hoppers, another line's end, items tossed on it, or a
  right-click (sneak: two at once). An empty hand takes the nearest item off. Breaking a belt drops its items.
- **The leading end** hands items to an inventory (machines included), another line (its start or its side:
  that's how a line turns a corner), an elevator boot, or drops them if it faces air. Otherwise the line backs
  up. Players and mobs on a moving belt ride it (not while sneaking).
- **Wrench:** reverses the whole line. With leather or wool in the other hand it repairs a jam. Its overlay
  shows speed, items, drives and why the line stopped.

## Bucket elevator

A column of **2 to 32** elevator blocks: the bottom is the boot (takes items from any side and the top), the
top is the head (outlet on the side the player faced; the wrench turns it). Drive it at the head, from a side
other than the outlet. Lifts at `|rpm| / 32` blocks/s, 0.125 Nm per block plus 0.5 Nm per item, jams above
**96 RPM**, repair with 2 iron nuggets.

## Diverter

A belt block with a flap on the player's left (the wrench swaps it). **Powered**, it pushes items off the far
side into whatever is there (inventory, belt, boot, air); if that can't take the item, the item goes straight
on. It never stops the line. A comparator reads 15 for 2 ticks whenever an item passes.

## Machines

The washer, trip hammer, lathe, sawmill and inline grinder are **line machines**, two blocks tall and placed like
a door (the block above must be free). The top block is the machine, with a built-in belt at eye level running
straight through it from its back (input) to its front (output): it lines up with ordinary belts on either side.
The bottom block is the **drive**: one socket, on any side or underneath, that finds an adjacent shaft by itself
when placed and moves to another shaft if its own goes; the wrench cycles it through the faces that have a shaft.
Breaking either half breaks the machine; the drive carries the drop.

There are no slots. Items come in from a belt end at the back or a chute from above, a hopper on either half, or
a right-click with an item the machine has a recipe for (one at a time). The built-in belt moves at the
machine's own speed (|RPM| ÷ 32 blocks a second) and holds 2 items. Each item stops in the middle, is worked,
turns into its result, and rides on; byproducts follow it out of the front. Items with no recipe ride straight
through. At the front the machine hands items on like a belt end (belt, inventory, or the floor); if that is
blocked, the item waits, the machine stops, and the line behind it backs up. Nothing is lost.

The screen shows information only: progress, speed against the working range, status, and the washer's water.
A comparator reads 0 empty, 7 with one item, 11 with two, 15 when the output is blocked.

| Machine | Range (RPM) | Load | Repair | Does |
| --- | --- | --- | --- | --- |
| Washer | 8 to 64 | 4 Nm (2 rusting) | 1 iron nugget, 1 planks | Washes crushed ore into dust; needs water |
| Trip hammer | 16 to 64 | 8 Nm | 2 iron nuggets | Plates and rivets; 2 strikes per revolution |
| Lathe | 128 to 224 | 4 Nm (3 copper) | 1 iron nugget, 1 flint | Rods, gears, copper pipe |
| Sawmill | 32 to 128 | 6 Nm (3 sticks) | 2 iron nuggets | Logs to 6 planks, planks to 3 sticks |
| Inline grinder | 12 to 96 | 6 Nm | 1 flint | The grinder's one-item recipes, inline |
| Trommel | 8 to 96 | 2 Nm + 0.25 per item | 2 iron nuggets | Sorts by size into three outlets |

The 0.1 grinder stays as it was ([grinder.md](grinder.md)).

**Hazards:** the trip hammer's head hurts anything standing in its top block (2 per strike), the saw blade 1
every half second.

**Washer water:** a 1,000 mB tank filled by a water pipe on the left or right side of the top block, or a water
bucket; 50 mB per crushed ore, 200 mB per iron nugget (which becomes rust flakes). Dry, it holds its item.

**Trommel:** three trommel blocks in a straight row form one drum, like a boiler; crafted 3 at a time. A row of
4 or more, or a row with another trommel block touching its side, doesn't form (right-click says why). It has
no direction: items go in at either end face or the top of either end block. The outlets are all underneath,
counted from the end the item entered: fines (`#fras:trommel/fine`: sand, red sand, dusts) under the entry
block, mediums (`#fras:trommel/medium`: crushed and raw ore, nuggets, gravel, flint, redstone, coal, …) under
the middle, everything else under the far end. A shaft on any long side drives it, one socket at a time, chosen
by itself; the wrench cycles it. The drum holds 8 items; a full outlet stops only its own class.

Recipes are data like the grinder's: `data/fras/recipe/{washing,hammering,turning,sawing}/*.json`, types
`fras:washing` (with `water`), `fras:hammering`, `fras:turning`, `fras:sawing`.

| Machine | Input | Result | Revolutions |
| --- | --- | --- | --- |
| Washer | Crushed iron / copper / gold | 1 dust, 1 in 3 a second; 10% iron nugget / gold nugget / copper nugget | 2.5 |
| Washer | Iron nugget | Rust flakes | 64 |
| Trip hammer | Iron / copper ingot | Iron / copper plate | 2 |
| Trip hammer | Iron rod | 8 rivets | 3 |
| Lathe | Iron ingot | 2 iron rods | 8 |
| Lathe | Iron plate | Iron gear | 12 |
| Lathe | Copper plate | 3 copper pipes | 8 |
| Sawmill | A log, wood, stem or hyphae | 6 planks of its kind | 4 |
| Sawmill | Bamboo block | 3 bamboo planks | 4 |
| Sawmill | Any planks | 3 sticks | 2 |

Dust smelts into an ingot in 100 ticks (50 in a blast furnace).

## The reference ore line

Raw ore → belt → inline grinder → washer (dust) → trommel: fines (dust) to furnaces, mediums (missed crushed or
raw ore, nuggets) back to the grinder or a chest, oversize to a chest. Keep rock crushing (sand) on its own line:
sand sorts as fines too. `/fras scenario ore_line` builds one ([debugging.md](debugging.md)).
