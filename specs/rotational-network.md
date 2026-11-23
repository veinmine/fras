# Rotational network: subsystem spec

Spec 1 of the design doc's section 12. Version 1.0 (2026-11-23), scope: **milestone 0.1 (Mechanical)**.
Builds on `Fire, rust and steam - design.md` v2.1 (section 3 and 9) and `spike/FINDINGS.md`. Where this
spec is more precise than the design doc, this spec wins; section 15 lists what changes in the design doc.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 14 collects them.

---

## 1. Scope

**In 0.1:** wooden shaft, iron shaft, bearing, bevel gear (wood, iron), gearbox, clutch, pulleys (small,
large) and leather belt, crank, water wheel, grinder (as the reference machine), tachometer, wrench
(configure, repair, inspect). The hand hammer is in 0.1 but has nothing to do with the network.

**Designed for, built later:** steam engine and flywheel (0.2), governor (0.3), belt conveyor drive and other
machines (0.4). Section 9.3 defines the interface they plug into, so 0.2 adds parts, not a new solver.

**Not here:** machine recipes and processing times (items and machines spec), the inspection overlay's look
and sound design (UX spec). This spec says *what* they show and *when* they sound.

---

## 2. What the player sees

| Quantity | Shown as | Where |
| --- | --- | --- |
| Speed | **RPM** plus a direction arrow (↻ / ↺) | Tachometer, wrench overlay |
| Torque | **Nm** | Tooltips ("Max torque 32 Nm"), wrench overlay |
| Load | **Percent**: torque drawn versus what the sources can give at the current speed | Wrench overlay |
| Power | Never shown | |

Tier-1 speed scale: crank up to ~32 RPM, water wheel ~24, steam engine 64 to 128 (0.2). **Hard cap 256 RPM:**
any part above it jams at once (section 6.2).

---

## 3. Physical model

### 3.1 Systems and networks

- A **part** is any block of this subsystem: a shaft, bearing, gear, gearbox, clutch, pulley, source or
  machine.
- A **system** is a set of parts **rigidly linked**: shafts, gears, gearboxes, engaged clutches and gripping
  belts. Every part in a system turns at a fixed ratio to the system's reference speed `ω` (section 4.3), so
  a system has **one degree of freedom**.
- A **network** is a set of systems coupled by **slipping belts** (section 7.4). Usually a network is one
  system. The network is the unit that pauses (section 10.4) and that the tachometer and overlay describe.

### 3.2 Torques

Each part contributes torques at its own local speed `ω_p = r_p · ω` (`r_p` signed, section 4.3):

- **Sources push** with a straight line from stall torque `S` at standstill to zero at top speed `V`:
  `τ = S · (1 − ω_p / V)`, clamped to `[−S, S]`, with `ω_p` measured in the source's own driving
  direction. Driven backwards, a source pushes at its stall torque and acts as a brake; driven above its top
  speed, it drags. `S` scales with the source's input (cranking, water flow, later steam pressure). A source
  with no input contributes no torque, only inertia and friction.
- **Machines pull** a constant **load** `L` while they have work, at any speed, against the direction of
  rotation; almost nothing while idle (section 9.2).
- **Friction** on every part: a viscous term `c_p · ω_p` plus a small constant term `f_p` against the motion,
  so a network with no push coasts **to a stop** rather than forever. Plain shafts cost least, gears,
  gearboxes and pulleys more; a sagging span or slipping belt more still. Wood and iron have the same
  friction. Target: a shaft costs about 2% of a crank's push at full speed *(tune)*.

### 3.3 Inertia

Every part has an inertia `I_p`. Referred to the system's reference speed, the system's inertia is
`I = Σ I_p · r_p²`. Small parts have small values; the flywheel (0.2) has a huge one (section 14). In 0.1
networks respond quickly: a crank line spins up or down within a few seconds.

### 3.4 Integration

Once per server tick (`dt` = 1/20 s), for each running system:

