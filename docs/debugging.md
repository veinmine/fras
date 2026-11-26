# Debug commands

`/fras …` commands (operator level 2) for testing in a game, made so an agent can drive a test from the
server console or `horizon-launcher` and read the results back as JSON. Every command answers in chat;
the JSON ones also print the JSON in feedback lines (split at 3,800 bytes) and write it to
`fras-debug/<name>.json` in the game directory, which is the easier copy to read.

Positions are block positions (`~ ~-1 ~` works). Commands act in the dimension they run in.

## Looking at state

| Command | Does |
| --- | --- |
| `/fras dump <pos> [what]` | JSON for the block at `pos`. `what` is `part` (the rotational part: speed, torque, jam, warnings), `network` (the whole network: every part's position, kind and speed), `line` (a belt line: blocks, drives, items with their offsets, stop reason, speed), `machine` (inventory, work done, status, tank, belt job), `plant` (whether a steam plant holds it), or `all` (the default). Written to `fras-debug/dump.json`. |
| `/fras counter add <pos>` | Counts items that pass the centre of the belt block at `pos`. `remove`, `list` (totals and items per minute over the last minute), `clear`. |
| `/fras events [count] [kind]` | The last `count` (default 20) events from the log, newest last, optionally only one kind. `/fras events clear` empties it. The log keeps the last 4,000. Written to `fras-debug/events.json`. |
| `/fras perf` | Mean and worst time per tick of the mod's hooks since the last reset (`level_tick`, `belts`, `steam`, `rot`). `/fras perf reset` starts again. |

Event kinds: `jam` (a part or line jammed, with the reason), `overspeed` (a warning started), `made`
(a machine finished an item; `item`, `machine`), `delivered` (a belt end handed an item on), `drop` (an
item fell off a belt end or diverter into the world), `backup` (a belt end has nowhere to deliver),
`line_jam`, `under_driven` (a line has fewer drives than it needs), `counted` (a counter saw an item),
`fuel` (a firebox took an item).

## Setting up

`/fras scenario <name>` builds a test rig next to the player (clearing the space first) and says what
it built and where:

- `ore_line`: the 0.4 reference ore line (machines spec 10): a chest with raw iron and copper and a hopper
  feeding a belt → grinder → belt with a washer above it in belt mode (water by copper pipe from a creative
  source) → trommel → chests for fines, mediums and oversize. Creative engines at 64 RPM drive the belts (reversed, so
  the east-heading belts run east) and machines. Expect dusts in the fines chest.
- `test_engine`: one flywheel and steam cylinder on 4 bar creative steam, an iron shaft through a tachometer
  to a creative load bank at 32 Nm. Expect about 60 RPM (64 on the load alone, less about 2 Nm of drivetrain
  friction).

## Fast-forward

| Command | Does |
| --- | --- |
| `/fras run <ticks>` | Takes a snapshot of the counters, runs `tick sprint <ticks>`, and when that's done writes an outcome report. Operators get a chat line "run of N ticks done". |
| `/fras report` | The last report: `jams`, `overspeeds`, `drops`, `backups`, `fuel_items_used`, all event counts, and per dimension `items_made`, `made_by_machine`, `delivered`, `dropped`, `diverted`, `counted` (per counter), `items_on_belts`, `jammed_parts_now`, plus `wall_seconds`. Written to `fras-debug/report.json`. |

A typical agent test: `/fras scenario ore_line`, `/fras counter add <a belt block>`, `/fras run 6000`,
wait for the chat line, read `fras-debug/report.json`, then `/fras events 50 jam` if anything jammed.

## Stepping

| Command | Does |
| --- | --- |
| `/fras rust set <pos> <stage>` | Sets an iron part's rust: `clean`, `spotty`, `rusty`, `leaking`, `painted`. |
| `/fras rust tick <pos> <times>` | Runs `times` rust random ticks on it at once (weather and paint apply as usual). |
| `/fras steam pause` | Stops the steam plants' step in every level (pressure, heat, water freeze; the rotational network keeps running). `/fras steam resume` starts them again. |
| `/fras steam step [steps]` | While paused, runs that many plant steps (one step is 4 ticks). |

The tick commands still help: `/tick rate 100`, `/tick freeze`, `/tick step 20`.
