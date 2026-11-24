# Steam and heat: subsystem spec

Spec 2 of the design doc's section 12. Version 1.0 (2026-11-24), scope: **milestone 0.2 (Steam)**.
Builds on `Fire, rust and steam - design.md` v2.1 (sections 4, 7 and 9), `specs/rotational-network.md`
(section 9.3, the interface the engine plugs into) and `spike/FINDINGS.md`. Written together with
`specs/fluids-and-pipes.md` (spec 3) and `specs/multiblocks.md` (spec 4), which cover the rest of 0.2. Where
this spec is more precise than the design doc, this spec wins; section 18 lists what changes in the design doc.

**Written without an interview.** Earlier specs were settled question by question with Marcin. This one was
written in one go on his request, so every decision here is a proposal. Section 19 lists the assumptions and
open questions for review.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 17 collects them, with the recipes.

---

## 1. Scope

**In 0.2:** firebox, the boiler's thermal model (water, temperature, boiling, pressure), steam networks and
their shared pressure, rated pressures and the failure ladder (warning, safety valve, rupture, dry firing),
explosion modes and the steam cloud, burns, the steam engine as a rotational source (cylinder + flywheel, twin
engine), pressure gauge, sight glass, the wrench overlay for all of these, the settings page (explosion mode).

**In the companion specs:** pipes, valves, pump, water flow, tanks as fluid stores and throughput limits
(spec 3); how boilers, engines and tanks form, split and render as one object (spec 4).

**Designed for, built later:** governor and furnace damper (0.3, section 13 reserves their hooks), bellows
(0.3), hot water as feed water, condenser and closed loop (0.3), rust triggers from steam jets and clouds (0.3),
superheating and steel boilers (tier 2).

**Not modelled (design doc 4.1, unchanged):** steam temperature, heat loss to the surroundings, per-block flow.

---

## 2. What the player sees

| Quantity | Shown as | Where |
| --- | --- | --- |
| Pressure | **bar** (gauge pressure, 0 = atmospheric), one decimal | Pressure gauge, wrench overlay, tooltips ("Rated 4 bar") |
| Water | **mB** (1000 mB = 1 bucket) and percent full | Sight glass, wrench overlay |
| Temperature | **°C** | Wrench overlay only |
| Steam flow | **mB/s** (1 mB of water boils into 1 mB of steam) | Wrench overlay |
| Heat | **kW** (a game unit, see 3.1) | Firebox screen, wrench overlay |

Tier-1 pressures run from 0 to about 6 bar. Rated pressures: copper boiler 4 bar, iron boiler 6 bar (section 6).

---

## 3. Firebox

### 3.1 Heat

- A firebox burns one fuel item at a time. Each fuel has a **heat rate** (kW) and burns for its **vanilla burn
  time**, so the heat a fuel gives in total is rate × time.
- **1 kW = 1 kJ/s**, a game unit scaled for playability, not real physics: boiling 1 mB of water takes
  **25 kJ** and warming 1 mB by 1 °C takes **0.1 kJ** (section 4). One coal firebox (1000 kW) boils 40 mB/s,
  about what one engine cylinder uses under load (section 9).

| Fuel | Heat rate | Burns for | Note |
| --- | --- | --- | --- |
| Logs, planks, sticks, wooden items, `#minecraft:logs_that_burn` etc. | 500 kW | vanilla time | Fast and cool |
| Coal, charcoal | 1000 kW | 80 s | Steady and hot |
| Block of coal | 1000 kW | 800 s | |
| Dried kelp block, other vanilla fuels | 750 kW | vanilla time | Default for anything not listed |
| Blaze rod | 2000 kW | 120 s | Very hot |
| Lava bucket | 2000 kW | 1000 s | Very hot; the bucket is given back |

All values *(tune)*. The table is data (`data/fras/fras/firebox_fuels.json`, item or tag → heat rate), so
datapacks can add fuels; burn time always comes from the game's fuel registry. An item with no burn time is
refused.

### 3.2 Behaviour

- **A firebox burns whenever it holds fuel**, like a lit fire. It lights by itself when fuel goes in and goes
  out when the last item has burnt. There is no off switch in 0.2: the player controls heat by how fast fuel
  is fed. (The furnace damper in 0.3 is the automatic control; section 13.)