1. Sum all torques referred to the reference: `T(ω) = Σ r_p · τ_p(ω_p)`. Source and viscous terms are linear
   in `ω`, so `T(ω) = A − B·ω − C·sign(ω)`, where `C` collects loads and constant friction.
2. **Semi-implicit step** (stable however stiff the curves): `ω' = (I·ω + k·dt·(A − C·sign(ω))) / (I + k·dt·B)`,
   with `k` the acceleration constant *(tune)*.
3. **Stiction:** if the step would carry `ω` through zero, or `ω` is 0 and `|A| ≤ C`, the result is `ω' = 0`.
   A network whose load exceeds its sources' stall torque never starts (section 9.2). Below 0.5 RPM *(tune)*
   with no push, snap to 0.
4. Write `ω'` back and update damage timers (section 6).

Tuning targets for `k` and the inertias: a 10-shaft crank line reaches 90% of its settled speed in ~2 s and
coasts to a stop in ~3 s; a flywheel engine (0.2) takes ~10 s to spin up.

---

## 4. Graph

### 4.1 Connections and sockets

- **Shaft-like parts** (shaft, bearing, clutch, pulley, gearbox, bevel gear) connect along their axis or
  faces as defined per part (section 7). Two shafts connect only when they lie on the same axis. Wood and
  iron connect to each other.
- **Machines are end points.** Each machine and source has **one marked shaft socket** (shown on its
  texture), set by placement and rotatable with the wrench. Power never passes through a machine. A through
  axle on a later machine (e.g. a trip hammer's cam shaft) is a deliberate per-machine exception, specified
  with that machine. The water wheel has its axle out of both hub faces (section 8.2).
- To drive several machines, players run a **line shaft** and branch with a T bevel gear or drop a belt to
  each machine.

### 4.2 Building

Networks are cached and rebuilt only when something changes: a part is placed, broken, jams, is repaired or
reconfigured, a clutch changes state, a belt grips or slips, or a chunk a network touches loads or unloads.
A rebuild walks the graph from the changed position(s) (breadth first, as in the spike) and produces systems
with ratios, the networks they form, the spanning tree used for torque flow (section 6.3), the support spans
(section 5) and the set of chunks the network occupies (section 10.4).

### 4.3 Ratios and direction

- Each part's **local speed** is signed relative to its own axis's positive direction (right-hand rule
  around +X, +Y or +Z).
- The walk assigns each part a signed ratio `r_p` relative to the system's **reference part** (the
  lowest position in block order, so the choice is deterministic). Shafts on one axis share a ratio; a
  gearbox multiplies by 2 or ½; a pulley pair by the diameter ratio; a crossed belt and certain gear meshes
  flip the sign.
- **Bevel gear signs** follow the geometry: two meshing bevel gears turn so their teeth move the same way at
  the contact point. The two outputs of a T gear lie on one axis on opposite sides of the input, so they turn
  opposite ways relative to that axis. The unit tests pin each orientation down.
- **Loops** are allowed when consistent: two paths between the same parts that imply the same ratio. An
  **inconsistent loop** (different ratio or direction) is a conflict (section 6.5).

---

## 5. Supports

- **Supports** are bearings and every non-shaft part: machines, sources, gearboxes, bevel gears, clutches.
  **Pulleys are not supports.** A jammed part still counts (it is still bolted down).
- A **span** is a run of plain shafts between two supports, horizontal or vertical. Maximum span: **8 blocks
  if any shaft in it is wooden, 16 if all are iron** *(tune)*.
- **Too long:** every shaft in the span **sags and wobbles** (visible offset, wobble sound), its torque limit
  is **halved**, its friction **doubled**, and the overlay and tooltip warn ("span 11, needs a bearing every 8").
  It breaks only by shearing past its (halved) limit (section 6.3).
