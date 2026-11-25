# Regulation: subsystem spec

Spec 2 of the design doc's section 12, second part. Version 1.0 (2026-11-25), scope: **milestone 0.3
(Regulation and cooling)**. Builds on `Fire, rust and steam - design.md` v2.1 (sections 3.4, 4.2 and 4.3),
`specs/steam-and-heat.md` (spec 2, the 0.2 part, whose section 13 reserved the hooks used here),
`specs/fluids-and-pipes.md` (spec 3, routing), `specs/rotational-network.md` (section 9.3, the interface the
governor plugs into) and what 0.2 shipped (`docs/development.md`, "Where 0.2 differs from the spec"). Written
together with `specs/cooling.md` and `specs/rust.md`, which cover the rest of 0.3. Where this spec is more
precise than the design doc, this spec wins; section 13 lists what changes in the design doc.

**Not yet reviewed.** Written in one go without an interview, like the 0.2 specs. Section 14 lists the open
questions with the assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 12 collects them, with the
recipes. This spec also holds the **0.3 exit criteria** for the whole milestone (section 11).

---

## 1. Scope

**In 0.3:** the centrifugal **governor** (speed control of engines, with lag and hunting), the **furnace damper**
(pressure control of fireboxes), the **bellows** (shaft-driven air for a firebox), their wrench settings,
overlay lines, looks and sounds.

**In the companion specs:** exhaust steam, the heat exchanger and radiators, hot water, the drain and machine
cooling (`specs/cooling.md`); rust, paint and scraping (`specs/rust.md`).

**Designed for, built later:** a governor driving a redstone output (tier 4 control), a damper on a liquid-fuel
burner (tier 2), superheating (tier 2).

---

## 2. What the player gets

0.2 left the player with two manual problems: an engine that runs away when its load drops, and a boiler that
climbs to the safety valve when steam goes unused. 0.3 gives each an automatic regulator, and each regulator
moves the problem somewhere else, as design doc 3.4 asks:

| Problem | Regulator | What it moves |
| --- | --- | --- |
| Speed swings with load; an idle engine overspeeds | **Governor** throttles the engine's steam | Steam goes unused, so boiler pressure climbs. Too sensitive, it hunts. |
| Pressure climbs when steam is unused | **Furnace damper** turns the fire down | Nothing new: this closes the loop. Too low a setting starves the engines. |
| A firebox gives only so much heat | **Bellows** blows air into it | Costs shaft power; more heat can overfire the boiler. |

A plant with a governor, a damper and an emergency valve as a backstop runs itself, idle or loaded. That is
the "well-tuned factory runs itself" of pillar 2.

---

## 3. Governor

### 3.1 Block and placement

- The governor is a **steam pipe part with a shaft socket**: an iron valve body on a pipe axis, like the
  manual valve (spec 3, 6.1), with a vertical spindle carrying two flyballs. Pipes connect on its two axis
  faces; the **shaft socket** is on one of the four faces across the axis (top by default, set by placement
  facing an adjacent shaft as for machines). The wrench doesn't rotate it, since its right-click sets the
  sensitivity (3.4): to move the socket, break and replace the governor.
- It is a **machine end point** in the rotational network (rotational spec 9.3): working range 0 to 192 RPM,
  load 1 Nm while turning, inertia 2, iron friction *(tune)*. Over 192 RPM it overspeeds and jams like any
  part. Direction doesn't matter.
- As a pipe part it is iron-bodied: 200 mB/s, rated 8 bar, steam only (it refuses to connect to a water, hot
  water or exhaust network; the overlay says why). It rusts unless painted (`specs/rust.md`).
- The player puts it **in the steam line feeding the engine(s)** and drives its socket from the engine's
  shaft: directly against the flywheel's hub axle, through a shaft, or by a belt. That is the real layout: a
  belt-driven flyball governor working the throttle in the steam pipe.

### 3.2 What it throttles

- The governor has a **throttle** `g` from 0 (closed) to 1 (open). It does not cut the pipe's throughput; it
  scales the **steam demand** of every consumer downstream of it on the routing tree (spec 3, section 5):
  each cylinder's demand becomes `q · |ω| · P · g` and its stall torque `S · g`, the hook reserved in spec 2,
  13. A consumer behind two governors gets the product of both.