- **Heat goes to the boilers it touches.** A firebox touches a boiler when it is directly below or beside a
  block of the boiler's bottom layer (spec 4, 3.3). Its heat is split **evenly among the distinct formed boilers it touches**
  (design doc 4.1). A firebox touching no formed boiler burns its fuel for nothing; the overlay says "heating
  nothing".
- A boiler is heated by every firebox that touches it; how many fit depends on its footprint, which is what
  the design doc means by "heating surface".

### 3.3 Block, screen, automation

- A full block: brick body, iron door with a glowing mouth on its front (set by placement). Light level 13
  while burning, smoke and flame particles at the mouth, a low roar while burning.
- **Screen:** like a furnace's with only the fuel slot: the flame shows the burn time left, plus a line
  "Coal: 1000 kW" and "Heating: copper boiler" (or "Heating nothing"). Right-click with anything but the
  wrench opens it.
- **Hoppers and droppers** insert fuel from the top and sides; extraction from below takes only empty buckets
  left by lava. The comparator gives the standard container signal.
- One block entity type, ticking (it is a heat source). Refuses pistons.

---

## 4. Boiler thermal model

A boiler is one formed multiblock (spec 4, section 3). Each boiler holds:

| State | Meaning |
| --- | --- |
| `water` | mB of water, 0 to capacity |
| `temp` | °C of the water and shell together, starting at 20 |
| (steam) | Not held per boiler: a boiler's steam space belongs to its steam network (section 5) |

Per boiler of `n` blocks *(tune)*:

| Value | Per block | 2×2×2 iron boiler |
| --- | --- | --- |
| Water capacity | 1000 mB | 8000 mB |
| Steam space | 100 mB per bar | 800 mB per bar |
| Shell heat capacity | 10 kJ/°C | 80 kJ/°C |

### 4.1 The step

The steam simulation runs **every 4 ticks** (`dt` = 0.2 s) *(tune)*, for every running steam network
(section 5) and the boilers on it. For each boiler, with `H` the heat arriving from its fireboxes this step:

1. **Heat capacity** `M = 0.1 · water + 10 · n` (kJ/°C).
2. **Warming.** If `temp < 100`: `temp += H·dt / M`, capped at 100; the heat left over at the cap goes to
   boiling.
3. **Boiling.** At 100 °C with water present: `boiled = H_left·dt / 25` mB, at most the water there. The water
   becomes the same amount of steam in the network (section 5).
4. **Dry shell.** At 100 °C with no water, heat raises the shell alone: `temp += H_left·dt / (10·n)`
   (section 7.4).
5. **Feed water** arriving this step (spec 3) mixes in: `temp = (M_water·temp + 0.1·fed·t_feed) / (M_water +
   0.1·fed)` with `t_feed` = 20 °C for water (90 °C for hot water in 0.3). Cold feed briefly stops the boil.
   Water entering a dry shell above 100 °C flashes: the excess shell heat boils it at once (a pressure jump,
   the classic way to blow up a dry boiler).

**Boiling point is fixed at 100 °C** whatever the pressure. The water never gets hotter than 100 °C; pressure
is purely how much steam is packed into the network.

### 4.2 Targets *(tune)*

- **Cold start:** a full 2×2×2 iron boiler with two coal fireboxes reaches 100 °C in about **35 s**, then
  1 bar about every 10 s with nothing drawing steam (≈ 4 bar after 75 s in total).
- A full 1×1×4 copper boiler on one coal firebox: about 35 s to boil, then 1 bar about every 10 s.
- One coal firebox boils 40 mB/s; a full 8000 mB boiler lasts 200 s on one firebox with no feed water.

---

## 5. Steam networks and pressure

- A **steam network** is every boiler steam space, steam pipe, steam tank, cylinder inlet, valve and vent
  connected by open pipes (spec 3 defines connections; a closed valve is a boundary). Boilers connect to it
  through their **steam domes**.
- **One pressure per network** (design doc 4.1): `P = steam / C`, where `steam` is the mB of steam held in
  the network and `C` its **steam capacity** in mB per bar: the sum of its boilers' steam spaces (100 per
  boiler block), steam tanks (400 per tank block) and pipes (5 per pipe block) *(tune)*.
- Each step: steam boiled in every boiler is added; steam drawn by cylinders, vents, safety valves and leaks
  is removed; `P` follows. Pressure can't go below 0; a consumer that wants more than is there gets what is
  there.
