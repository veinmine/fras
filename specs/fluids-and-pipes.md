# Fluids and pipes: subsystem spec

Spec 3 of the design doc's section 12. Version 1.1 (2026-11-24), scope: **milestone 0.2 (Steam)**.
Builds on `Fire, rust and steam - design.md` v2.1 (sections 4.5, 4.6 and 9), `specs/steam-and-heat.md`
(spec 2: pressure, ratings, burns), `specs/multiblocks.md` (spec 4: how tanks form) and `spike/FINDINGS.md`.
Where this spec is more precise than the design doc, this spec wins; section 14 lists what changes in the
design doc.

**Reviewed with Marcin** (2026-11-24) after being written without an interview, like specs 2 and 4. Section 15
records the decisions; all of this spec's were taken as proposed.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 13 collects them.

---

## 1. Scope

**In 0.2:** copper pipe, iron pipe, the four valves (manual, redstone, steam release, emergency), the pump,
storage tanks as fluid stores, buckets, pipe networks and their contents rule, throughput limits and how
steam and water are routed, bursts.

**Designed for, built later (section 10):** hot water, exhaust steam, condenser, heat exchanger, radiators,
drain and machine cooling (0.3); rust and leaks on iron pipes (0.3); lagging; steel pipe (tier 2); a pipe that
carries other mods' fluids.

---

## 2. Contents and units

- **Contents in 0.2:** water (vanilla water) and steam (a FRAS fluid with no world form or bucket, per the
  spike). Hot water joins in 0.3.
- Amounts in **mB**, flows in **mB/s**. 1 mB of water boils into 1 mB of steam.
- **Water has flow only, no pressure** (design doc 4.5). Steam has one pressure per network (spec 2, 5).

---

## 3. Pipe networks

### 3.1 Parts and endpoints

- **Pipe parts** carry: copper pipe, iron pipe, manual valve, redstone valve. A **pipe network** is a set of
  pipe parts joined by connected faces.