- **Bearing mounts.** A bearing must be attached to something: a sturdy face (the vanilla "can support
  centre" test, as for a lantern), a fence, a wall, iron bars, or a chain above (hanging). If its mount goes
  away, the bearing **drops as an item** and the line splits there.

---

## 6. Damage: overspeed, shear, conflicts, jam, repair

### 6.1 Speed limits by part type

| Part | Speed limit |
| --- | --- |
| Machines | Working range, min and max (grinder 12 to 96 RPM *(tune)*). |
| Gears, gearboxes, pulleys | **Max only**, by material: wood ~64, iron ~192 RPM *(tune)*. |
| Shafts, bearings, clutches | **None.** Their limit is torque (6.3). |
| Sources | None of their own (torque falls to zero at top speed). |
| Everything | Global cap 256 RPM. |

The network's **red line** is the lowest max of any part on it; the tachometer and overlay show it, so the
player sees which part breaks first.

### 6.2 Overspeed

- Above its max, a part's **warning timer** fills: it rattles, throws sparks, the tachometer needle sits in the
  red. Full after **5 s** *(tune)*: the part **jams**.
- At 1.5× its max or more, the timer fills **5× faster** (about 1 s). Above 256 RPM any part jams at once.
- Back in range, the timer **drains at the rate it filled**. A brief spike survives; repeated spikes add up.
- The rattle quickens as the timer fills.

### 6.3 Torque limit and shear

- **Torque flow.** For each shaft-like part, the solver computes the torque **flowing through it**: the net
  torque of everything on one side of it (sources, loads, friction, and the torque spent accelerating those
  parts). On a tree this is a cut sum over the spanning tree. Where a consistent loop gives two paths, the
  torque is **split evenly**. The implementation treats the system as a network of unit conductances and takes
  the least-squares flow, which equals the cut sum on trees and splits evenly between parallel paths. Flow is
  computed in reference units and divided by `|r_p|` for the local torque, so **torque doubles on the slow
  side of a gearbox**.
- Recompute flows when the topology changes or any source or load changes by more than 5%, at most every
  4 ticks *(tune)*. The timers use the latest flows.
- **Only shaft segments have a torque limit** (shaft, bearing, clutch): wood 32 Nm, iron 128 Nm *(tune)*;
  bearings and clutches have iron's limit. Gears and gearboxes break only by overspeed or conflict.
- Over its limit, a shaft **creaks and twists** and fills a warning timer with the same rule as overspeed
  (5 s, 5× faster at 1.5×, drains when back under). Full: it **shears** (jams).
- Overspeed **rattles**, overload **creaks**, so the sound tells which fix is needed.

### 6.4 The jammed state

- A jammed gear, gearbox, pulley or sheared shaft is a **break**: the network splits there. The side with the
  sources keeps running; the side cut off coasts down. The part stops turning and shows wear.
- A jammed machine **drops off** as an end point: no load, no work, inventory kept.
- **Cascades are allowed:** a jammed machine sheds its load, the network speeds up, and the next weak part
  starts to rattle.
- Jam state, its reason and the warning timers are saved with the part.

### 6.5 Direction conflicts

- **Joining two turning networks** (placing the connecting part, engaging a clutch, a belt gripping) normally
  shares momentum: the new speed is the inertia-weighted average, `ω = Σ I_i ω_i / Σ I_i` in common reference
  units. If the two were turning **in opposite directions and both faster than 8 RPM** *(tune)*, the
  connecting part takes the shock and **jams at once**. Systems that only rejoin because a chunk loaded take
  no shock.
- **Opposing sources** on one network fight: their signed torques add, and the one driven backwards acts as a
  brake (3.2). Nothing breaks.
- **Inconsistent loop:** the part whose placement or change closed the loop **jams at once**. On a rebuild
  without such a part (e.g. after loading), the part chosen is the one in the loop with the lowest speed
  limit, ties broken by block order, so the result is deterministic.

### 6.6 Repair

- **Right-click a jammed part with the wrench.** The materials come from the inventory, about one eighth
  of the part's recipe *(tune)*:

  | Part | Repair |
  | --- | --- |
  | Wooden shaft, wooden bevel gear, pulley | 1 stick |
  | Iron shaft, bearing, clutch, iron bevel gear | 1 iron nugget |
  | Gearbox | 2 iron nuggets |
  | Grinder | 2 iron nuggets, 1 flint |

- Repair works **at any time**, network running or not. The part reconnects at once and joins at the
  network's speed (6.5 applies). If the cause remains, it warns again, and the overlay says why ("still 20%
  over its limit").
- Free in creative. Missing materials: the overlay lists them, nothing happens.
- Breaking a jammed part drops its normal item. Repair exists so that the player doesn't have to rebuild.

---

## 7. Transmission parts

### 7.1 Shafts

Wooden and iron. Placed against a face, a shaft lies along that face's axis (as in the spike). Torque limit
and span per material (5, 6.3). Square cross-section, visibly turning.

### 7.2 Bearing

- A **shaft segment with a housing**: its own block in the line, with an inner axle that turns. One
  bearing, with an iron housing, iron's torque limit (never the weak link) and a cheap recipe (iron nuggets
  and a plank).