- **Throughput limits** (pipes, domes, valve openings) cap how fast steam reaches each consumer, not the
  pressure. Spec 3, section 5 defines the routing. A consumer that gets less than it asks for runs **starved**
  (section 9.2). This is how "a thin pipe can't feed a big engine even at high pressure" works.
- **Joining** two steam networks (opening a valve, placing a pipe) pools their steam: the new pressure is
  `(steam₁ + steam₂) / (C₁ + C₂)`. **Splitting** gives each part steam in proportion to its capacity, so both
  keep the pressure they had.
- Several boilers on one network act as one large boiler: the network pressure is shared, each boils with its
  own fireboxes.

---

## 6. Rated pressure

| Part | Rated pressure | Fails as |
| --- | --- | --- |
| Copper boiler | 4 bar | Rupture (7.3) |
| Iron boiler | 6 bar | Rupture |
| Copper pipe | 4 bar | Burst (spec 3, 4.4) |
| Iron pipe | 8 bar | Burst |
| Valves (all four) | 8 bar | Burst |
| Steam tank (iron) | 6 bar | Rupture |
| Steam cylinder | 8 bar | Jam (damaged state, rotational spec 6.4) |
| Steam dome | as its boiler | Part of the boiler |

All *(tune)*. The network's **red line** is the lowest rating of any part on it. The pressure gauge and the
overlay show it, so the player sees which part fails first (the same idea as the rotational red line).

---

## 7. The failure ladder

Design doc 4.3, made exact. An attentive player always gets a chance to react.

### 7.1 Warning

- **From 90% of a part's rating:** steady hiss rising in pitch, wisps of steam at the part's seams.
- **Above its rating:** the part's **warning timer** fills: loud hissing, rattling, the boiler shakes and puffs
  steam, the gauge needle sits in the red. Full after **10 s** *(tune)*: the part fails.
- At **1.25× its rating** or more the timer fills **5× faster** (2 s). At **1.5×** it fails at once.
- Back under its rating the timer **drains at the rate it filled**, as in the rotational spec.
- Only the parts over their own rating fill timers; the weakest goes first, and its failure usually relieves
  the rest.

### 7.2 Safety valve

The **emergency valve** (spec 3, 6.4) is the safety valve. It opens at its set pressure and vents up to
80 mB/s *(tune)*, loudly, wasting steam; nothing breaks. If the boiler makes more steam than the valves on its
network can vent (or the route to them can carry), pressure keeps rising past the valve and the ladder goes
on. Example: one coal firebox (40 mB/s) is held by one valve; four (160 mB/s) need two.

### 7.3 Rupture

When a boiler's or steam tank's timer fills:

- The multiblock **ruptures**: all steam on the part's network is released at once, and an **explosion**
  centred on the multiblock follows, with power `0.25 · P · ∛n` (P in bar at the moment of failure, `n` its
  block count), capped at 4 (TNT) in tier 1 *(tune)*. A 4-block copper boiler at 5 bar gives about 2 (less
  than a creeper), an 8-block iron boiler at 7.5 bar about 3.75.
- What the explosion does depends on the **explosion mode** (7.5).
- A **steam cloud** (7.6) is left where the multiblock was.
- Water in the ruptured boiler is lost.

### 7.4 Dry firing

- A boiler with **no water** that is still being heated has its shell temperature rise above 100 °C
  (4.1 step 4). From 120 °C: it glows dull red, creaks, the sight glass shows empty with a red frame, and the
  overlay says "Dry! Shell 180 °C". At **300 °C (copper) / 400 °C (iron)** *(tune)* it **ruptures** (7.3), with
  power from the network pressure but at least 1.
- With two coal fireboxes on a 2×2×2 iron boiler the shell climbs 25 °C/s: about 12 s from dry to rupture.
- No heat loss in tier 1: a dry, hot boiler whose fire has gone out stays hot until water comes in.
- **Low water** warnings come earlier: the sight glass turns yellow below 25% and red below 10%, with a
  gurgle sound.

### 7.5 Explosion mode (setting)

Server setting on the FRAS settings page (Options > Mods > FRAS), design doc 4.3:

