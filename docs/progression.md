# Progression (0.4)

Spec: [specs/progression.md](../specs/progression.md). The factory makes the factory: better blocks need parts
that machines make.

| Layer | Made from | Examples |
| --- | --- | --- |
| Entry | Vanilla materials | Crank, water wheel, wooden shafts and gears, pulleys (wooden and iron), grinder, firebox, copper boiler, pipes, valves, gauges, conveyor belt |
| Hand-hammered | Hand-hammer plates | Steam cylinder, iron pipe, lathe, trip hammer, diverter |
| Machine parts | Iron rods and gears (lathe), rivets (trip hammer), plates | Gearbox, iron bevel gear, clutch, governor, damper, tachometer, iron boiler, storage tank, heat exchanger, washer, sawmill, trommel, bucket elevator |

## New parts

| Item | Made on | From | Yield |
| --- | --- | --- | --- |
| Iron rod | Lathe | Iron ingot | 2 |
| Iron gear | Lathe | Iron plate | 1 |
| Rivet | Trip hammer | Iron rod | 8 |

Tags: `fras:plates/iron`, `fras:plates/copper`, `fras:rods/iron`, `fras:gears/iron`, `fras:rivets`.

## Recipes that changed

| Result | Now |
| --- | --- |
| Iron shaft ×4 | Still 2 ingots; also 2 iron rods |
| Gearbox | 4 iron plates, 2 rivets, 2 iron gears |
| Iron bevel gear ×2 | 2 iron gears, an iron shaft |
| Clutch | 2 iron shafts, an iron gear, redstone dust |
| Governor | The iron gear replaces the iron shaft |
| Furnace damper | 4 iron plates, a rivet, redstone dust |
| Iron boiler | 6 iron plates, 2 rivets |
| Storage tank ×2 | 4 iron plates, 2 rivets, 2 iron ingots |
| Heat exchanger | 4 iron plates, 2 rivets, 2 copper pipes |
| Tachometer | 4 iron nuggets, a glass pane, an iron gear, redstone dust |

The changed recipes keep their ids, so a recipe book that knew the old ones shows the new ones.

## The way through

1. Crank or water wheel, wooden shafts, grinder, hand hammer.
2. Firebox, copper boiler, pipes, pump, steam cylinder (6 hand plates) and flywheel: the first engine.
3. Lathe and trip hammer on the engine's shaft. The lathe needs 128 RPM, so step up 2:1 with a large iron
   pulley to a small one (iron pulleys run to 192 RPM). Belts from leather or wool.
4. The first line: ingots → lathe → rods → trip hammer → rivets; ingots → trip hammer → plates → lathe → gears.
5. Gearbox, iron gears, clutch, governor, damper, iron boiler and the rest.
6. The ore line: inline grinder, washer, trommel, bucket elevator, diverter, sawmill.

`cargo test` checks the tree (`src/game/recipe_tree.rs`): everything is reachable from vanilla items and the
machines, and nothing gated can be made without rods, gears or rivets.