- Place it like a shaft, or **right-click a bearing onto an existing shaft** to swap it in (the shaft is
  given back).
- **Mount side** chosen automatically: below, then the sides, then above (hanging). The wrench cycles through
  the valid choices; the housing's foot or strap points at its mount.

### 7.3 Bevel gear

- One block, **L by default** (input and one output at 90°); the wrench toggles a **T** (a second output,
  opposite the first). Ratio 1:1; signs as in 4.3. The overlay shows direction arrows on each face.
- Wood (max ~64 RPM) and iron (max ~192 RPM). Counts as a support.

### 7.4 Gearbox

- **Inline:** input and output on one axis. **2:1** between its two marked faces (the large-gear face turns
  half as fast and carries double torque); the wrench flips it to 1:2. Direction is preserved.
- Power may flow either way; "input" is only a label for the ratio.
- Iron only, max ~192 RPM on its fast side. Counts as a support.

### 7.5 Pulleys and leather belt

- A **pulley** is a shaft segment in the line, **small** (ratio 1) or **large** (ratio 2, drawn 3 blocks wide;
  it needs its plane clear of solid blocks to place). Wooden in 0.1 (max ~96 RPM at the pulley); iron later.
  Not a support. One belt per pulley; several pulleys on one line shaft give several drops.
- **Belt:** right-click pulley A, then pulley B, with a **leather belt** item. The axes must be parallel, the
  offset between them perpendicular to the axis, the distance at most 16 blocks *(tune)*. There is no
  obstruction check in 0.1 (the belt is drawn as two straight runs). The belt costs 1 item per 4 blocks of
  distance, rounded up *(tune)*. The wrench toggles **crossed** (reverses direction). Breaking either pulley
  removes the belt and drops its items.
- **Ratio** = diameter A / diameter B (small 1, large 2).
- **Slip instead of break.** A belt carries at most **24 Nm** at its smaller pulley *(tune)*. Above that it
  **slips**: it squeals, the two sides become separate systems in one network, and the belt passes exactly its
  limit from the faster side (by ratio) to the slower, with extra friction. It **re-grips** when the relative
  speed falls under 0.5 RPM and the torque needed is under the limit. A belt never breaks: it is the period's
  natural torque limiter.

### 7.6 Clutch

- A shaft segment that **disengages while powered** by redstone; unpowered it is a plain shaft.
- Engaging shares momentum (6.5, including the opposite-direction shock); disengaging splits, and each side
  keeps its speed (3.3).
- Iron torque limit; counts as a support. A gradual, slipping engagement is left for later if the snap feels
  harsh.

---

## 8. Sources (0.1)

### 8.1 Crank

- **Hold** the use button on it to turn it; **sneak** while holding to turn the other way. Stops pushing the
  tick after release.