| Mode | Rupture does |
| --- | --- |
| **Full** (default) | A real explosion: terrain damage, entity damage. The multiblock's blocks are destroyed and drop with the vanilla explosion chance. |
| **Contained** | Entity damage and the explosion effect, no terrain damage. Only the multiblock's own blocks are destroyed, dropping with the same chance. |
| **Off** | No explosion. Every block of the multiblock goes into the **damaged state** (stops working, visible cracks), the steam vents as a cloud. Repair as in section 12. |

Burst pipes and valves (spec 3) never explode in any mode.

### 7.6 Steam cloud

- A short-lived scalding cloud: radius `1 + ∛n` blocks around the multiblock, lasting **5 s** *(tune)*, dense
  white particles, a roar of escaping steam.
- Anything inside takes **scald** damage (section 8). In 0.3 the cloud also counts as a rust trigger.

---

## 8. Burns

- A new damage type **`fras:scald`**: 2 damage (1 heart) per second *(tune)*, dealt every 10 ticks while
  exposed. **Fire resistance prevents it** (it is heat); armour doesn't help. Death message: "was scalded".
- **Sources in 0.2:**

  | Source | Area |
  | --- | --- |
  | Open emergency valve, open steam release | 2 blocks out of the outlet face |
  | Burst pipe or valve | 1 block around it |
  | Cylinder exhaust | The block above the cylinder, while the engine runs |
  | Steam cloud | Its radius (7.6) |
  | **Hot steam pipe** | Standing on it or walking into it, like a magma block: 1 damage per second; sneaking prevents it on top, as for magma |

- A steam pipe is "hot" while its network is above 0.2 bar. Boilers, domes, cylinders and tanks don't burn
  on touch (they are lagged). Lagging for pipes comes later (design doc 4.6).
- **Warnings:** an emergency valve **simmers** (wisps, rising whistle) from 0.2 bar below its set pressure before
  it opens; a steam release hisses for half a second before full flow.
- Kid Mode's invulnerability already covers burns and explosions (design doc 4.3).

---

## 9. The steam engine

The engine multiblock (cylinder(s) + flywheel, spec 4 section 4) is one **source** in the rotational network,
plugged into the interface of the rotational spec 9.3. No solver change.

### 9.1 Curve

Per cylinder, with `P` the network pressure at its inlet and `fill` (0 to 1) the share of its steam demand it
actually gets (9.2):

- **Stall torque** `S = 16 Nm per bar · P · fill` *(tune)*.
- **Top speed** `V = 32 RPM per bar · P` *(tune)*.
- The rotational source curve `τ = S · (1 − ω/V)` (rotational spec 3.2). A twin engine adds both cylinders'
  `S`; `V` is the same.
- At 4 bar one cylinder gives 64 Nm at a standstill and nothing at 128 RPM. At 2 bar, 32 Nm and 64 RPM. A
  steam engine can shear a wooden shaft (32 Nm): run an iron trunk from it.
- The engine's `S` and `V` are updated each steam step; the rotational solver uses the latest values every
  tick.

### 9.2 Steam draw

- **Demand** per cylinder: `q · |ω| · P` mB/s, with `q` = 0.16 mB/s per RPM per bar *(tune)*: a cylinder
  takes a fixed volume of steam per stroke, so draw grows with speed and pressure. `ω` is the flywheel's
  average speed over the step.
- Under load the engine slows, so it draws less; idle it runs fast and draws most. A **stalled engine draws
  nothing**, so its steam goes unused and the boiler pressure climbs: this is how an overloaded mechanical
  network overheats the steam side (design doc 4.1).
- **Starving:** if the network can't deliver the demand (throughput, an almost closed valve, an empty
  network), `fill` = delivered / demand and the torque falls with it. Throttling with a manual valve works
  this way.
- **Worked example** *(tune targets, friction ignored)*: one cylinder on a 2-firebox iron boiler (80 mB/s)
  driving eight grinders through a 2:1 gearbox (24 Nm at the engine) settles near 4.8 bar, 105 RPM at the
  engine and 52 RPM at the grinders. Idle, the same engine settles near 4 bar and 128 RPM. An engine runs
  faster than a grinder's 96 RPM limit, so machines hang off it through a gearbox or a large pulley, and an
  engine left idle overspeeds whatever it drives directly: throttle it with a valve, gear it down, or wait
  for the governor (0.3). On one firebox the idle engine settles near 2.8 bar and 90 RPM: an underfired boiler
  sags.

