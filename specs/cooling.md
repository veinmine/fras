# Cooling and hot water: subsystem spec

Spec 3 of the design doc's section 12, second part. Version 1.0 (2026-11-25), scope: **milestone 0.3
(Regulation and cooling)**. Builds on `Fire, rust and steam - design.md` v2.1 (section 4.5), `specs/fluids-and-pipes.md`
(spec 3, the 0.2 part, whose section 10 reserved the hooks used here), `specs/steam-and-heat.md` (spec 2: the
boiler step, the engine's steam draw), `specs/grinder.md` (section 11 reserved its water cooling) and what 0.2
shipped (`docs/development.md`). Written together with `specs/regulation.md` (which holds the 0.3 exit criteria)
and `specs/rust.md`. Where this spec is more precise than the design doc, this spec wins; section 15 lists what
changes in the design doc.

**Not yet reviewed.** Written in one go without an interview. Section 16 lists the open questions with the
assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 14 collects them.

---

## 1. Scope

**In 0.3:** exhaust steam as a pipe content, the **heat exchanger** (which is also the condenser), copper and
iron **radiators**, **hot water** as a fluid and pipe content, hot water as boiler feed, the **drain**, **water
cooling** for the grinder, and the fluid capability for other mods that 0.2 left over.

**Designed for, built later:** water cooling for the 0.4 machines (same rule, section 9.3); lagging; a hot water
bucket and world form; heat loss to the surroundings (not in tier 1).

---

## 2. Contents

0.3 has four pipe contents. One content per network still holds, decided by endpoints (spec 3, 3.2).

| Content | Pressure | Comes from | Goes to |
| --- | --- | --- | --- |
| Water | none (flow only) | pumps, water tanks, heat exchangers cooling hot water | boilers, tanks, cooled machines, drains |
| **Hot water** (new) | none | heat exchangers condensing exhaust, cooled machines, hot water tanks | boilers, tanks, heat exchangers, drains |
| Steam | one per network | domes, steam tanks | cylinders, vents, governors' downstream |
| **Exhaust steam** (new) | none | cylinder tops | heat exchangers only |

Exhaust steam is kept apart from live steam so that it can never reach a boiler, a cylinder inlet or a steam
tank (spec 3, 10).

---

## 3. The closed loop

### 3.1 Open and closed

- **Open loop** (0.2): pump → boiler → engine → exhaust vented at the cylinder. Every mB of steam is a mB of
  fresh water heated from 20 °C.
- **Closed loop** (0.3): pump → boiler → engine → **exhaust pipe** → **heat exchanger** → **hot water** → back to
  the boiler. Each mB comes back at 90 °C, the pump only makes up for vents and leaks, and the cylinder stops
  puffing and scalding.

### 3.2 Why it saves fuel

With spec 2's game units, boiling 1 mB costs 25 kJ and warming it costs 0.1 kJ per °C:

| Feed | Warming | Boiling | Total per mB |
| --- | --- | --- | --- |
| Water at 20 °C | 8 kJ | 25 kJ | 33 kJ |
| Hot water at 90 °C | 1 kJ | 25 kJ | 26 kJ |

So a boiler on hot water feed makes **27% more steam from the same fire**, or the same steam on **21% less
fuel**. The saving shows as fuel only when a damper (`specs/regulation.md`, 4) matches the fire to the steam;
without one the firebox burns at full rate and the gain shows as more pressure. The 0.3 exit criterion asks for
at least 15% less coal in game (`specs/regulation.md`, 11, item 8).

### 3.3 Machine heat

Cooled machines (section 9) turn water into hot water as they work, so their waste heat preheats boiler feed
(design doc 4.5). Their flow is small next to an engine's, so this is a bonus, not a power source.

---

## 4. Exhaust steam

- A steam cylinder's **top face** becomes a pipe outlet (spec 2, 13). With a pipe connected there, it is an
  **exhaust source** that puts out, each step, exactly the steam the cylinder drew that step (1 mB of steam,
  1 mB of exhaust).
- **Flow only:** exhaust has no pressure and no capacity; pipes don't burst on it. Copper pipe carries it as
  well as iron (throughput limits still apply).
- **What can't be delivered vents at the cylinder**, as in 0.2: a puff per stroke, the scald above the
  cylinder (spec 2, 8) and a rust trigger (`specs/rust.md`). So an exhaust pipe to nowhere, a thin pipe, or too
  little cooling all show at the cylinder, with the overlay naming the reason ("exhaust: 28 of 64 mB/s
  condensed, the rest vents").
- No back pressure in tier 1: an engine's power doesn't depend on its exhaust.
- **Hot pipes:** a pipe that carried exhaust in the last step burns on touch like a steam pipe (spec 2, 8).
  Hot water pipes don't burn.
- Exhaust endpoints: cylinder tops (source), heat exchanger inlets (consumer). A connection that would put
  exhaust and anything else on one network is refused, as for steam and water.

---

## 5. Heat exchanger

### 5.1 Block

- One iron block on a pipe axis: an **inlet** at one end, an **outlet** at the other (arrows on the texture),
  set by placement (the inlet faces the player's side, as a hopper's output faces away) and flipped with the
  wrench. Its four side faces take radiators (section 6). Waterloggable.
- It **takes heat out of what flows through it**:

  | In | Out | Heat removed |
  | --- | --- | --- |
  | Exhaust steam | Hot water (condensing) | 26 kJ per mB |
  | Hot water | Water (cooling) | 7 kJ per mB |

- The design doc's **condenser** is a heat exchanger fed with exhaust: the overlay and tooltip call it
  "condensing". There is no separate condenser block (question C1).
- It rusts unless painted (`specs/rust.md`). A waterlogged exchanger cools best and rusts fastest: paint it.

### 5.2 Cooling power

| Source | Power *(tune)* |
| --- | --- |
| The exchanger in air | 250 kW |
| The exchanger **waterlogged** | 500 kW |
| Each copper radiator on a side face | +125 kW |
| Each iron radiator on a side face | +75 kW |

- So one waterlogged exchanger with four copper radiators removes 1000 kW: it condenses **about 38 mB/s**, close
  to the 40 mB/s one coal firebox boils. Rule of thumb for the player: **one fully fitted, waterlogged exchanger
  per firebox.** In air with four copper radiators: 750 kW, about 29 mB/s.
- On hot water the same exchanger cools 1000 / 7 ≈ 143 mB/s, within its pipe throughput.
- Capacity in mB/s = power / heat per mB, capped by its throughput (200 mB/s, iron valve body).

### 5.3 Several exchangers

**Several exchangers on one exhaust line, in parallel, add up:** each is a consumer whose demand is its capacity,
and routing (spec 3, 5) splits the exhaust between them in proportion. In series, the second exchanger sees the
first one's outlet content (hot water) and cools it further to water. No extra rule.

### 5.4 Outlet

- The outlet is a **source** of the converted content, putting out what the exchanger took in that step. Its
  content follows the inlet's: an exchanger on an exhaust line gives hot water, on a hot water line water, on an
  empty inlet nothing.
- **If the outlet can't take it** (no pipe, the boiler full, a network of another content), the exchanger takes
  nothing in, and the inlet backs up: exhaust vents at the cylinder, hot water backs up to whatever made it.
  The exchanger holds a 100 mB buffer *(tune)* so step order doesn't matter. The overlay says "outlet full" or
  "outlet carries water".
- The outlet face never mixes: if its network already has another content, the exchanger stops ("outlet carries
  water") rather than refusing the connection, since its own content can change when the inlet changes.

### 5.5 Look and sound

A riveted iron box with fins and two flanges; condensate drips from the outlet flange and faint steam curls off
the fins while condensing. A low gurgle while working; a bubbling simmer when waterlogged.

---

## 6. Radiators

- A thin finned panel attached to a **side face of a heat exchanger** (not its inlet or outlet), like a gauge.
  Up to four per exchanger. A radiator on anything else does nothing ("not on a heat exchanger").
- **Copper radiator:** +125 kW, never rusts (visual oxidation only, waxable). **Iron radiator:** +75 kW, cheaper
  (iron bars), rusts unless painted; leaking, it gives no cooling (`specs/rust.md`).
- Radiators work in air; waterlogging a radiator neither helps nor hurts (the exchanger's own waterlogged bonus
  is what counts).
- Not a pipe part, no block entity work: the exchanger counts its radiators when its network is rebuilt (a
  radiator placed or broken marks it dirty).

---

## 7. Hot water

### 7.1 The fluid

- **Hot water** is a FRAS fluid at a fixed **90 °C**, with no world form and no bucket in 0.3 (the spike: a mod
  fluid with no world form, like steam). It never cools by itself: tier 1 has no heat loss, so a tank of hot
  water stays hot.
- Measured in mB like water. 1 mB of exhaust condenses into 1 mB of hot water.

### 7.2 Boilers

- A boiler face (not the top) is a **feed** endpoint that takes **water or hot water**, whichever its network
  carries. It no longer sets a network's content by itself: a network of only boiler faces and pipes is empty
  until a source joins (spec 3, 3.2 amended). A boiler can be fed by a water network on one face and a hot
  water network on another.
- Hot water mixes in at 90 °C (spec 2, 4.1 step 5, `t_feed` = 90). It doesn't stop the boil the way cold feed
  does.

### 7.3 Tanks

A storage tank can hold hot water (4000 mB per block, as water). An empty tank takes the content of the first
network it is connected to, as in 0.2. Hot water tanks can't rupture.

### 7.4 Routing order

Spec 3, 5.2's three passes become, each with the capacity left over from the one before:

1. Hot water sources (exchangers, cooled machines) → boilers
2. Hot water tanks → boilers
3. Water sources (pumps, exchangers cooling hot water) → boilers, then cooled machines
4. Water tanks → boilers, then cooled machines
5. Remaining flow from all sources → tanks of its content
6. Remaining flow → drains

So boilers come first and prefer hot feed, tanks store surplus, and drains only take what nothing else wants.
A cooled machine takes water after the boilers on its network (a machine on its own cooling line is the usual
build).

### 7.5 Other mods

Boilers and water or hot water tanks expose their fluid through the `FLUIDS` capability (spike: `ModFluidTanks`),
which 0.2 left for 0.3: other mods' pumps can fill them, and drain tanks. Hot water is visible to other mods as
`fras:hot_water`. Steam and exhaust are not exposed.

---

## 8. Drain

- A small grated box with one pipe face: a **consumer** of water or hot water that dumps whatever reaches it,
  up to 200 mB/s *(tune)*. It takes only surplus (7.4, pass 6).
- Hot water leaves with **steam puffs** and a hiss (design doc 4.5); water with a splash. Neither scalds.
- Copper: it never rusts.
- Use: dumping a machine's hot water when the boiler is full, or emptying a line. A drain on an exhaust line
  refuses to connect ("a drain takes water or hot water; exhaust needs a heat exchanger").

---

## 9. Machine cooling

### 9.1 The grinder

- The grinder (`specs/grinder.md`) gets a **cooling inlet** on the face opposite its socket and a **hot water
  outlet**: whichever of its two remaining side faces has a pipe (either). Pipes on those faces don't stop
  hoppers inserting from the top.
- **Water use:** while it has work, 0.1 mB/s per RPM *(tune)* (9.6 mB/s at 96 RPM, 12 mB/s at 120), all of it
  out of the outlet as hot water.
- **Cooled** means: it received its full water demand this step and its 100 mB hot water buffer *(tune)* isn't
  full. Then its working range is **12 to 128 RPM** instead of 12 to 96 (the hook `specs/grinder.md`, 3 left for
  this) and its speed bar runs to 128.
- **Losing cooling** while running above 96 RPM starts its overspeed warning (rotational spec 6.2) and the
  overlay says why ("cooling lost: no water" / "hot water can't get out"). Restoring the water in time stops the
  warning; otherwise it jams. Failure from bad planning: the drain or a heat exchanger is what keeps a cooling
  loop flowing when the boiler is full.
- An idle grinder takes no water.

### 9.2 The cooling loop

Typical builds, all falling out of the rules above:

- **To the boiler:** pump → grinder → hot water → boiler. Preheated feed, no waste.
- **Closed:** grinder → hot water → heat exchanger → water → back to the grinder, with a pump for start-up.
- **Dump:** pump → grinder → drain.

### 9.3 Later machines

Any machine spec can opt in with the same rule: an inlet, an outlet, water per RPM while working, a raised
maximum while cooled, a buffer. The 0.4 machine specs decide which do.

---

## 10. Routing changes (spec 3, section 5)

- New endpoints:

  | Endpoint | Content | Role |
  | --- | --- | --- |
  | Cylinder top | exhaust | source |
  | Heat exchanger inlet | exhaust or hot water | consumer, demand = capacity |
  | Heat exchanger outlet | hot water or water, from the inlet | source |
  | Boiler face | water or hot water (feed, sets no content) | consumer |
  | Grinder cooling inlet | water | consumer |
  | Grinder hot water outlet | hot water | source |
  | Drain | water or hot water | consumer, surplus only |
  | Tank face | its content, including hot water | source and consumer |

- Exhaust networks route like water networks (no pool, no pressure): sources' flow this step goes to consumers
  along the widest-path tree, narrowest part caps, proportional split, loops add nothing. What isn't delivered
  returns to its source (vents at the cylinder).
- The governor's throttle factor applies only on steam networks (`specs/regulation.md`, 3.2).

---

## 11. Wrench overlay, tooltips, sounds

- **Heat exchanger:** inlet content, mode ("condensing" / "cooling"), power and where it comes from ("250 kW in
  air + 2 copper radiators = 500 kW"), capacity and flow ("19 / 19 mB/s"), outlet state ("outlet full").
- **Radiator:** material, kW, "not on a heat exchanger", rust stage.
- **Cylinder** (added to spec 2, 11): exhaust piped or vented, how much condensed.
- **Pipe** (added): exhaust and hot water contents.
- **Boiler** (added): feed temperature ("feed 90 °C hot water, 12 mB/s").
- **Grinder** (added): cooled or not, water flow, hot water buffer, range.
- **Drain:** flow dumped.
- Tooltips: "Condenses exhaust into hot water, or cools hot water. Radiators and waterlogging help." "One fully
  fitted, waterlogged exchanger handles one firebox." "+125 kW on a heat exchanger." "Dumps water or hot water
  nothing else takes."
- Sounds (UX spec): condensing gurgle, drain hiss and splash, cooled grinder's water rush.

---

## 12. On Horizon

- **Heat exchangers, radiators and drains don't tick** (non-ticking block entities, like pipes); the steam step
  handles them. The grinder already ticks.
- Waterlogging: FRAS blocks have a `waterlogged` state already (shafts); 0.2 left the pump's waterlogging out,
  so the exchanger's is the first that matters. If waterlogged state turns out not to work for a FRAS block, the
  bonus falls back to "a water block touching any side face".
- Hot water and exhaust steam are mod fluids with no world form (spike: content declarations).
- Saved: exchanger facing and buffer; grinder water buffer; tank hot water as tank content. Exhaust holds
  nothing.

---

## 13. Code layout and tests

- **`sim::fluid`** gains the two contents and the new endpoints; **`sim::fluid::exchange`**: power from
  waterlogging and radiators, capacity per content, conversion, the outlet buffer; **`sim::fluid::cool`**: machine
  cooling demand, cooled state and range.
- **Unit tests** at least for: the 21% (33 vs 26 kJ) fuel saving through a dampered boiler step; exhaust
  undelivered vents at the cylinder; an exchanger's power from air, waterlogging and each radiator; capacity on
  exhaust and on hot water; two exchangers in parallel split by capacity; series gives water; outlet blocked
  stops intake; a boiler on hot water feed doesn't stop boiling; boiler faces set no content; the pass order of
  7.4 (boilers first, hot feed first, drains last); refused mixes (exhaust with water, a drain on exhaust); a
  grinder cooled, losing cooling on no water and on a full buffer; save and reload of buffers and hot water tanks.

---

## 14. Starting numbers *(tune)*

Hot water 90 °C. Heat removed: exhaust → hot water 26 kJ/mB, hot water → water 7 kJ/mB. Heat exchanger 250 kW in
air, 500 waterlogged, throughput 200 mB/s, buffer 100 mB. Radiator copper +125, iron +75 kW, four per exchanger.
Drain 200 mB/s. Grinder cooling 0.1 mB/s per RPM while working, buffer 100 mB, cooled range 12 to 128 RPM. Tank
block 4000 mB hot water.

### 14.1 Recipes (0.3)

| Result | Recipe |
| --- | --- |
| Heat exchanger | shaped `PCP` / `C C` / `PCP` (4 iron plates, 4 copper pipe) |
| 2 copper radiator | shaped `BBB` / `BBB` (6 copper bars) |
| 2 iron radiator | shaped `BBB` / `BBB` (6 iron bars) |
| Drain | shapeless: copper pipe + iron trapdoor |

The heat exchanger needs plates; radiators and the drain are entry recipes. Grinder cooling needs no new item.

### 14.2 Terms for the translation dictionary

Proposed Polish names (**mod** terms unless noted): Heat exchanger: Wymiennik ciepła. Condensing: skraplanie.
Copper / iron radiator: Miedziana / Żelazna chłodnica. Hot water: Gorąca woda (already in the dictionary).
Exhaust steam: Para wylotowa. Drain: Spust. Cooling: chłodzenie. Closed loop: obieg zamknięty.

---

## 15. Changes to the design doc

- 4.5: exhaust steam is its own pipe content (cylinder tops to heat exchangers only); the heat exchanger is
  the condenser (no separate block); hot water is fixed at 90 °C with no world form or bucket; boiler faces take
  water or hot water; drains take only surplus; one fully fitted waterlogged exchanger condenses one coal
  firebox's steam.
- 4.5: machine cooling: the grinder takes 0.1 mB/s per RPM while working and runs to 128 RPM while cooled;
  losing cooling above 96 RPM is an overspeed.
- 13: the condenser leaves the block list; radiators are copper and iron.

---

## 16. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

1. **C1. No separate condenser block:** a heat exchanger fed with exhaust is the condenser (5.1). *Alternative:*
   a condenser block for exhaust and the heat exchanger for hot water only, one more block with the same
   numbers.
2. **C2. The fuel saving is 21%** with spec 2's units (3.2). Enough of a reward? Raising it means a bigger warming
   cost per °C, which would also slow 0.2's cold start.
3. **C3. Cooling scale:** one fully fitted, waterlogged exchanger condenses one coal firebox's steam (5.2), so a
   two-firebox engine needs two. Too many blocks, or right for "the reward"?
4. **C4. Undeliverable exhaust vents at the cylinder** (4), with no back pressure. *Alternative:* back pressure
   that costs the engine torque.
5. **C5. Hot water has no world form or bucket** in 0.3 (7.1). A bucket would let players carry it to a tank by
   hand.
6. **C6. Boiler faces take either water or hot water** and set no content (7.2), so a boiler can have both feeds.
7. **C7. Grinder cooling:** inlet opposite the socket, outlet either remaining side (9.1), 0.1 mB/s per RPM,
   cooled to 128 RPM. Losing cooling above 96 is an overspeed warning, then a jam.
8. **C8. Radiators:** copper +125 kW and never rusts; iron +75 kW from cheap iron bars and rusts. Different
   enough to both be worth making?