- Stall 16 Nm, top 32 RPM *(tune)*. Hunger cost about that of sprinting for the time spent cranking *(tune)*.
- One socket. If Horizon's hooks can't detect a held use, fall back to click-to-push (each click pushes for
  20 ticks, as in the spike).
- The copper golem (design doc 3.6) is a later, optional add-on to this source.

### 8.2 Water wheel

- A **single block with a large render**: a 3×3 vertical wheel around its hub, the axle out of both hub
  faces (either or both may connect). Placing needs the 8 positions around the hub in its plane to be air or
  water. No multiblock (that system is 0.2). The blades have no collision in 0.1.
- **Torque from flowing water** in those 8 paddle positions: 4 Nm per flowing block, falling water on the top
  row counting double (an overshot wheel), up to **32 Nm**; top speed **24 RPM** *(tune)*. Each block's
  contribution is signed by its flow direction along the wheel's rim, so opposing flows cancel. Still and
  source water give nothing. The flow is re-read on neighbour updates and every 40 ticks.
- Deliberately weak: one grinder runs at ~19 RPM, two at ~15, three stall below the grinder's minimum
  (with the numbers in section 14).
- Inertia 20: a heavy wheel smooths things a little.

---

## 9. Machines

### 9.1 The grinder (reference machine)

One socket, load 6 Nm while it has work *(tune)*, working range 12 to 96 RPM, inertia 4. Its recipes and
times belong to the items and machines spec.

### 9.2 Machine rules (all machines)

- **Inside its range,** work speed is proportional to RPM (twice as fast at 96 as at 48).
- **Below its minimum,** no progress, but it keeps pulling its full load while it has work: the struggle
  stays visible. It never lets go of the load below the minimum (that would flutter on and off).
- **Load above the sources' stall torque:** the network never starts. Overlay: "Stalled: load 30 Nm, sources
  give 24 Nm".
- Direction doesn't matter unless the machine's spec says so.
- Idle: friction only.

### 9.3 Interface for later parts

Every part type supplies: its connection faces, ratio rule, inertia, friction (`c`, `f`), speed limit(s),
torque limit (shaft segments), whether it supports, and per tick either a **source curve** (`S`, `V`,
direction) or a **load** (`L`). The steam engine is a source whose `S` follows steam pressure and the
throttle; the flywheel is a part with large inertia; the governor sets the engine's throttle from the
network's speed, with lag (steam spec). None of them needs a solver change.

---

## 10. On Horizon

### 10.1 Block entities

- **One block entity type per block** (spike lesson 1). Hooks name the type, not the block.
- **Shafts, bearings, gears, gearboxes, clutches and pulleys don't tick** (FINDINGS risk 1). Only sources,
  machines and the tachometer tick (`block_entity_ticks` → true only for them).
- Sprites in block entity drawing are named **without a namespace**; always check `cube`'s returned bool
  (spike lesson 2).
- Parts refuse pistons (push reaction "block"). Structure rotation rotates saved axes, sockets and mounts.

### 10.2 Membership without ticking

- A **level attachment** holds the part registry (position → type, axis, material, configuration, state)
  and the cached networks.
- Parts enter on `block_entity_on_load` and on placement, are marked unloaded on `on_chunk_unloaded`, and
  leave on break. Each change marks the affected networks dirty.
- Sources and machines report their torque or load for the tick from `block_entity_tick` into the server
  state, as the spike does.

### 10.3 The tick

In `level_tick_post`: rebuild dirty networks; check pause status (10.4); integrate each running system
(3.4); update damage timers, flows (6.3) and belt slip; then mark the block entities whose client-visible
state changed. Machines read the speed of their network from the server state in their next tick.

### 10.4 Pause outside simulation

- A network **pauses as a whole** if any of its parts is not simulated (`should_tick_blocks_at` not true) or
  not loaded. While paused: no integration, no timers, no machine progress; speed is kept.
- To keep this cheap, a network keeps the set of chunks it spans and checks one position per chunk, every
  10 ticks *(tune)* and on chunk load or unload.