- Steam saved by throttling stays in the boiler, so **pressure climbs** (design doc 3.4, point 2). The damper
  (section 4) is the answer.
- Everything downstream is throttled, vents too. An emergency valve or steam release placed **behind** a
  governor can't vent while the governor is closed; the governor's overlay warns ("emergency valve behind
  the governor: it can't vent when the throttle closes"). Put safety valves on the boiler side.

### 3.3 The control law: flyballs with lag

The governor senses the speed `s = |ω|` at its own socket (after any gearing between it and the engine).

1. **Flyballs.** The ball position `x` (in RPM, the speed the balls "show") follows `s` as a lightly damped
   second-order system, integrated every tick:
   `x'' = ωn² · (s − x) − 2 · ζ · ωn · x'`, with `ωn` = 1 rad/s and `ζ` = 0.2 *(tune)*. The balls take a few
   seconds to settle at a new speed and overshoot a little. This lag is what makes a governor hunt
   (Maxwell's analysis of 1868); a plain first-order lag doesn't, because the engine curve (spec 2, 9.1) is
   already self-steadying.
2. **Throttle.** Each steam step: `g = clamp((target − x) / band, 0, 1)`. Below `target − band` the throttle is
   wide open; at `target` and above it is shut. Between, it is proportional.
3. **Droop.** Under load the engine settles where the throttle gives enough torque, somewhere between
   `target − band` and `target`, never above. A wide band droops more under load; a narrow band holds the
   target tighter but hunts.

### 3.4 Settings

| Setting | Values | Default |
| --- | --- | --- |
| **Target** | 8 to 192 RPM in steps of 8, at the governor's socket | 64 RPM |
| **Sensitivity** | 1 to 5: band 32, 16, 8, 4, 2 RPM | 1 (band 32: steady, droops) |

- **No screen** (the wrench never opens one, as on every other FRAS part). The settings are clicks, like the
  manual valve and the emergency valve:
  - **Target:** right-click with an empty hand raises it 8 RPM, sneak-right-click lowers it 8 RPM (stopping at 8
    and 192), with the manual valve's squeak.
  - **Sensitivity:** wrench right-click cycles it 1 → 2 → 3 → 4 → 5 → 1, as the wrench cycles the emergency
    valve's set pressure.
  - Each click shows the new value on the action bar ("Target 72 RPM", "Sensitivity 3: band 8 RPM").
    Sneak-right-click with the wrench inspects as everywhere, with the speed, ball position and throttle.
- The default is the steady, sloppy setting, so a first governor never surprises; tightening it is the tuning
  game.

### 3.5 Tuning targets *(tune)*

A simulation of the 0.2 engine (one cylinder at 4 bar, 16 Nm of load at the engine, target 90 RPM, the
flywheel's inertia of 90 plus ~10 for the rest of the line) with the law in 3.3 gives:

| Sensitivity (band) | One flywheel | Two flywheels | Three flywheels |
| --- | --- | --- | --- |
| 1 (32 RPM) | steady at 71 RPM | steady at 71 | steady at 71 |
| 2 (16 RPM) | **hunts** 72 to 82 | steady at 79 | steady at 79 |
| 3 (8 RPM) | **hunts** 74 to 84 | hunts 82 to 85 | steady at 84 |
| 4 (4 RPM) | **hunts** 75 to 86, throttle slamming 0 to 100% | hunts 84 to 87 | hunts 85 to 87 |
| 5 (2 RPM) | **hunts** 76 to 87 | hunts 84 to 88 | hunts 86 to 88 |

When the load drops to nothing, sensitivity 4 or 5 on one flywheel overshoots to about 98 RPM before it settles
(a grinder's limit is 96), with two flywheels to about 94, with three to about 92. `tools/governor_sim.py`
reproduces the table. These are the shapes the unit tests pin down
(section 10): sloppy and steady at sensitivity 1; hunting with one flywheel from 2; extra flywheels (a bare
flywheel anywhere on the line, spec 4, 4.1) calm it. The ball period, about 6 s, is slow enough to see on the
tachometer and hear in the chuff.

### 3.6 Engines that fight, and the shared governor

These fall out of 3.2 and need no extra rule; the overlay explains them:

- **Two governors with different targets** on two engines driving one shaft: above the lower target, its
  governor shuts and that engine only drags; the other does all the work (design doc 3.4, point 3).
- **Two governors with the same target and band** share the load roughly evenly.
- **One governor on a shared steam line** feeding several engines throttles them all alike: the clean way to
  run a twin bank.

### 3.7 When the governor stops turning

The balls fall when their spindle stops. A governor whose socket isn't turning (the drive belt slips, a shaft
shears, a clutch opens, the governor jams) sees `s` = 0 and **opens wide**, like a real governor whose belt
broke: the engine runs away to its unthrottled speed. The overlay and the governor's look warn ("not turning:
throttle wide open"), and the balls visibly drop. This is planning, not wear: drive the governor from the
engine's side of any clutch.

### 3.8 Look and sound

- The spindle turns with the socket (`spin` + `keep`, like shafts). The ball arms' angle follows `x`,
  quantised to 8 positions and synced when it changes (14.4 of spec 2: never per tick).
- A small lever on the valve body shows the throttle.
- Sound: a soft whirr whose pitch follows the spindle; a throttle hiss that swells as it opens. A hunting
  governor is heard as the chuff rising and falling every few seconds.

---

## 4. Furnace damper

### 4.1 Block and placement

- A flap with a pressure diaphragm that **attaches to any outer face of a boiler block**, like a gauge (spec 2,
  10). It reads that boiler's steam network pressure (a sealed boiler's too) and scales the heat **every
  firebox gives that boiler**. Iron, so it rusts unless painted.
- One damper per boiler is enough. With several on one boiler, the most closed one counts.
- Not a pipe part and not an end point; it doesn't connect to anything.

### 4.2 Control

- **Opening target** `h* = clamp((set − P) / 1 bar, 0, 1)`: fully open 1 bar below its set pressure, shut at it,
  linear between *(tune)*. The flap moves toward `h*` at most 25% per second (4 s from open to shut).
- **Redstone:** a powered damper shuts (`h*` = 0), whatever the pressure. This is the "redstone-closed air door"
  that decision S4 kept in reserve, so a lever can bank a fire.
- **Set pressure:** wrench right-click cycles it in 0.5 bar steps from 1.0 to 8.0 bar, like the emergency
  valve. Placed, it takes the red line of the boiler's network minus 1.0 bar, one step below the emergency
  valve's default (spec 3, 6.4), so in a plant left at defaults the damper turns the fire down before the
  safety valve pops.

### 4.3 Effect on fireboxes

- A firebox's heat into a boiler is multiplied by that boiler's damper opening `h` (1 with no damper): the
  hook reserved in spec 2, 13.
- **Fuel burns slower when damped:** the firebox's burn timer runs at the mean of `h` over the boilers it
  heats. A firebox heating one boiler whose damper is half shut gives half the heat and its coal lasts twice
  as long. A firebox heating nothing is not damped.
- **Banked fire:** at `h` = 0 the fire holds but burns nothing; the mouth glows dim (light 7 instead of 13) and
  the roar drops to a murmur. It flares back as soon as the damper opens.

### 4.4 Steady state

With a damper, a boiler settles at the pressure where heat in equals steam out, somewhere in the damper's
1 bar band below its set pressure, and **fuel use follows steam use**. An idle governed plant banks its fire
and nearly stops burning fuel; a loaded one opens up. This is what makes fuel savings measurable (the closed
loop of `specs/cooling.md`): without a damper a firebox burns at full rate whatever happens to its steam.

### 4.5 Look and sound

A hinged flap on a small riveted box, its angle following `h` (4 positions in the block state); a creak when it
moves. The firebox's own glow and roar follow `h` (4.3).

---

## 5. Bellows

### 5.1 Block and placement

- A shaft-driven pair of leather bellows on a wooden frame: **nozzle** on one face, **shaft socket** on the
  opposite face. Placed against a firebox, the nozzle faces it (any firebox face but its mouth). Wood and
  leather: it never rusts.
- A machine end point: working range 16 to 128 RPM, load 6 Nm while blowing, inertia 4, wooden friction
  *(tune)*. It blows (and pulls its load) only while its firebox is burning and not banked; otherwise it idles.
  Above 128 RPM it overspeeds and jams.
- One bellows per firebox counts; a second does nothing (the overlay says so).

### 5.2 Effect

- **Boost** `b = 0.5 · min(1, ω / 64)` inside its working range, 0 below 16 RPM *(tune)*.
- The firebox's heat rate is multiplied by `1 + b`; its **burn time is unchanged**, so the same fuel gives up to
  **50% more heat**: "more heat from the same fuel" (design doc 4.2). A coal firebox with bellows at 64 RPM
  gives 1500 kW and boils 60 mB/s instead of 40.
- With a damper: heat = rate × (1 + b) × `h`, burn timer × `h`.
- The engine feeds its own fire: more heat, more pressure, more speed, more air, up to the 64 RPM saturation.
  The danger is overfiring a boiler sized for plain fireboxes; the ladder of spec 2, 7 still applies.

### 5.3 Look and sound

The bellows pump with the shaft (a rocking animation whose rate follows speed: `spin` with a small angle range
if supported, as the 0.2 rod fallback; otherwise drawn per frame like the rod). A whoosh per stroke; the
firebox roars louder and throws more sparks while blown.

---

## 6. Wrench overlay and tooltips

- **Governor:** target, sensitivity (band), speed at the socket, ball position, throttle %, which engines it
  throttles (count), warnings ("not turning: throttle wide open", "emergency valve behind the governor",
  "hunting: speed swinging ±5 RPM" when the last 10 s swung more than half the band).
- **Damper:** set pressure, network pressure, opening %, "banked", "shut by redstone".
- **Bellows:** speed and working range, boost %, "firebox not burning", "a second bellows does nothing".
- **Firebox** (added to spec 2, 11): heat now including boost and damper ("Coal: 1000 kW × 1.5 bellows × 40%
  damper = 600 kW"), burn rate.
- **Engine** (added): throttle from governors, "throttled to 35%".
- Tooltips: "Throttles the steam to the engines downstream. Hand clicks set the target, the wrench the sensitivity."
  "Turns the fire down as pressure nears its setting. Redstone shuts it." "Blows a firebox: up to 50% more
  heat from the same fuel."

---

## 7. Repair

As spec 2, 12: wrench right-click on a jammed part, materials from the inventory.

| Part | Repair |
| --- | --- |
| Governor | 2 iron nuggets |
| Bellows | 1 leather |
| Damper | 1 iron nugget (only when rusted through, `specs/rust.md`) |

---

## 8. Saving and syncing

- **Governor:** target, sensitivity, ball position and speed, throttle, socket face; plus the rotational fields.
  Ball state is saved so a reload doesn't kick a running plant.
- **Damper:** set pressure, opening. **Bellows:** socket face plus the rotational fields.
- Client sync (spec 2, 14.4): the governor's ball angle step (0 to 7) and throttle lever (0 to 3); the damper's
  flap step; the bellows as a rotational part. Sent only when a step changes.
- All three join the plant (spec 2, 14.2): the governor links its rotational network to its steam network, the
  damper belongs to its boiler, the bellows links its rotational network to its firebox's boilers. A plant
  pauses as a whole, as in 0.2.

---

## 9. On Horizon

- **The governor and bellows tick** (machines). The damper doesn't: the steam step updates it from its boiler,
  like a gauge (0.2 shipped gauges as ticking; the damper may follow that if simpler).
- Order in `level_tick_post`: rotational solve → governors integrate their balls (every tick) → every 4 ticks
  the steam step reads each governor's `g`, each damper's `h` and each bellows' `b`, then steps boilers and
  routes (spec 2, 14.1).
- The governor's settings are block-entity fields changed by the same use-item / use-without-item hooks as the
  manual and emergency valves (`pipes.rs`); no menu.
- Routing (spec 3, 5) already builds a tree from the sources; each consumer gets a throttle factor, the product
  of the `g` of governors on its path, computed at rebuild (the path) and each step (the product).

---

## 10. Code layout and tests

- **`sim::steam::govern`**, game-free: the ball integration, `g` from target and band, the throttle product on
  routes. **`sim::steam::damper`**: `h*`, travel rate, redstone, the firebox's heat and burn factor.
  **`sim::steam::bellows`**: boost from speed.
- **Unit tests** at least for: the table in 3.5 within ±2 RPM (hunting or steady, and the swing) with the 0.2
  engine and solver; overshoot on a load drop, smaller with a second flywheel; droop under load at sensitivity
  1; an idle governed engine draws a small fraction of its unthrottled steam; two governors with different
  targets: one does all the work; a shared governor throttles both cylinders; a governor not turning opens
  wide; a vent behind a governor is throttled; damper opening from pressure, travel rate and redstone; a banked
  firebox's timer stops; burn rate is the mean over two boilers; bellows boost at 0, 16, 32, 64 and 100 RPM and
  burn time unchanged; save and reload keep ball state, settings and openings.

---

## 11. 0.3 exit criteria (in game)

For the whole milestone, across the three specs. Driven through the Horizon launcher and its MCP server, with
`/tick rate` raised while waiting, `/tp` between test sites, and `/gamerule randomTickSpeed` raised for rust.

**Regulation (hunting and tuning playable):**

1. **Governor holds speed:** an engine driving grinders through a governor at target 64 drops its load (clutch
   out); the speed stays at or under the target; the overlay shows the throttle closing and steam demand
   falling; boiler pressure climbs.
2. **Hunting:** at sensitivity 4 with one flywheel, the tachometer needle swings several RPM every few seconds
   and the chuff rises and falls; a second flywheel on the line, or sensitivity 1, steadies it; sensitivity 1
   droops visibly under load.
3. **Overshoot:** with a hunting setting set just under a grinder's 96 RPM limit, dropping the load overspeeds
   the grinder into its warning; a second flywheel or a lower target prevents it.
4. **Fight and share:** two governed engines on one shaft with targets 64 and 72: the 64 engine idles (overlay);
   one governor on their shared steam line shares the load.
5. **Belt broken:** cutting the governor's drive (clutch out between engine and governor) opens it wide and the
   engine runs away into overspeed warnings.
6. **Damper:** an idle governed plant's pressure settles in the damper's band, the firebox banks (dim mouth,
   fuel timer stopped) and the emergency valve stays shut; loading the engine opens the damper; a lever on the
   damper banks the fire.
7. **Bellows:** a coal firebox with bellows at 64 RPM or more shows 1500 kW and its coal lasts the same 80 s.

**Cooling (closed loop saves fuel):**

8. **Closed loop:** a governed, dampered engine running a fixed load for 10 game minutes burns **at least 15%
   less coal** with its exhaust piped through heat exchangers back to the boiler as hot water than with the
   exhaust vented and a pump feeding cold water (the model gives 21%, `specs/cooling.md` 3.2). With the loop the
   cylinder stops puffing and scalding, and the pump stays idle except for make-up water.
9. **Undersized condenser:** with too little cooling the remainder vents at the cylinder (puff, scald) and the
   overlay names the short exchanger; adding radiators or waterlogging the exchanger fixes it.
10. **Machine cooling:** a cooled grinder runs at 120 RPM without a warning; cutting its water starts its
    overspeed warning; its hot water feeds the boiler.
11. **Drain and backing up:** with the boiler full and nowhere for hot water to go, exchangers stop and exhaust
    vents at the cylinder; a drain on the hot water line takes the surplus with steam puffs.

**Rust (stages and leaks work):**

12. **Triggers:** iron pipes in water, waterlogged, under open sky in rain and in a steam release's jet rust
    through the stages; the same pipe under a roof, or carrying water through a dry room, never rusts; copper
    never rusts.
13. **Leaks:** a leaking steam pipe vents, scalds and lowers the pressure; a leaking water pipe drips and
    delivers less; a leaking iron shaft jams ("rusted through").
14. **Paint and scraping:** a painted pipe in water stays clean; paint is refused on a rusty part; the wrench
    scrapes a part clean and drops flakes, a leaking one only with an iron plate; flakes → grinder → pigment →
    paint; a pre-painted pipe from the crafting grid is protected when placed.
15. **Settings:** rust off stops all rusting; rust speed 4× is visibly faster.

**All of 0.3:**

16. **Pause and reload:** a plant crossing the simulation distance pauses as a whole; save and reload keep
    governor and damper settings and state, hot water in tanks, rust stages and paint.
17. **Performance** *(targets)*: the 0.2 benchmark plant (50 boilers, 50 engines, 2,000 pipe blocks) plus 50
    governors, 50 dampers, 50 bellows and 50 heat exchangers with radiators costs under 0.6 ms of server time
    per tick on average (0.2 measured 0.46 ms without them); 2,000 rusting iron parts add under 0.02 ms per
    tick.

---

## 12. Starting numbers, recipes and terms *(tune)*

### 12.1 Numbers

Governor: 0 to 192 RPM, load 1 Nm, inertia 2; balls `ωn` 1 rad/s, `ζ` 0.2; target 8 to 192 in 8s (default 64);
sensitivity bands 32, 16, 8, 4, 2 RPM (default 32); valve body 200 mB/s, 8 bar. Damper: band 1 bar below its
set pressure, travel 25%/s, set 1.0 to 8.0 bar in 0.5s, default red line − 1.0. Banked firebox light 7.
Bellows: 16 to 128 RPM, load 6 Nm, inertia 4, boost 0.5 × min(1, ω/64). Repairs: governor 2 iron nuggets,
bellows 1 leather, damper 1 iron nugget.

### 12.2 Recipes (0.3)

| Result | Recipe |
| --- | --- |
| Governor | shaped `I I` / ` S ` / `PVP` (2 iron ingots as the balls, an iron shaft, 2 iron plates, a manual valve) |
| Furnace damper | shaped `PPP` / `PRP` (5 iron plates, a redstone dust) |
| Bellows | shaped `WWW` / `LLL` / `WSW` (planks, 3 leather, a wooden shaft) |

The governor and damper need hand-hammered plates, as the iron boiler does; the bellows is an entry recipe.
Each unlocks on picking up an ingredient. Intermediate parts (gears, rods, rivets) arrive with the lathe in
0.4; the recipes may move to them then.

### 12.3 Terms for the translation dictionary

Proposed Polish names (**mod** terms): Governor: Regulator odśrodkowy. Target speed: prędkość zadana.
Sensitivity: czułość. Throttle: przepustnica. Hunting: kołysanie (regulatora). Furnace damper: Szyber. Banked
fire: przytłumiony ogień. Bellows: Miech.

---

## 13. Changes to the design doc

- 3.4: the governor is a steam pipe part with a shaft socket, placed in the engine's steam line and driven from
  its shaft; it throttles everything downstream; settings are a target (8 to 192 RPM) and a sensitivity (1 to
  5); hunting comes from flyball lag; a governor that stops turning opens wide.
- 4.2: the damper attaches to a boiler and scales every firebox heating it; it shuts on a redstone signal; a
  shut damper banks the fire (burns nothing); the bellows gives up to 50% more heat with burn time unchanged.

---

## 14. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

1. **G1. The governor is a pipe part in the steam line** with a shaft socket (3.1), throttling everything
   downstream. *Alternative:* a block placed on the engine (on the cylinder or the flywheel) that throttles only
   that engine. The pipe version gives the shared governor for free and matches the real belt-driven layout.
2. **G2. Hunting from flyball lag** (3.3, 3.5). A faithful first-order lag doesn't hunt with the 0.2 engine
   curve, so the balls get second-order dynamics. Hunting starts at sensitivity 2 with one flywheel; the
   default (1) is steady but droops. OK, or should the default be a middle setting that hunts a little?
3. **G3. A governor that stops turning opens wide** (3.7), as a real one with a broken belt. *Alternative:* it
   shuts (fail safe), which removes a classic hazard but also one way to learn.
4. **G4. Decided (Marcin, 2026-11-25): no screen,** because the wrench doesn't open screens on any other
   machine. Empty-hand clicks raise and lower the target, wrench clicks cycle the sensitivity (3.4); the
   socket is set by placement only.
5. **G5. Governors throttle vents behind them too** (3.2), with an overlay warning. *Alternative:* only cylinders
   are throttled.
6. **G6. The damper attaches to the boiler** and scales all its fireboxes (4.1). *Alternative:* one damper per
   firebox, finer but tedious on a lying boiler with many fireboxes.
7. **G7. A shut damper banks the fire to zero** (4.3). *Alternative:* a minimum of 10%, so a banked boiler still
   slowly climbs to its safety valve.
8. **G8. The damper takes redstone** (shut while powered), so a gauge comparator or a lever can bank a fire.
9. **G9. The bellows gives free efficiency:** up to +50% heat with burn time unchanged (5.2), paid for in shaft
   power (6 Nm). *Alternative:* faster burning for more heat with the same total, which makes it a power boost
   rather than a fuel saver.