- **Endpoints** attach to a network through a connected face and give it its content:

  | Endpoint | Content | Role |
  | --- | --- | --- |
  | Steam dome | steam | source (its boiler's steam) |
  | Steam cylinder inlet | steam | consumer |
  | Steam release, emergency valve | steam | consumer (vents) |
  | Boiler block face (not top) | water | consumer (feed water) |
  | Pump outlet | water | source |
  | Storage tank face | its content, none while empty | source and consumer |

- Gauges attach to a part's face but are not endpoints and don't connect.

### 3.2 One content per network

- A network's content is set by its endpoints: **steam** if it touches any steam endpoint, **water** if any
  water endpoint, **empty** if none. An empty tank takes the content of the first network it is connected to
  and keeps it until it is empty again.
- **A connection that would put steam and water endpoints on one network is never made.** The face stays
  disconnected and the wrench overlay says "would mix steam and water". This is the design doc's "one content
  per pipe network, no mixing", decided by what a network is connected to rather than by what happens to be
  inside it, so it is stable when a pipe is momentarily empty.
- A pipe touching both a dome and a boiler side when placed connects to the face it was placed against first,
  then to every other face that agrees with that content.

### 3.3 Connections

- Each pipe part stores **six face flags** (connected or not) in its block state, which also draw its arms.
  Copper and iron pipe connect to each other.
- **Placing** a pipe connects every face whose neighbour is a pipe part or an endpoint facing it, unless the
  content rule (3.2) forbids it.
- **Wrench, right-click a face:** toggles that face (connect / disconnect) on both sides. Valves are placed
  along an axis like a shaft; the wrench rotates them.
- A network is rebuilt only when a connection changes, a part is placed, broken, bursts or is repaired, a
  valve closes fully or opens from closed, or a chunk it touches loads or unloads (as rotational networks).

---

## 4. Pipe materials

### 4.1 Numbers

| Material | Throughput | Rated pressure | Steam capacity | Rusts |
| --- | --- | --- | --- | --- |
| Copper pipe | 80 mB/s | 4 bar | 5 mB/bar | Never (visual oxidation only, waxable) |
| Iron pipe | 200 mB/s | 8 bar | 5 mB/bar | From 0.3 (design doc 5) |
| Valves (all four) | 200 mB/s × opening | 8 bar | 5 mB/bar | From 0.3 |
| Steam dome | 100 mB/s | its boiler's | — | |

All *(tune)*. The same throughput applies to water and steam. A twin engine at 4 bar draws about 130 mB/s, so
copper pipe starves it and iron doesn't; a boiler on four coal fireboxes boils 160 mB/s and needs two domes.

### 4.2 Look

- A 6-pixel pipe with flanged arms to each connected face (a multipart model, no block entity drawing).
  Copper oxidises through the vanilla copper stages visually; wax stops it.
- **Readability:** a steam pipe shows faint wisps at its joints above 75% of its network's red line; a water
  pipe shows nothing. Pipes creak softly near the red line.

### 4.3 Hot pipes

Pipes on a steam network above 0.2 bar burn on touch like a magma block (spec 2, section 8).

### 4.4 Burst and repair

- A pipe or valve whose pressure timer fills (spec 2, 7.1) **bursts**: it goes into the damaged state with a
  split seam, stays part of its network, and **vents** like an open steam release with its full throughput
  (route-limited, 5), with a loud jet that scalds the block around it. Bursts never explode.
- Water networks never burst.
- **Repair:** wrench right-click, from the inventory: copper pipe 1 copper nugget, iron pipe or valve 1 iron
  nugget *(tune)*. It stops venting at once; free in creative.
- In 0.3 a rusted **leaking** pipe reuses this, venting at a fraction of its throughput.

---

## 5. Routing and throughput

Each steam step (every 4 ticks, spec 2, 4.1) every running network delivers fluid from its sources to its
consumers. The behaviour, whatever the implementation:

1. **No pipe part carries more than its throughput** (valves: throughput × opening; a dome: its own limit).
2. A consumer's supply is capped by the **narrowest part on its route** from the sources.
3. Consumers that share a narrow part **split it in proportion to their demand**.
4. **Loops add no capacity:** each consumer is fed along one route, its widest path from the sources
   (computed at rebuild; ties broken by block order, so the choice is deterministic).

Implementation: at rebuild, build the widest-path tree from a virtual root joined to every source; each step,
add the consumers' demands up the tree, and wherever a part's sum exceeds its capacity, scale the demands
below it down to fit, top down. A partly opened valve changes only a capacity, not the tree.

### 5.1 Steam

- **Sources:** steam domes and steam tank faces. All steam in the network is one pool (spec 2, 5), so a
  source's limit is only a route capacity.
- **Consumers:** cylinders (demand from spec 2, 9.2), open steam releases and emergency valves (80 mB/s each),
  burst parts (their throughput).
- Deliveries are then scaled down together if they would take more steam than the network holds this step.
  Each consumer's `fill` = delivered / demand.

### 5.2 Water

- **Sources:** pumps (their flow this step, 7.2) and water tanks (what they hold).
- **Consumers:** boilers (their free space) and water tanks (their free space).
- Three passes, each with the capacity left over from the one before: **pumps → boilers**, then **tanks →
  boilers**, then **pumps → tanks**. So boilers come first, tanks fill from what's left, and a tank only
  drains toward a boiler.
- A boiler accepts water until it is full. Feed water mixes into its temperature (spec 2, 4.1 step 5).
- Pipes hold no water themselves: water moves from source to consumer within the step.

---

## 6. Valves

All four are iron-bodied, rated 8 bar, throughput 200 mB/s, and placed along an axis like a shaft (the two
vents have one pipe face instead). They work on steam; the manual and redstone valves on water too.

### 6.1 Manual valve

- A pipe segment with a handwheel. **Opening 0 to 100% in 10% steps:** right-click with an empty hand opens it
  one step, sneak-right-click closes it one step. The wheel turns visually; a squeak per step.
- Its throughput is 200 mB/s × opening, so a part-opened valve **throttles** an engine (it starves, spec 2,
  9.2): the design doc's manual speed regulation (3.4).
- **Fully closed** it is a boundary: the two sides become separate networks (spec 2, 5 says how steam
  splits).

### 6.2 Redstone valve

- A pipe segment that is **open while powered** and closed unpowered, like a door. Binary, no partial
  opening. A gear-driven actuator on top shows the state.
- Example: a lever starts and stops an engine; a sight glass comparator through an inverter shuts a feed
  line.

### 6.3 Steam release

- A vent with one pipe face, mounted on a pipe end, a dome or a steam tank. **Right-click toggles** it open or
  closed. Open, it vents up to 80 mB/s *(tune)*, route-limited, with a scalding jet 2 blocks out of its
  outlet (spec 2, section 8) after a half-second warning hiss.
- Steam only: it can't connect to a water network.

### 6.4 Emergency valve

- The safety valve (spec 2, 7.2): a vent with one pipe face, steam only, mounted like the steam release.
- **Set pressure** by wrench right-click, cycling in 0.5 bar steps from 1.0 to 8.0 bar. When placed it takes
  the red line of the network it attaches to minus 0.5 bar (3.5 bar if it attaches to nothing yet). Shown on
  its tooltip-like overlay and on a small dial.
- **Simmers** from 0.2 bar below its set pressure (wisps, rising whistle). **Opens fully** at its set pressure
  and vents up to 80 mB/s *(tune)*, route-limited, with a loud pop and a scalding jet upward. **Reseats** when the
  pressure falls 0.3 bar below the set pressure.
- No redstone input. Several can sit on one network; each vents its own 80 mB/s.

---

## 7. Pump

### 7.1 Block

- A machine on the rotational network (rotational spec 9.3): one shaft socket (set on placement, the wrench
  rotates it), the **outlet on the face opposite the socket**. The other four faces are intakes.
  Waterloggable.
- Working range **8 to 128 RPM**, load **4 Nm while pumping**, inertia 4, friction 0.05, iron's speed limit
  rules otherwise *(tune)*. Direction doesn't matter. Repair: 2 iron nuggets and 1 copper ingot.
- Look: an iron barrel with a copper outlet; a rocking beam on top whose rate follows the speed. Sound: a
  rhythmic clank and gurgle.

### 7.2 Flow

- **Source cap:** 25 mB/s for each **water source block** among its four intake faces, plus 25 if it is
  itself a waterlogged source *(tune)*: at most 125 mB/s. Flowing water doesn't count. The pump never removes
  source blocks. This is the design doc's "a puddle can't feed a boiler house".
- **Flow** = min(source cap, 0.75 mB/s per RPM) *(tune)* inside its working range; nothing below 8 RPM.
- It pushes into its outlet: a water network, or directly into a boiler or tank against that face.
- **Idle** when nothing takes its water (everything full, or no outlet): no load, as an idle machine. It pulls
  its full load whenever it delivers anything.
- Examples: on an engine at ~100 RPM with four or more source blocks it delivers 75 mB/s, close to one
  loaded cylinder's use (spec 2, 9.2); on a water wheel (~20 RPM) 15 mB/s; on a one-block puddle 25 mB/s at
  most.

---

## 8. Tanks and buckets

- The **storage tank** is a cuboid multiblock of iron tank blocks, up to 27 blocks (spec 4, section 5).
- **Content:** empty, water or steam (3.2). Water: 4000 mB per block. Steam: 400 mB per bar per block of
  steam capacity, rated 6 bar, so a steam tank is an **accumulator** that adds capacity to its steam network
  and smooths load spikes (spec 2, 5). A steam tank can rupture (spec 2, 7.3); a water tank can't.
- Every face of every tank block can take a pipe; each connection is a source and a consumer for routing (5).
- **Buckets:** right-click a boiler or a water tank with a water bucket to add 1000 mB if there's room; an
  empty bucket on a water tank takes 1000 mB. Boilers can't be emptied by bucket.
- **Other mods:** water tanks and boilers expose vanilla water through the `FLUIDS` capability (spike:
  `ModFluidTanks`), so other mods' pumps and pipes can fill them and drain tanks. Steam is not exposed in 0.2.

---

## 9. Wrench overlay, tooltips, sounds

- **Pipe:** content; for steam, pressure, red line and which part sets it; flow through this part versus its
  throughput ("carrying 56 / 80 mB/s"); face connections, and why a face won't connect.
- **Manual / redstone valve:** opening, flow through it.
- **Steam release / emergency valve:** open or closed, set pressure, venting rate, "simmering".
- **Pump:** speed and working range, sources found and cap, flow, "no water", "outlet full", "too slow".
- **Tank:** content, amount and capacity, pressure for steam.
- **Burst part:** "Burst! Repair: 1 iron nugget".
- Tooltips, one line, Shift for details: "Carries 80 mB/s. Rated 4 bar." "Opens in 10% steps." "Draws 25 mB/s
  per water source it touches."
- Sound cues (designed in the UX spec): pipe creak near the red line, valve squeak, emergency valve simmer and
  pop, release hiss, burst jet roar, pump clank.

---

## 10. Hooks for 0.3

- **Hot water** becomes a third content with its own endpoints (condenser and machine cooling outlets, boiler
  feed, drain) and feeds boilers at 90 °C (spec 2, 4.1).
- **Exhaust steam** becomes a fourth content, from cylinder tops to condensers, so it can never reach a boiler
  or a cylinder inlet.
- **Drain:** a water and hot water consumer that dumps what it gets.
- **Rust:** iron pipes and valves rust from outside triggers only (design doc 5); leaking reuses 4.4.
- The routing in section 5 takes new contents and endpoints without change.

---

## 11. On Horizon

- **Pipes, valves and tanks don't tick.** They are non-ticking block entities (one type per block, spike
  lesson 1) for `block_entity_on_load` and `on_chunk_unloaded` membership, like shafts. Only the pump ticks.