- **Visually, a paused network keeps turning** at its last speed: the server simply doesn't send a stop.
  Looping sounds continue. Frozen shafts at the edge of view would look like a bug. (This settles the
  question the design doc 2.1 left to this spec.)

### 10.5 Saving and loading

- Each part saves its **local speed**, jam state and reason, warning timers, and configuration (mount, T,
  gearbox flip, belt partner and crossed flag, socket).
- **The first tick after a load runs before the network exists** (spike lesson 3): a part with no network
  keeps what it saved.
- A rebuilt network starts at the **inertia-weighted average** of its members' saved speeds (the same rule as
  joining), so a world reloads at the speed it was saved at.

### 10.6 Client sync and rendering

- `block_entity_update_tag` sends a small fixed set: local RPM (signed), axis, jam state, warning level
  (0 to 3, for rattle/creak/squeal intensity), sag flag, and configuration needed to draw (T, flip, belt
  partner, crossed).
- Send when the speed changes by more than max(0.5 RPM, 3%) *(tune)* or any other field changes. Never per
  tick.
- Rendering uses `spin` + `keep` (`rpm × 0.3` degrees per tick), with the phase formula from the Horizon book
  so a part doesn't jump when its speed changes and meshing gears stay meshed.
- Sagging spans get a visible offset; the wobble/twist animation uses whatever the renderer allows cheaply,
  checked during implementation (fallback: the offset plus particles).
- View distance and render bounds are raised for large pulleys and the water wheel.

---

## 11. Presentation

### 11.1 Tachometer

- Attaches to a face of any network part, like a button. It reads that part's network. It does not transmit
  and is not a support.
- Dial: needle at the current speed (local to the part it's on); **red** above the network's red line;
  **yellow** below the highest minimum among the network's working machines.
- **Comparator output** 0 to 15, linear from 0 RPM to the red line (15 = at or over it), direction ignored.
  Example: a comparator into a clutch sheds a machine line when the speed sags.

### 11.2 Wrench

- **Right-click:** configure (rotate socket, cycle bearing mount, toggle T, flip gearbox, toggle crossed
  belt), or **repair** if the part is jammed.
- **Sneak-right-click:** inspection overlay.
- Breaking works as normal; the wrench breaks FRAS parts quickly and they drop as items.

### 11.3 Inspection overlay (content, 0.1)

Speed and direction; red line and working range; load percent and "load X Nm / sources give Y Nm"; for a
shaft segment, torque carried versus limit ("carrying 40 / 64 Nm"); span status; for a source, its stall
torque and top speed and current input; for a machine, working / below range / idle; jam reason and repair
cost; warning timer if filling; "paused: part of this network is outside the simulated area" when paused.

### 11.4 Tooltips

One line; Shift for details, e.g. "Max torque 32 Nm. Needs support every 8 blocks." or "Max 64 RPM."

### 11.5 Sounds (cues; the sounds themselves belong to the UX spec)

Rattle (overspeed, quickening with the timer), creak (overload), squeal (belt slip), gear whine with speed,
crank ratchet, water wheel splash, a clunk on jam, a clank on repair.

---

## 12. Code layout and tests

- **`sim::rot`**, game-free, like Kid Mode's `logic.rs`: `graph` (parts, connections, systems, ratios, loop
  conflicts, spans), `solve` (integration, stiction, momentum sharing), `flow` (torque flow), `damage`
  (timers, jams), `belt` (grip and slip). No Horizon types.
- **Game layer:** blocks, block entities, the level attachment, hooks, rendering, sync, overlay.
- **Unit tests** at least for: spin-up and settling against the straight-line curve; coasting to a stop;
  several sources adding up; opposing sources; stiction (a load above stall never starts); gearbox ratios and
  torque doubling; bevel L and T signs in every orientation; consistent loops accepted, inconsistent loops
  jam the closing part (and the deterministic choice on load); momentum sharing on join and the
  opposite-direction shock; split keeping speeds; torque flow on a line, a tree and a parallel loop; shear and
  overspeed timers (fill, 5× rate, drain, cap); span lengths with mixed materials and sag effects; belt slip
  and re-grip; pause when any chunk is unsimulated and resume at the same speed; reload from saved speeds;
  no rebuild when nothing changed.