### 9.3 Direction, exhaust, look

- **Direction** is a flywheel setting: the wrench on the flywheel toggles forward / reverse (the reversing
  gear). Both cylinders of a twin follow it.
- **Exhaust** is vented at the top of each cylinder: a puff per stroke (2 per revolution, so the chuff rate
  follows speed, design doc 7.1), scalding the block above (section 8). Piping exhaust to a condenser comes
  with 0.3; in 0.2 the cylinder's top takes no pipe.
- The rod and crosshead animate with the flywheel (spec 4, 6.3). Sound: chuff per stroke, pitch and rate from
  speed; a starved engine wheezes.

### 9.4 Rotational numbers

| Part | Max RPM | Inertia | Friction `c` | Support |
| --- | --- | --- | --- | --- |
| Flywheel | 192 | 200 | 0.05 | yes |
| Steam cylinder | — (not a rotational part) | | | |

The flywheel is the engine's rotational part: its hub axle comes out of both faces (either or both connect),
as the water wheel's does. Spin-up target from the rotational spec 3.4: about 10 s to 90% of settled speed.
Repair: flywheel 4 iron nuggets; cylinder 2 iron nuggets *(tune)*.

---

## 10. Gauges

### 10.1 Pressure gauge

- Attaches to a face of any steam network part (boiler, dome, pipe, valve, tank, cylinder), like the
  tachometer. Reads that network. Doesn't connect or count as a pipe.
- Dial 0 to 8 bar *(tune)*: needle at the network pressure, **red** above the network's red line (section 6),
  **yellow** from 90% of it.
- **Comparator output** 0 to 15, linear from 0 bar to the red line (15 at or over it), as the tachometer.
- On a water pipe or tank it reads 0 (water has no pressure, design doc 4.5).

### 10.2 Sight glass

- Attaches to a side face of a boiler or tank. Shows the fluid level in a vertical glass (the fluid render
  helper from the spike), with the warning colours of 7.4 on a boiler.
- **Comparator output** 0 to 15 proportional to how full it is (0 only when empty), for low-water alarms (with
  a redstone inverter) or a pump clutch.

Neither gauge ticks: the steam step marks them when their reading changes.

---

## 11. Wrench overlay (content, 0.2)

- **Firebox:** fuel, heat rate, time left, which boilers it heats and its share.
- **Boiler:** material, size, water (mB and %), temperature, boiling rate (mB/s), heat in (kW, from how many
  fireboxes), network pressure and red line, warning timer, "Dry! Shell 180 °C", what's missing if not formed
  (spec 4, 7).
- **Steam network** (any part): pressure, red line and which part sets it, capacity, steam in (boiled) and out
  (drawn, vented) in mB/s.
- **Engine** (flywheel or cylinder): stall torque and top speed now, steam demand and fill ("starved: 62%"),
  direction, cylinders found.
- **Valves, pump, tanks, pipes:** spec 3, section 9.
- "Paused: part of this plant is outside the simulated area" when paused (section 14.2).

Tooltips (one line, Shift for details): "Rated 4 bar. Up to 4 blocks." "Burns any fuel. Heats boilers above
or beside it." "Stall 16 Nm per bar, top 32 RPM per bar."

---

## 12. Repair

As the rotational spec 6.6: right-click a damaged part with the wrench, materials from the inventory, free in
creative.

| Part | Repair |
| --- | --- |
| Copper boiler (per damaged block) | 1 copper ingot |
| Iron boiler (per damaged block) | 1 iron plate |
| Steam tank (per damaged block) | 1 iron plate |
| Steam cylinder | 2 iron nuggets |
| Flywheel | 4 iron nuggets |
| Burst pipe, valve | spec 3, 4.4 |

A damaged multiblock (explosion mode "off") works again once every block is repaired; it restarts cold and
empty.

---

## 13. Hooks for 0.3

- **Governor:** sets each engine's throttle `g` (0 to 1) from the network speed, with lag. In 0.2 `g` = 1.
  The demand becomes `q · |ω| · P · g` and the stall torque `S · g`, so throttling saves steam (design doc
  3.4).
