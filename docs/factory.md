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

All five follow the grinder ([grinder.md](grinder.md)): one input and 2×2 outputs (the trommel differs), push
one item every 8 ticks into the inventory or belt below, take items tossed on top, a socket the wrench turns,
a GUI with a speed bar and a status line, the container comparator signal, a cheap repair.

| Machine | Range (RPM) | Load | Repair | Does |
| --- | --- | --- | --- | --- |
| Washer | 8 to 64 | 4 Nm (2 rusting) | 1 iron nugget, 1 planks | Washes crushed ore into dust; needs water |
| Trip hammer | 16 to 64 | 8 Nm | 2 iron nuggets | Plates and rivets; 2 strikes per revolution |
| Lathe | 128 to 224 | 4 Nm (3 copper) | 1 iron nugget, 1 flint | Rods, gears, copper pipe |
| Sawmill | 32 to 128 | 6 Nm (3 sticks) | 2 iron nuggets | Logs to 6 planks, planks to 3 sticks |
| Trommel | 8 to 96 | 2 Nm + 0.25 per item | 2 iron nuggets | Sorts by size into three outlets |

**Belt mode:** the washer and trip hammer placed directly on a flat belt block work on the items riding under
them: the item stops under the machine, turns into the result, and byproducts land right behind it. Items
with no recipe pass by. Anywhere else they work from their input slot (slot mode, where 4 sand → sandstone
also works on the hammer).

**Hazards:** the trip hammer's head hurts anything under it (2 per strike, in belt mode), the saw blade 1 every
half second.

**Washer water:** a 1,000 mB tank filled by a water pipe or a water bucket; 50 mB per crushed ore, 200 mB per
iron nugget (which becomes rust flakes).

**Trommel:** two blocks long, placed like a bed. Input at the start end and the top of the first block. Fines
(`#fras:trommel/fine`: sand, dusts, crushed ore) fall under the first block, mediums (`#fras:trommel/medium`:
gravel, flint, nuggets, raw ore, coal, …) under the second, everything else rolls out of the far end. The drum
holds 8 items, each tumbling one revolution.

Recipes are data like the grinder's: `data/fras/recipe/{washing,hammering,turning,sawing}/*.json`, types
`fras:washing` (with `water`), `fras:hammering`, `fras:turning`, `fras:sawing`.

| Machine | Input | Result | Revolutions |
| --- | --- | --- | --- |
| Washer | Crushed iron / copper / gold | 1 dust, 1 in 3 a second; 10% redstone / gold nugget / copper nugget | 2.5 |
| Washer | Iron nugget | Rust flakes | 64 |
| Trip hammer | Iron / copper ingot | Iron / copper plate | 2 |
| Trip hammer | Iron rod | 8 rivets | 3 |
| Trip hammer | 4 sand / red sand (slot mode) | Sandstone / red sandstone | 1 |
| Lathe | Iron ingot | 2 iron rods | 8 |
| Lathe | Iron plate | Iron gear | 12 |
| Lathe | Copper plate | 3 copper pipes | 8 |
| Sawmill | A log, wood, stem or hyphae | 6 planks of its kind | 4 |
| Sawmill | Bamboo block | 3 bamboo planks | 4 |
| Sawmill | Any planks | 3 sticks | 2 |

Dust smelts into an ingot in 100 ticks (50 in a blast furnace).

## The reference ore line

Raw ore → grinder → belt under a washer (dust) → trommel: fines to furnaces, mediums (missed raw ore) back to
the grinder or a chest, oversize to a chest. `/fras scenario ore_line` builds one ([debugging.md](debugging.md)).