---

## 13. 0.1 exit criteria (in game)

Driven through the Horizon launcher and its MCP server, as in the spike:

1. A crank turns a line and a grinder; the line sags while grinding and coasts to a stop when released.
2. A water wheel in a stream runs one grinder well, two slowly, three below the minimum.
3. Gearing a water wheel up with two gearboxes overspeeds a wooden bevel gear: rattle, then jam; the cut-off
   side coasts; the wrench repairs it with a stick.
4. A long wooden line loaded past 32 Nm creaks and shears near the source; an iron trunk with wooden branches
   doesn't.
5. A 12-block wooden span sags and warns; a bearing on a fence fixes it; removing the fence drops the bearing.
6. Engaging a clutch between two lines turning opposite ways jams the clutch; an inconsistent gear loop jams
   the part that closed it.
7. A belt drop to a grinder slips under too much load and re-grips when the load drops; a crossed belt
   reverses.
8. A network crossing the simulation distance pauses (still turning visually) and resumes at the same speed;
   save and reload keep speeds and jam states.
9. Tachometer: needle, red line and comparator output behave as in 11.1.
10. Performance: 2,000 parts on one network cost under 0.1 ms of server time per tick at steady state; a
    rebuild of that network under 2 ms *(targets)*.

---

## 14. Starting numbers *(tune)*

| Part | Torque limit | Max RPM | Inertia | Friction `c` (Nm per RPM) | Support |
| --- | --- | --- | --- | --- | --- |
| Wooden shaft | 32 Nm | — | 1 | 0.01 | no |
| Iron shaft | 128 Nm | — | 1 | 0.01 | no |
| Bearing | 128 Nm | — | 1 | 0.01 | yes (needs a mount) |
| Clutch | 128 Nm | — | 2 | 0.02 | yes |
| Bevel gear, wood / iron | — | 64 / 192 | 2 | 0.02 | yes |
| Gearbox | — | 192 (fast side) | 4 | 0.04 | yes |
| Pulley small / large (wood) | belt 24 Nm | 96 | 2 / 4 | 0.02 | no |
| Crank | — | (top 32) | 2 | 0.02 | yes |
| Water wheel | — | (top 24) | 20 | 0.05 | yes |
| Grinder | — | 12 to 96 | 4 | 0.05 | yes |

Other values: constant friction `f` 0.05 Nm per part; crank stall 16 Nm; water wheel 4 Nm per flowing block,
max 32 Nm; grinder load 6 Nm; flywheel inertia ~200 (0.2); global cap 256 RPM; warning timer 5 s, 5× at 1.5×;
shock threshold 8 RPM; spans 8 (wood) / 16 (iron); belt 16 blocks max, 1 item per 4 blocks; snap-to-zero
0.5 RPM; flow recompute 5% / 4 ticks; sync threshold max(0.5 RPM, 3%); pause check every 10 ticks;
acceleration constant `k` set by the targets in 3.4.

---

## 15. Changes to the design doc

- 3.2: speed limits by part type (6.1): shafts, bearings and clutches have a torque limit instead of a speed
  range; gears, gearboxes and pulleys have a max only.
- 3.3: supports include every non-shaft part except pulleys; a sagging span halves the torque limit; the
  bearing is a shaft segment with one iron housing.
- 3.5: pulley belts slip at their torque limit instead of breaking; the clutch disengages while powered.
- 3.6: the water wheel is one block with a large render, no multiblock; the crank is held, not clicked.
- 9: a paused network keeps turning visually.
- New: friction on every part, the 256 RPM cap, machines as end points with one socket.