- **Furnace damper:** scales a firebox's heat by a factor from 0 to 1 driven by boiler pressure. In 0.2 the
  factor is 1. Fuel burn time scales with it (a damped fire lasts longer).
- **Hot water feed:** `t_feed` in 4.1 step 5 already takes the feed temperature.
- **Exhaust:** the cylinder's top face becomes a pipe outlet for exhaust steam, a separate content from live
  steam so it can't loop back into the boiler (spec 3, 10).
- **Rust:** steam clouds, vents and leaks become rust triggers; a leaking part reuses the burst behaviour.

---

## 14. On Horizon

### 14.1 Ticking

- **Firebox, flywheel and pump tick** (sources and machines). Boiler blocks, domes, pipes, valves, tanks,
  cylinders and gauges **don't**: like shafts, they join their networks from `block_entity_on_load`,
  placement and chunk unload, and the steam step handles them (FINDINGS risk 1).
- The steam step runs in `level_tick_post` every 4 ticks, after the rotational solve of that tick: rebuild
  dirty multiblocks and networks (spec 4, spec 3), check pause status, step boilers (4.1), route and deliver
  steam and water (spec 3, 5), update pressures, timers and failures, then hand each engine its `S` and `V`.

### 14.2 Pausing: plants

- Networks of different kinds are linked through shared parts: an engine joins its steam network and its
  rotational network; a pump its rotational and water networks; a boiler its steam and water networks and
  fireboxes. Everything linked this way is one **plant**.
- **A plant pauses as a whole** if any part of it is not simulated or not loaded (extending design doc 9 and
  rotational spec 10.4). While paused nothing burns, boils, flows, fills a timer or fails, and every value is
  kept. This stops a half-loaded plant from blowing up its boiler while the engine side is frozen.
- A rotational network that touches no steam part pauses alone, exactly as in 0.1.
- The pause check reuses the rotational spec's per-chunk check (one position per chunk, every 10 ticks and on
  chunk load or unload).
- Paused engines keep turning visually; paused vents keep their particles and sound, like paused shafts.

### 14.3 Saving and loading

- Boiler state (water, temperature, warning timer) is saved in each boiler block in proportion (each block
  holds its share), so a boiler keeps its water across saves and when a chunk border splits it on load.
- Network steam is saved per part in proportion to capacity and pooled again on rebuild, so pressure survives
  a reload. As in the rotational spec, the first tick after a load runs before networks exist: parts keep
  what they saved.
- Firebox: fuel slot, current item's heat rate and time left.

### 14.4 Client sync

- `block_entity_update_tag` sends a small fixed set: for a boiler block its formation shape (spec 4) and a
  warning level (0 to 3: none, simmer, warning, critical) for shaking, puffs and sound; for a firebox lit or
  not; for a gauge the reading; for the flywheel the rotational fields plus the cylinder phase.
- Send when pressure changes by more than 0.1 bar or 3%, or any other field changes *(tune)*. Never per tick.

### 14.5 Settings

- The settings page (Options > Mods > FRAS) is new in 0.2, with **Explosion mode** (full / contained / off),
  stored server-side. Rust settings join it in 0.3.

---

## 15. Code layout and tests

- **`sim::steam`**, game-free: `boiler` (the step in 4.1, dry firing), `net` (capacity, pressure, join and
  split, red line), `engine` (curve, demand, fill), `fail` (timers, rupture power, cloud radius), `fuel` (the
  heat-rate table). `sim::fluid` (spec 3) does routing; `sim::multi` (spec 4) formation.
- **Unit tests** at least for: cold start timing to the targets in 4.2; boiling stops while cold feed water
  mixes in; dry shell heating and rupture temperature; flash boiling of water fed onto a hot dry shell;
  pressure from steam and capacity; join pooling and split keeping pressure; red line; warning timer fill,
  5× rate, instant failure, drain; rupture power and its cap; engine curve at several pressures, starving,
  stalled engine draws nothing, twin engine; the worked example in 9.2 within 10%; safety valve holding one
  firebox, failing to hold four; plant pause keeps every value; save and reload keep water, temperature and
  pressure.

---

## 16. 0.2 exit criteria (in game)

For the whole milestone, across the three specs. Driven through the Horizon launcher and its MCP server, with
`/tick rate` raised while waiting for warm-up, pressure and timers.

