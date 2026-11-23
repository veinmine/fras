# The rotational network (for players)

The full rules are in [specs/rotational-network.md](../specs/rotational-network.md); this page is the short
version.

## Push and pull

- **Sources push.** A crank gives 16 Nm at a standstill and nothing at 32 RPM; a water wheel up to 32 Nm,
  nothing at 24 RPM. The faster they turn, the less they push.
- **Machines and friction pull.** A grinder with work pulls 6 Nm; every part adds a little friction.
- The network settles where push and pull balance. Add a second grinder and everything slows down. Let go of
  the crank and the line coasts to a stop.

## Ratios

- A gearbox is 2:1: its large-gear face turns half as fast with twice the torque. The wrench flips it.
- A large pulley belted to a small one is 2:1 the same way. A crossed belt (wrench) reverses direction.
- A bevel gear turns the line by 90° at 1:1.

## What breaks

| Problem | Warning | Then |
| --- | --- | --- |
| **Overspeed**: a gear, gearbox or pulley over its max RPM (wood 64, iron 192, pulley 96) | Rattle, sparks; the tachometer goes red | After 5 s (faster at 1.5× the limit) the part **jams** |
| **Overload**: a shaft, bearing or clutch carrying more than its torque limit (wood 32, iron 128 Nm; half that while sagging) | Creaking, dust | After 5 s it **shears** (jams) |
| **Shock**: joining two lines turning opposite ways faster than 8 RPM | — | The part that joined them jams at once |
| **Conflict**: a loop of gears that can't agree on a speed | — | The part that closed the loop jams |

A jammed part splits the network there; the rest coasts. **Repair** it by right-clicking it with the wrench:

| Part | Repair |
| --- | --- |
| Wooden shaft, wooden bevel gear, pulleys, crank, water wheel | 1 stick |
| Iron shaft, bearing, clutch, iron bevel gear | 1 iron nugget |
| Gearbox | 2 iron nuggets |
| Grinder | 2 iron nuggets, 1 flint |

Free in creative.

## Supports

A run of plain shafts between two supports (bearings and every part except shafts and pulleys) may be 8 blocks
long if any of it is wooden, 16 if it's all iron. Longer, it **sags**: it wobbles, carries half its torque and
has more friction. A bearing needs something to hang on; take that away and it drops.

## Belts

Right-click two pulleys with a leather belt. Their axes must be parallel and the pulleys side by side in one
plane, at most 16 blocks apart. The belt costs one leather belt per 4 blocks. A belt carries at most 24 Nm at
its smaller pulley: above that it **slips** (squeals) and passes only 24 Nm; it grips again when the load
drops. Breaking a pulley gives the belt back.

## Inspecting

Sneak-right-click any part with the wrench: speed and direction, red line, load ("load 30 Nm, sources give
24 Nm"), torque carried versus limit, span, warning timers, jam reason and repair cost. A tachometer shows
the speed on a dial, and a comparator next to it gives 0 to 15 up to the red line.

## Unloaded chunks

A network with any part outside the simulated area **pauses** as a whole and keeps its speed (it still turns
on screen). It carries on when you come back. Speeds and jams are saved with the world.
