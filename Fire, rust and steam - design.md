# Fire, Rust and Steam
## A Horizon native technology mod

Design document, version 2.1 (2026-11-23). Supersedes the first draft, refined in a design interview;
2.1 adds what the technical spike found (`spike/FINDINGS.md`).
Numbers marked *(tune)* are starting
values for balancing, not commitments.

---

## 1. What it is

Fire, Rust and Steam (FRAS, mod id `fras`) is a Minecraft technology and automation mod for
[Horizon](https://playhorizon.dev). It provides power generation, ore processing, logistics and
machine fabrication, with progression across tiers: from a hand crank and a water wheel, through
steam engines and line shafts, to steel, electricity and automated control.

### Pillars

1. **Be the engineer.** You read gauges, listen to the machinery and tune the system. Speed sags
   under load, pressure climbs when steam goes unused, and the controls that fix one problem create
   the next one.
2. **Failure comes from bad planning, never from normal use.** There is no wear and no routine
   maintenance. Machines run too fast and break, iron left in the rain rusts and leaks, and boilers
   left unwatched over-pressure. A well-tuned factory runs itself.
3. **Everything is visible and audible.** Shafts turn, cylinders pump, belts move, steam hisses.
   An experienced player can hear trouble before seeing it.
4. **The factory makes the factory.** Higher tiers are built from parts that earlier machines
   produce, on production lines, not at a crafting table.
5. **Danger grows by tier.** Tier 1 bites a little; later tiers bite harder. Every danger warns
   before it strikes.

---

## 2. Tiers

| Tier | Theme | Highlights |
| --- | --- | --- |
| 1 | **Steam and shafts** | Crank, water wheel, low-pressure boilers, steam cylinder + flywheel, line shafts, belts, grinder/washer/trip hammer/lathe. Vanilla copper and iron only. |
| 2 | **High pressure and steel** | Coke oven, blast furnace, steel, high-pressure steel boilers, superheating, bigger machine tools, assembly table, maybe hydraulics, maybe bronze/brass (new ores). |
| 3 | **Electricity** | Dynamo driven by steam engines (later a steam turbine), wires, electric motors, electromagnets, magnetic separation, filter sorter. Builds on the shaft world rather than replacing it. |
| 4 | **Control** | Relays, punched-card controllers, simple computers that read gauges and drive valves and clutches. |
| Later | Open | Possibly far beyond: nuclear power, particle accelerators, teleporters. Future work. |

**Current focus: tier 1.** Tiers 2 to 4 are an outline only. Each subsystem gets its own detailed
spec before it is implemented (section 12).

---

## 3. Rotational power

### 3.1 Model: push versus pull

- A **shaft network** is every shaft, gear and machine connected mechanically. Everything on it
  shares one **speed** (RPM, signed for rotation direction). Gearboxes and pulleys link
  sub-networks by a fixed ratio, so a gear-linked system is solved as one.
- **Power = speed × torque.**
- **Sources push.** Each source provides a torque that depends on its input (steam pressure, water
  flow, a player cranking) and **falls as speed rises**, reaching zero at its top speed.
- **Consumers pull.** A working machine draws a load torque; an idle machine draws almost none.
  Belts draw a little per item carried.
- The network settles at the speed where **total push = total pull**. Idle: it climbs toward the
  sources' top speed. Loaded: it sags. Overloaded: it crawls or stalls.
- **Inertia.** Flywheels (and to a lesser extent everything that turns) add inertia, so speed
  changes gradually rather than instantly.
- **Several sources** on one network simply add their push at the shared speed.

### 3.2 Working range and overspeed

Every machine and transmission part has a **working speed range**:

- **Below** the range it works slower, or not at all. Running slow never damages anything.
- **Above** the range it **overspeeds**. It warns first (rattle, sparks, the tachometer needle in
  the red) for a few seconds *(tune)*. If the speed doesn't drop, it **jams**: it enters a
  **damaged state**, stops working and stops transmitting torque, and shows visible wear. The rest
  of the network keeps running around it.
- **Repair** in place with the wrench and a few cheap materials (e.g. iron nuggets for a gearbox).
  Nothing is lost.

### 3.3 Network rules

- **Rotation direction** exists and is visible. Most machines run either way; direction matters
  only where it makes sense (belts, possibly pumps).
- **Conflicts break things.** Joining parts turning in opposite directions, or a gear loop that
  demands two speeds for one shaft, puts the offending gearbox/gear into the damaged state.
- **Torque limit per shaft material.** Wooden shafts early, iron for heavy loads, steel in tier 2.
  Exceeding the limit shears the shaft (damaged state).
- **Supports.** Shafts may be any length but need support at least every 8 blocks (wood) or
  16 blocks (iron) *(tune)*. Supports are **bearings** and anything the shaft connects to
  (machines, gearboxes, engines). A bearing must itself be attached to something, or it breaks;
  it doesn't need a full block (a fence post works and looks good). Bearings can mount on walls
  and hang from ceilings. A span that's too long **sags and wobbles** with a warning and carries
  less torque; it only breaks if also pushed past its torque limit.

### 3.4 Speed regulation

**Manual:** a valve on the steam line. Works for a steady load; an unexpected idle overspeeds.

**Centrifugal governor** (early tier 1, after the first engine): placed on the engine or a shaft
next to it, it throttles the engine's steam intake toward a **target speed** set with the wrench.
It adds no power: heavy load still sags the speed. It also saves steam when idle. It is not a
"place one and done" solution:

1. **Lag and hunting.** The governor reacts with a delay. Too sensitive and the speed oscillates
   ("hunts"), overspeeding machines on the peaks. Too sluggish and a sudden load drop overshoots.
   Flywheel size damps this, so engine, flywheel and governor are tuned together. Sensitivity and
   target are wrench settings; the tachometer shows the oscillation.
2. **The problem moves to the boiler.** Throttling means steam goes unused, so boiler pressure
   rises. An idle, well-governed factory pushes its boiler toward the safety valve, which calls for
   a **furnace damper** (section 4.2) or venting.
3. **Engines fight.** Two governed engines with different targets don't share load: one does all
   the work while the other idles or hunts against it. Match them, or drive several engines from
   one shared governor.

### 3.5 Transmission parts

| Part | Notes |
| --- | --- |
| **Shaft** (wood, iron; steel in T2) | Square cross-section, visibly rotates. Torque limit and support spacing per material. |
| **Bearing** | Shaft support; attaches to blocks, fences, walls, ceilings. |
| **Bevel Gear** (was "angular gears") | 90° turn; T-shape variant gives one input and two outputs for simple branching. |
| **Gearbox** | Fixed 2:1; the wrench flips it to 1:2 (faster/weaker or slower/stronger). Craftable gear sets with other ratios later, if needed. |
| **Pulley + leather belt** (replaces the splitter multiblock) | Links parallel shafts at a distance; different pulley sizes change the ratio. The period way to distribute power in a mill. |
| **Clutch** | Shaft segment that disconnects on a redstone signal. Turns a machine line off without stopping the engine. |

### 3.6 Power sources (tier 1)

| Source | Notes |
| --- | --- |
| **Crank** | Right-click to turn; small torque, very slow. A copper golem can be **assigned** to it (lead or right-click the golem onto the crank): the crank holds it in place, animates its arm and produces the torque. A golem in the rain oxidises into a statue and stops unless waxed. *Optional: the spike found the hooks plausible (brain behaviours, goals, entity interaction) but
the arm animation unproven. Prototype it in 0.4 for a day; drop it if it needs a mixin.* |
| **Water wheel** | Placed in flowing water; torque depends on how much flow touches it. Steady and **deliberately weak**: enough for a grinder or two, not a factory. Makes milestone 0.1 playable. No windmill for now. |
| **Steam engine** | Steam cylinder + flywheel (section 4.4). The real power of tier 1. |

---

## 4. Heat, water and steam

### 4.1 Simulation model

Lumped: one set of values per boiler and per pipe network, not per block. Steam and heat update
every few ticks *(tune)*.

- **Firebox:** fuel becomes heat per tick, shared evenly among the boilers it touches.
- **Boiler:** tracks **water level**, **water temperature** and **steam pressure**. Heat first
  warms the water (a cold start takes a while). At boiling point, further heat turns water into
  steam and raises pressure.
- **Pipe network:** one shared pressure for steam, plus a **throughput limit** per pipe material,
  so a thin pipe can't feed a big engine even at high pressure.
- **Engines** draw steam in proportion to their speed and load, lowering pressure.
- **Rated pressure** per boiler and pipe. Above it, a safety valve vents if fitted; otherwise
  failure (4.3). Running a hot boiler dry is also a failure.
- **Overload overheats the steam side.** When the mechanical network is overloaded, engines stall,
  steam goes unused and pressure climbs. Machines themselves never overheat from running slowly.
- **Not modelled in tier 1:** steam temperature/superheating (tier 2), heat loss to surroundings,
  per-block flow.

### 4.2 Heat sources

- **Firebox** (renamed from "Furnace", which clashes with the vanilla block). Takes any vanilla fuel.
  Fuels differ in **heat rate** as well as duration: wood and sticks burn fast and cool, coal and
  charcoal steady and hot, blaze rods and lava very hot *(tune: a small heat-rate table)*. GUI like a
  furnace's, with only a fuel slot. Fed by hand, hoppers or belts.
- **Bellows** (shaft-driven) blow air into a firebox for more heat from the same fuel: the engine
  helps feed its own fire.
- **Furnace damper:** driven by boiler pressure, it scales the firebox's heat down as pressure
  rises. The second regulator, after the governor.
- **No ash** for now (maybe later).
- **Later tiers:** coke oven (coal → coke, a hotter fuel, plus creosote), liquid-fuel burner fed by
  pipe (creosote, later oil).

### 4.3 Failures and hazards

An escalation ladder, so an attentive player always gets a chance to react:

1. **Warning:** gauge needle in the red → hissing and rattling → the boiler shakes and puffs steam.
2. **Safety valve** (if fitted): vents loudly, wasting steam. Nothing breaks.
3. **Rupture** (no valve, or one that can't keep up): the boiler explodes. Strength scales with
   stored pressure and boiler size; the boiler blocks are destroyed; a short-lived **scalding steam
   cloud** remains.
4. **Burns:** venting steam, steam releases, rusted leaks and bare steam pipes hurt anyone in or
   touching them. Machines warn before releasing steam.

**Setting: explosion mode** = full (default; terrain damage) / contained (only the boiler breaks) /
off (the boiler goes into the damaged state). Kid Mode's invulnerability already covers burns and
explosions.

### 4.4 Multiblocks

- **Formation is automatic.** Place the parts correctly and they visibly connect (seams disappear,
  the engine's rod appears). No hammer or "form" click. Sneak-right-clicking with the wrench says
  what's missing ("no steam dome", "flywheel blocked"). Breaking any part splits the multiblock; a
  boiler vents its steam.
- **Boiler:** a **cuboid** of boiler blocks with any dimensions and orientation within the tier's
  limit (2×2×2 squat, 1×1×4 tall, 1×2×4 lying like a locomotive boiler). It is rendered as one
  proper boiler (rivets, bands, rounded shell). Capacity and heating surface follow from its
  dimensions. **Size limit per tier**, so bigger systems need several boilers and engines:
  tier 1 at most 8 blocks, low rated pressure; tier 2 steel up to ~27 blocks, higher
  pressure *(tune)*. Extra blocks simply don't join, and the wrench shows a warning.
- **Steam dome:** on top of the boiler, its only steam outlet; more domes allow more throughput.
- **Fireboxes** go under or beside the boiler.
- **Steam engine:** a **steam cylinder** (renamed from "steam piston", which clashes with the
  vanilla piston) connected by an animated rod to a **flywheel**, a 3×3 vertical wheel with a shaft
  out of its hub. Cylinder speed depends on steam pressure and flow. Exhaust steam is vented (with a
  hiss: it burns) or piped to a condenser. A second cylinder on the other side of the flywheel makes
  a **twin engine**: more torque, smoother running.
- **Storage tank:** a cuboid multiblock for water, hot water or steam (a steam accumulator smooths
  load spikes).

### 4.5 Water and fluids

- **Water has flow only, no pressure.** Pressure is a steam concept, which keeps the gauges
  meaningful.
- **Pump:** placed in or beside water. The number of source blocks it touches caps how much it can
  draw (a puddle can't feed a boiler house); shaft speed sets the actual flow.
- **Hot water** is its own fluid, which fits the "one content per pipe network" rule.
- **Open loop** (early, simple): pump → boiler → engine → vent. Works, but wastes water and fuel.
- **Closed loop** (the reward): engine exhaust → **condenser** (heat exchanger + radiators) → hot
  water → back to the boiler. Hot feed water needs less heat to boil, so the same firebox makes more
  steam, and little fresh water is needed.
- **Machine cooling feeds the boiler.** Water-cooled machines can run faster (draft idea). Their
  hot output water is piped into the boiler as preheated feed water instead of being dumped, so
  machine waste heat saves fuel.
- **Drain:** releases hot water (with steam puffs) for players who don't want to recycle it.
- **Heat exchanger:** cools steam or hot water by air, or faster when waterlogged. Several can be
  combined. **Radiators** in different materials attach to it and improve cooling.

### 4.6 Pipes

- One **Pipe** family for steam and fluids (renamed from "steam pipe"). Pipes connect
  automatically when their contents match or one is empty: **one content per pipe network, no
  mixing**. The wrench connects, disconnects and rotates them.
- **Materials:**
  - **Copper:** early and cheap, low pressure rating, never rusts.
  - **Iron:** higher pressure rating and throughput, but **rusts and leaks when exposed** (section 5).
  - **Steel** (tier 2): high pressure.
- **Hot pipes:** pipes carrying steam burn on touch, like a magma block. **Lagging** (insulation),
  added later, prevents this.
- **Readability:** pipes show their contents subtly (e.g. steam wisps at joints under high
  pressure).
- **Valves:** manual valve (on/off), redstone valve, steam release (manual venting), emergency
  valve (vents automatically above a set pressure).

---

## 5. Rust

The "rust" of the title: a consequence of poor planning, never of normal use.

- **Triggers:** an **iron** part rusts when it touches water, is waterlogged, is open to the sky
  in rain, or sits in a steam jet or vent cloud. What flows *inside* it never matters.
- **Stages** (like vanilla copper oxidation): clean → spotty → rusty → **leaking**. Each is
  visible; progress is slow (several in-game days *(tune)*). Only **leaking** has an effect: a pipe
  loses fluid and pressure and sprays steam (burns); a machine jams (damaged state).
- **Prevention:** **red oxide paint** stops rust permanently, like waxing copper. Parts can also be
  crafted **pre-painted** (e.g. pipe + paint), so big builds never need painting in place.
- **Repair:** scraping with the wrench returns a part to clean and drops **rust flakes**; a leaking
  part also needs an iron plate.
- **Paint supply:** rust flakes → (grinder) → pigment → paint. Early on, flakes come from scraping;
  later, without manual work, the **washer** slowly rusts iron nuggets in water into flakes.
- **Copper** parts oxidise visually only, like vanilla copper (wax works), with no effect.
- Setting: rust on/off and rust speed.

---

## 6. Items, processing and logistics

### 6.1 Ore processing

| Step | Machine | Result |
| --- | --- | --- |
| Crush (T1) | **Grinder** | Raw ore → **crushed ore**, 1.5× on average (a 50% chance of a second). Stacks with Fortune (grind the raw ore you mined). Smelts in a vanilla furnace. |
| Wash (T1) | **Washer** (water-fed) | Crushed ore → clean dust + small byproduct chance (e.g. copper → gold nuggets, iron → a little redstone *(tune)*). |
| Later (T2+) | Stamp mill, later electric pulveriser | Push total yield toward 2–3×. Alloys arrive. |

The grinder also does non-ore work: cobblestone → gravel → sand, wool → string, bones → extra bone
meal. All recipes are FRAS's own: Horizon's registry forbids copying vanilla recipe data.

### 6.2 Tier-1 machines

| Machine | Role |
| --- | --- |
| **Grinder** | Crushing (GUI). |
| **Washer** | Ore washing; rusting nuggets into flakes. Works on items on a belt or from its input. |
| **Trip hammer** | Ingots → **plates**; compacting. Works on items passing beneath it on a belt. Loud, period, fun to watch. |
| **Lathe** | Rods and plates → **shafts, gears, pipe sections**: the parts later machines are built from. |
| **Sawmill** | Logs → more planks than by hand. |
| **Bellows** | More heat from a firebox (section 4.2). |
| **Trommel** | Rotating sieve; separates gravel, sand and crushed ore by size. Steam-era sorting. |

Machines show a GUI where appropriate (firebox, grinder, etc.) and also accept items in the world
(hand, hoppers, belts). Later tiers: drill/block breaker, mechanical crafter, mixer, alloy furnace.

### 6.3 Logistics

- **Conveyor belt:** connected belts form one continuous belt; a shaft can drive it from anywhere
  along it, up to ~16 blocks per drive *(tune)*. **Belt speed follows shaft speed**, and items add a
  little load, so a struggling engine visibly slows the whole line. Items come from hoppers,
  machine outputs or by hand; belts insert into compatible machines.
- **Processing on the belt:** some machines (trip hammer, washer) work on items riding beneath them.
- **Slopes:** belts can rise one block per block. **Bucket elevator** (shaft-driven) for vertical
  lifts.
- **Diverter** (tier 1): redstone-flipped flap that pushes items off to the side.
- **Filter sorter:** not a steam-era device; it comes with electricity (tier 3).
- **Direct hand-off:** machines push output into adjacent inventories, belts and hoppers.

### 6.4 Crafting progression

- **Entry, hand-crafted from vanilla materials:** crank, water wheel, wooden shafts and gears,
  bearing, wrench, grinder, firebox, small copper boiler, copper pipe. Enough for a first engine.
- **Hand hammer:** makes plates in the crafting grid at the **same yield** as the trip hammer.
  Making them one by one is punishing enough; the trip hammer is about speed and automation.
- **Intermediate parts as the gate:** better tier-1 blocks (iron shafts, governor, gearbox, lathe,
  bigger boilers) need **plates, gears, rods and rivets** that the trip hammer and lathe make. That
  is the first production line, with no special crafting block.
- **Tier-2 gate: steel.** Needs a coke oven and blast furnace built from tier-1 parts and
  firebricks; steel parts are machined on the tier-1 hammer and lathe. Tier-2 machines are probably
  put together on an **assembly table** (frame + parts), where fabrication beyond the crafting table
  starts in earnest. Designed with tier 2.
- **Discovery:** recipes unlock in the vanilla recipe book as you pick up materials.

### 6.5 Metals

Tier 1 uses only vanilla **copper** and **iron**; no new ores. **Steel** (iron + coke) in tier 2.
Bronze or brass (needing tin or zinc ores and world generation) possibly in tier 2.

---

## 7. Player experience

### 7.1 Reading and controlling machines

- **Gauge blocks** with moving dials, readable in the world:
  - **Pressure gauge:** on boilers, pipes and tanks.
  - **Tachometer:** shaft speed, with the connected machine's safe range marked in red.
  - **Sight glass:** boiler water level.
  - All three give a **comparator signal** (low-water alarms, a clutch that sheds load when speed
    sags, etc.).
- **Wrench inspection:** sneak-right-click shows an overlay with exact values: speed, load versus
  capacity, pressure, temperature, rust stage, and what's wrong if a part is jammed or a multiblock
  incomplete. It replaces a separate goggles item.
- **Wrench:** rotate parts, change output sides, connect/disconnect pipes, set governor/damper/
  gearbox, scrape rust, repair.
- **Redstone:** comparator output from gauges, redstone valve, clutch, diverter.
- **Sound as information:** hiss pitch rises with pressure, rattling near overspeed, the engine's
  chuff rate follows its speed, gear whine, the hammer's thump, pipe creaks.

### 7.2 Learning

- From 0.1: **tooltips** (one line; Shift for details such as "Max torque 64. Needs support every
  8 blocks"), **wrench diagnostics**, and **written docs** in the repository, one page per
  subsystem.
- **Recipes viewer:** blocked upstream. Horizon's official Recipes viewer can't load on current launchers,
  and its port (horizon-mods #1214) hasn't landed; no API for other mods to add pages to it is known.
  Not planned before it lands; the Horizon watch flags it. Fallback: FRAS's own recipe pages in the
  engine's item panel (`panel_recipes`).
- 1.0 goal: an in-game **Engineer's Handbook** with chapters per subsystem and example builds,
  as a mod screen. Horizon supports one, but everything on it is hand-drawn (text layout, pages,
  images, links), so the handbook is a subsystem with its own spec.

### 7.3 Look and sound

- Vanilla-style **16×16 textures** and blocky models; a palette of warm copper, soot-black iron,
  riveted plates and red-oxide paint.
- **Animated:** shafts, gears, flywheels, cylinder rods, belts, pulleys, trip hammer, governor balls,
  gauge needles.
- **Effects:** steam puffs from cylinders and vents, glowing firebox mouth, leak jets, rust textures
  per stage, overspeed sparks, explosion steam cloud.
- **Assets** must be original or freely licensed (Horizon's registry forbids Minecraft assets).
  Keep a credits list from the first asset.
- **Placeholder art during development, final art later.** The tool (local Stable Diffusion, an
  external service, or hand-made) is decided when the time comes.

---

## 8. Compatibility

- **Standalone** in tier 1; revisit power interop with other Horizon tech mods (Voltz, Deepbore
  Quarry) once electricity exists.
- **Vanilla inventories:** machines accept items from hoppers and droppers and push into chests;
  mods using the same hooks work for free.
- **Recipes viewer** support once Horizon's port of it lands (section 7.2).
- **Kid Mode:** nothing needed.

---

## 9. Multiplayer and performance

- **Server-authoritative.** The server simulates; clients only render.
- **Cheap rotation sync:** the server sends a network's speed only when it changes noticeably;
  clients animate locally. No per-tick packets per shaft.
- **Cached networks:** rebuilt only when a block is placed or broken, or a chunk it touches loads or
  unloads.
- **Shafts don't tick.** Shafts, bearings and gears have no per-tick work: network membership comes from
  block entity load/unload and block changes. Only sources and machines tick (each tick is a hook call
  with a data snapshot, too costly for hundreds of shafts).
- **Tick budget:** mechanical networks solved per tick; steam/heat every few ticks.
- **Unloaded chunks:** if any part of a network is unloaded, **the whole network pauses**.
  This avoids half-loaded exploits; nothing explodes or rusts while nobody is around. "Unloaded" means
  **not simulated**: block entities stop ticking at the simulation distance, usually nearer than the view
  distance, so a network crossing it pauses where players can still see it. Whether a paused network
  keeps turning visually is decided in the rotational network spec (the spike shows it stopped).
- **No chunk loader in tier 1**; possibly later behind a server setting.

---

## 10. Technical setup

- **Native `.hmod`**, pinned to one Horizon build (`horizon-mod.toml` `[native]`), like Kid Mode.
- **As few mixins as possible:** hooks first; each mixin documented and justified.
- **Layout:** the simulation (torque balance, governor, heat, pressure, flow, rust) in
  **game-free modules with unit tests**, like Kid Mode's `logic.rs`; blocks, hooks and rendering a
  thin layer on top.
- **CI:** `cargo fmt --check`, clippy, tests on every push; weekly Horizon-watch workflow (from
  Kid Mode) for API changes, with a rule for the Recipes viewer port.
- **Licence:** GPL-3.0-or-later.
- **Languages:** English and Polish from the start (with a terminology dictionary, as in Kid Mode).
- **Settings page** (Options > Mods > FRAS): explosion mode, rust on/off and speed, possibly a
  difficulty multiplier.
- **Publishing** is outside this plan.

---

## 11. Roadmap (tier 1)

| Milestone | Content | Exit criteria |
| --- | --- | --- |
| **0 — Technical spike** (not released) — **done** | One ticking block entity, a smoothly rotating shaft render, a network that survives chunk unload/reload, a simple GUI. Check API support for: block entity renderers at scale, menus, fluids/inventories, chunk events, entity hooks for the golem, Recipes viewer integration, custom book screens. | A written feasibility report; risks resolved or redesigned. Done 2026-11-23: `spike/FINDINGS.md`, all checks passed in game. |
| **0.1 — Mechanical** | Crank, water wheel, wooden/iron shaft, bearing, bevel gear, gearbox, clutch, pulley + belt drive, wrench (incl. inspection), tachometer, **grinder**, hand hammer. | Push-vs-pull speed model, overspeed/jam/repair, supports, conflicts working in game. |
| **0.2 — Steam** | Firebox, boiler, steam dome, pipes (copper, iron), valves (manual, redstone, release, emergency), pressure gauge, sight glass, pump, steam cylinder, flywheel, twin engine, storage tank. | Boilers warm up, build pressure, feed engines; escalation ladder and explosion modes work. |
| **0.3 — Regulation and cooling** | Governor, furnace damper, bellows, heat exchanger, radiators, condenser, hot water, drain, machine water cooling; rust, paint, scraping. | Hunting/tuning playable; closed loop saves fuel; rust stages and leaks work. |
| **0.4 — Factory** | Conveyor belt, slopes, bucket elevator, diverter, trommel, washer, trip hammer, lathe, sawmill; intermediate parts; copper golem (optional). | A full ore line runs unattended; "factory makes the factory" progression in place. |
| **1.0 — Tier 1 complete** | Balance pass, final art and sounds, Polish translation complete, docs, Engineer's Handbook (if feasible). | Tier 1 playable start to finish. |

---

## 12. Before implementation: subsystem specs

Each subsystem gets its own detailed spec, written and agreed before it is implemented:

1. **Rotational network:** graph building, the push/pull solver and its curves, inertia, gear
   ratios, conflicts, supports, overspeed timing, client sync. **Written:** [`specs/rotational-network.md`](specs/rotational-network.md)
   (2026-11-23); it refines sections 3 and 9 (see its section 15).
2. **Steam and heat:** boiler state, heat rates per fuel, boiling, pressure, throughput, engine
   steam draw, governor and damper control loops (with hunting), failure ladder. **Written for 0.2:**
   [`specs/steam-and-heat.md`](specs/steam-and-heat.md) (2026-11-24, holds the 0.2 exit criteria; the governor
   and damper are reserved for 0.3). **Written for 0.3:** [`specs/regulation.md`](specs/regulation.md)
   (2026-11-25: governor, damper, bellows; holds the 0.3 exit criteria).
3. **Fluids and pipes:** pump flow, pipe throughput per material, tanks, hot water, condensers,
   machine cooling. **Written for 0.2:** [`specs/fluids-and-pipes.md`](specs/fluids-and-pipes.md)
   (2026-11-24; hot water, condensers and cooling reserved for 0.3). **Written for 0.3:**
   [`specs/cooling.md`](specs/cooling.md) (2026-11-25: exhaust steam, heat exchanger, radiators, hot water,
   drain, machine cooling).
4. **Multiblocks:** formation and validation, chunk-border handling, rendering as one object. **Written
   for 0.2:** [`specs/multiblocks.md`](specs/multiblocks.md) (2026-11-24).
5. **Rust:** triggers, stage timing, effects, paint and repair. **Written:** [`specs/rust.md`](specs/rust.md)
   (2026-11-25, for 0.3).
6. **Items and machines:** recipes, processing times, speed ranges and loads per machine, belts.
   **Grinder written:** [`specs/grinder.md`](specs/grinder.md) (2026-11-23), the one machine 0.1 needs; it
   refines section 6 (see its section 13). The other machines follow with their milestones.
7. **Progression:** full tier-1 recipe tree and balance targets.
8. **UX:** gauges, inspection overlay, GUIs, sounds, tooltips.
9. **Engineer's Handbook:** text layout, pages, images, cross-links, content per subsystem (for 1.0).

---

## 13. Tier-1 block and item list

**Power:** crank, water wheel, steam cylinder, flywheel.
**Transmission:** wooden shaft, iron shaft, bearing, bevel gear, gearbox, pulley, leather belt,
clutch, governor.
**Steam and heat:** firebox, boiler, steam dome, bellows, furnace damper, storage tank.
**Fluids:** copper pipe, iron pipe, valve, redstone valve, steam release, emergency valve, pump,
heat exchanger, radiator(s), condenser, drain; fluid: hot water (steam as a pipe content).
**Machines:** grinder, washer, trip hammer, lathe, sawmill, trommel.
**Logistics:** conveyor belt, bucket elevator, diverter.
**Gauges:** pressure gauge, tachometer, sight glass.
**Tools:** wrench, hand hammer.
**Materials:** crushed ores, ore dusts, plates, rods, gears, rivets, rust flakes, pigment,
red oxide paint.

## 14. Renames from the first draft

| Draft | Now | Why |
| --- | --- | --- |
| Furnace | **Firebox** | Clashes with the vanilla furnace. |
| Steam piston | **Steam cylinder** | Clashes with the vanilla piston. |
| Steam pipe | **Pipe** (copper/iron/steel) | It carries water too. |
| Angular gears | **Bevel gear** | The real name. |
| Splitter (multiblock) | **Pulley + leather belt**, T-shaped bevel gear | Period-appropriate, also changes ratios. |