1. **Cold start:** a 2×2×2 iron boiler, filled by bucket, two coal fireboxes: boils in about 35 s, the gauge
   climbs about 1 bar per 10 s.
2. **First engine:** cylinder + flywheel on that boiler through iron pipe spins up in about 10 s and drives
   grinders; the speed sags under load; idle it overspeeds a grinder unless throttled with the manual valve or
   geared down.
3. **Twin engine** doubles the stall torque (tachometer and overlay).
4. **Water:** a pump driven by the engine keeps the boiler topped up; driven by a water wheel it falls
   behind; on a 1-block puddle it can't keep up at any speed.
5. **Throughput:** a twin engine fed through copper pipe runs starved (overlay); iron pipe fixes it; a big
   boiler with one dome starves its engines, a second dome fixes it.
6. **Ladder:** a copper boiler overfired with no emergency valve hisses, shakes and ruptures; with each
   explosion mode the result matches 7.5. An emergency valve set at 3.5 bar holds one coal firebox; one valve
   can't hold four, two can.
7. **Dry firing:** a boiler left to boil dry with its fire burning glows, creaks and ruptures.
8. **Burst:** copper pipe on an iron boiler bursts above 4 bar, scalds, vents; the wrench repairs it.
9. **Valves:** manual throttle steps change engine speed; a redstone valve on a lever stops and starts an
   engine; a steam release vents with a scalding jet; fire resistance prevents the scald.
10. **Accumulator:** a steam tank on the network makes a sudden load sag the pressure visibly less.
11. **Multiblocks:** seams vanish on formation; the wrench names what's wrong for a non-cuboid boiler, an
    oversized one, a missing dome and a blocked flywheel; breaking a pressurised boiler block vents a puff and
    splits it.
12. **Gauges:** pressure gauge needle, red line and comparator; sight glass level and comparator (a low-water
    lamp).
13. **Pause and reload:** a plant crossing the simulation distance pauses as a whole and nothing fails while
    away; save and reload keep water, temperature, pressure, fuel and formation.
14. **Performance** *(targets)*: 50 boilers, 50 engines and 2,000 pipe blocks cost under 0.2 ms of server
    time per tick on average; a pipe network rebuild of 2,000 blocks under 2 ms.

---

## 17. Starting numbers, recipes and terms *(tune)*

### 17.1 Recipes (0.2)

| Result | Recipe |
| --- | --- |
| Firebox | shaped `BBB` / `BFB` / `BBB` (8 bricks around a furnace) |
| Pressure gauge | shaped `NRN` / `NGN` (copper nuggets, redstone dust, a glass pane), as the tachometer with copper |
| Sight glass | shaped `N` / `G` / `N` (2 copper nuggets, a glass pane) |

Boilers, domes, cylinder, flywheel and tank: spec 4, section 8. Pipes, valves and pump: spec 3, 13.1. The
firebox and gauges are entry recipes from vanilla materials (design doc 6.4).

### 17.2 Terms for the translation dictionary

Proposed Polish names, to be added to `docs/translations/dictionary.md` (all **mod** terms unless noted):
Firebox: Palenisko. Boiler: Kocioł (Miedziany / Żelazny kocioł). Steam dome: Kołpak parowy. Pipe: Rura
(Miedziana / Żelazna rura). Valve: Zawór. Redstone valve: Zawór sterowany czerwonym kamieniem. Steam release:
Upust pary. Emergency valve: Zawór bezpieczeństwa. Pressure gauge: Manometr. Sight glass: Wodowskaz. Pump:
Pompa. Steam cylinder: Cylinder parowy. Flywheel: Koło zamachowe. Storage tank: Zbiornik. Steam: Para; hot
water (0.3): Gorąca woda. Pressure: ciśnienie; rated pressure: ciśnienie dopuszczalne; rupture: rozerwanie;
burst: pęknięcie; scald: poparzenie ("was scalded": "został poparzony parą"). Unit: bar.

### 17.3 Other values