- A **level attachment** holds the fluid part registry and cached networks, next to the rotational one. The
  steam step (spec 2, 14.1) rebuilds dirty networks and routes.
- Face flags live in the block state (needed for the model anyway); valve openings, emergency set pressures,
  burst state and tank contents in the block entity.
- Steam is saved only in boilers and tanks, in proportion to capacity; pipes hold too little to matter and
  start a reload empty.
- Parts refuse pistons. Structure rotation rotates face flags and axes.
- The spike found that a block's container is the same for every side (`ModContainer` is static per block).
  The pump has no inventory, so this only matters for tanks, which use the fluid capability, not a container.

---

## 12. Code layout and tests

- **`sim::fluid`**, game-free: `graph` (parts, faces, endpoints, networks, the contents rule), `route` (the
  widest-path tree and scaling), `water` (the three passes), `pump` (flow from speed and sources), `valve`
  (openings, emergency valve hysteresis).
- **Unit tests** at least for: a connection mixing steam and water is refused; an empty tank takes the first
  content; a line capped by its narrowest pipe; a shared bottleneck split in proportion to demand; a loop
  adding no capacity and the deterministic route; a part-open valve scaling flow; a closed valve splitting a
  network and reopening joining it; the water passes' order; a pump's flow from speed and source count, idle
  when the outlet is full; emergency valve simmer, open and reseat; a burst part venting its throughput;
  buckets; save and reload of tank contents.