Steam step every 4 ticks. Heat: boiling 25 kJ/mB, warming 0.1 kJ per mB per °C, shell 10 kJ/°C per boiler
block. Boiler block: 1000 mB water, 100 mB/bar steam space. Steam tank block 400 mB/bar; pipe block 5 mB/bar.
Fuel rates: wood 500, default 750, coal 1000, blaze rod and lava 2000 kW. Ratings: copper boiler 4, iron
boiler 6, copper pipe 4, iron pipe and valves 8, steam tank 6, cylinder 8 bar. Size limits: copper boiler 4,
iron boiler 8 blocks. Warning from 90%; timer 10 s, 5× at 1.25×, instant at 1.5×. Emergency valve and steam
release 80 mB/s. Rupture power 0.25 · P · ∛n, cap 4; dry rupture 300 °C copper / 400 °C iron, minimum power 1.
Steam cloud radius 1 + ∛n, 5 s. Scald 2 damage/s; hot pipe 1 damage/s above 0.2 bar. Engine per cylinder:
16 Nm/bar stall, 32 RPM/bar top, demand 0.16 mB/s per RPM per bar. Flywheel max 192 RPM, inertia 200. Sync
threshold 0.1 bar or 3%.

---

## 18. Changes to the design doc

- 4.1: boiling point fixed at 100 °C; one pressure per steam network including the boilers' steam spaces;
  throughput limits cap flow to consumers, not pressure; engine steam draw is proportional to speed and
  pressure (load acts through speed).
- 4.2: the firebox burns whenever it holds fuel and has no off switch in 0.2; fuel duration is the vanilla burn
  time.
- 4.3: rated pressures and the warning timer (10 s, 5× at 1.25×, instant at 1.5×); dry firing is a shell
  temperature that ruptures at a limit; contained mode still damages entities; "off" leaves the steam cloud.
- 4.4: two tier-1 boiler materials (copper 4 blocks / 4 bar, iron 8 blocks / 6 bar).
- 9: a **plant** (networks linked by shared parts) pauses as a whole.
- New: the `fras:scald` damage type; the settings page arrives in 0.2.

---

## 19. Assumptions and open questions

Decisions taken without asking. Each is a proposal; comment on any of them.

1. **Units:** bar, mB, °C, and kW as a game unit (not real physics: real ratios make warm-up take half an
   hour). Alternative: hide heat numbers and show only "weak / steady / hot" fuel.
2. **Fixed 100 °C boiling point.** Real boilers boil hotter at higher pressure; modelling it would slow
   pressure build-up and add a number nobody reads. Revisit with superheating in tier 2?
3. **Two boiler materials** in tier 1: entry copper (4 blocks, 4 bar, hand-crafted from ingots) and iron
   (8 blocks, 6 bar, from hand-hammered plates). The design doc names a "small copper boiler" and "bigger
   boilers" from plates; this is my reading. The iron boiler will rust in 0.3.
4. **The firebox has no off switch** (no redstone control) so that the damper in 0.3 matters and the safety
   valve is needed. If that feels too harsh in play, a redstone-closed air door is easy to add.
5. **Engine draw ∝ speed × pressure,** not "speed and load" literally. It reproduces everything the design doc
   asks for (idle wastes steam, governor saves it, a stall raises pressure). A side effect: the worked
   example's 4.8 bar under normal load would be over a copper boiler's 4 bar rating with the same two
   fireboxes, so the emergency valve is essential from the first engine. Intended, or should loaded running
   sit safely under the rating?
6. **Top speed scales with pressure** (32 RPM per bar). At 6 bar an iron boiler's idle engine reaches 192 RPM,
   the flywheel's limit. Should the top speed be capped at 128 regardless?
7. **Dry firing ruptures** (design doc 4.1 says it is a failure, and 4.3 makes failures ruptures). A gentler
   alternative is a burnt-out (damaged) boiler with no explosion.
8. **Explosion modes:** contained still hurts entities; "off" still leaves the scalding steam cloud. Destroyed
   blocks drop with the vanilla explosion chance in full and contained modes.
9. **Burns:** 1 heart per second for jets, clouds and bursts, half that for touching a hot pipe; fire
   resistance protects. Boilers, domes and cylinders don't burn on touch.
10. **Plant-wide pause** (section 14.2) is new and is the safe reading of "nothing explodes while nobody is
    around". It means a long line shaft from an engine pauses the boiler too if its far end leaves the
    simulated area.
11. **Exhaust can't be piped in 0.2.** It vents at the cylinder and scalds the block above it.
12. **Single-cylinder engines self-start** from any position (no dead centre) and run as smoothly as twins in
    the simulation; the twin only doubles torque (smoothness is visual and sound only).