---

## 13. Starting numbers *(tune)*

Throughput: copper pipe 80, iron pipe 200, valves 200 × opening, steam dome 100 mB/s. Ratings: copper pipe
4 bar, iron pipe and valves 8 bar. Pipe steam capacity 5 mB/bar. Manual valve steps 10%. Steam release and
emergency valve vent 80 mB/s; emergency valve 1.0 to 8.0 bar in 0.5 steps, default red line − 0.5, simmer
0.2 below, reseat 0.3 below. Pump: 8 to 128 RPM, 4 Nm, inertia 4, 25 mB/s per source block (max 125),
0.75 mB/s per RPM. Tank block: 4000 mB water or 400 mB/bar steam, rated 6 bar. Wisps above 75% of the red
line. Repair: copper pipe 1 copper nugget, iron pipe and valves 1 iron nugget, pump 2 iron nuggets + 1 copper
ingot.

### 13.1 Recipes (0.2)

| Result | Recipe |
| --- | --- |
| 6 copper pipe | shaped `CCC` (3 copper ingots) |
| 6 iron pipe | shaped `PPP` (3 iron plates) |
| Manual valve | shapeless: copper pipe + iron ingot |
| Redstone valve | shapeless: manual valve + redstone dust |
| Steam release | shapeless: manual valve + iron bars |
| Emergency valve | shapeless: manual valve + iron ingot + copper ingot |
| Pump | shaped `I I` / `PSP` / `III` (I iron ingot, P copper pipe, S iron shaft) |

Copper pipe and the valves are entry recipes from vanilla materials (design doc 6.4); iron pipe needs plates
from the hand hammer. Each unlocks in the recipe book on picking up one of its ingredients.

---

## 14. Changes to the design doc

- 4.6: a network's content is decided by its endpoints; a connection that would mix is refused. The manual
  valve opens in 10% steps (it is the throttle of 3.4); the redstone valve is open while powered.
- 4.5: the pump's cap is 25 mB/s per touching source block and it never removes sources; pipes hold no water.
- New: routing rules (section 5): narrowest part on the route, proportional split, loops add no capacity.
- New: bursts (4.4) as the failure of pipes and valves; they vent, never explode.

---

## 15. Decisions

Reviewed with Marcin on 2026-11-24; all of this spec's points were taken as proposed.

1. **F1. Contents by endpoints** rather than by what's inside (3.2). A run that gains a steam endpoint must be
   disconnected before it can carry water.
2. **F2. Routing ignores loops** (one widest route per consumer). A second parallel pipe adds no capacity; use
   iron pipe or a separate line.
3. **F3. Throughputs:** copper 80, iron 200, dome 100 mB/s *(tune)*. Copper pipe starves a twin engine.
4. **F4. Manual valve:** right-click opens, sneak-right-click closes, 10% steps, empty hand. The wrench stays
   for connecting faces and inspection.
5. **F5. Redstone valve:** open while powered (like a door, the opposite of the clutch) and binary, so it
   can't replace the 0.3 governor.
6. **F6. Pump intake:** source blocks only, never removed (infinite), on four faces plus waterlogged.
7. **F7. Valves are iron-bodied,** never set the red line, and rust in 0.3 unless painted.
8. **F8. Bursts don't break the pipe** or split the network; they vent until repaired.
9. **F9. No other mods' fluids** in 0.2; tanks and boilers accept vanilla water through the fluid capability.
