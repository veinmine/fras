# Rust: subsystem spec

Spec 5 of the design doc's section 12. Version 1.0 (2026-11-25), scope: **milestone 0.3 (Regulation and
cooling)**. Builds on `Fire, rust and steam - design.md` v2.1 (section 5, and 9 on pausing), `specs/steam-and-heat.md`
(spec 2: scald areas, whose section 13 made steam a rust trigger), `specs/fluids-and-pipes.md` (spec 3: bursts,
which leaks reuse), `specs/rotational-network.md` (the jammed state) and `specs/grinder.md` (section 11:
flakes → pigment). Written together with `specs/regulation.md` (which holds the 0.3 exit criteria) and
`specs/cooling.md`. Where this spec is more precise than the design doc, this spec wins; section 13 lists what
changes in the design doc.

**Not yet reviewed.** Written in one go without an interview. Section 14 lists the open questions with the
assumption this spec makes for each.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 12 collects them.

---

## 1. Scope

**In 0.3:** which parts rust, the triggers, the four stages and their timing, what a leaking part does, red
oxide paint and pre-painted parts, scraping and repair with the wrench, rust flakes → pigment → paint, the rust
settings, looks, overlay lines.

**Designed for, built later:** the washer rusting iron nuggets into flakes (0.4, the automated paint supply);
steel parts (tier 2: decided with steel, probably rusting slower); lagging.

---

## 2. The rule

Rust is the consequence of poor planning, never of normal use (pillar 2, design doc 5):

- **Only iron parts rust**, and only from **outside**: water touching them, rain on them, steam blowing on them.
  What flows **inside** a part never matters.
- **Only the last stage does anything.** Before that, rust is a visible warning that takes in-game days.
- **Paint stops it for good.** A painted part, or one crafted pre-painted, never rusts.
- **Scraping undoes it.** The wrench returns any part to clean, and the flakes it drops are what paint is made
  from.

---

## 3. Which parts rust

| Rusts (iron) | Never rusts |
| --- | --- |
| Iron shaft, bearing, clutch, iron bevel gear, gearbox | Wooden shaft, wooden bevel gear, pulleys, belt, crank, water wheel |
| Iron pipe, manual valve, redstone valve, steam release, emergency valve, governor | Copper pipe, steam dome, copper boiler, gauges, sight glass (copper: visual oxidation only) |
| Iron boiler, storage tank (per block) | Firebox (brick), grinder (timber), bellows (wood and leather) |
| Steam cylinder, flywheel, pump | Copper radiator, drain |
| Heat exchanger, iron radiator, furnace damper | Tachometer, wrench, hand hammer |

Each rusting block tracks its own stage, so a boiler whose top is under the rain rusts on top only.

---

## 4. Triggers

A part is **exposed** when any of these holds at its position *(each checked when the part is visited, 5.1)*:

| Trigger | Check | Weight |
| --- | --- | --- |
| **Waterlogged** | its `waterlogged` state | 1 |
| **Touching water** | water (source or flowing) on any of its six faces | 1 |
| **Rain** | it is raining at the position above it and that position can see the sky (`is_raining_at`, `can_see_sky`; snowy and dry biomes don't count) | 1 |
| **Steam** | it lies in an active scald area: a vent's or burst's jet, a leak, a cylinder's exhaust puff, a steam cloud (spec 2, 8) | 2 |

- Weights don't add: an exposed part gains the **highest** weight that applies.
- Steam counts double because it is hot and wet: an exhaust puffing onto an iron pipe above a cylinder rusts it
  in half the time.
- Contents never count: an iron pipe carrying water through a dry room, a boiler full of water, a waterlogged
  copper pipe: none rust.

---

## 5. Stages and timing

### 5.1 How time is counted: random ticks

- Rusting parts take **vanilla random ticks**, as copper does (Horizon: block content `random_ticks` and the
  `random_tick` hook, both in the pinned SDK). At the default `randomTickSpeed` of 3, a block gets one about
  every 68 s, about **17.6 per in-game day**.
- Each random tick, an exposed part adds its weight (section 4) to its **rust count**. A part that isn't exposed gains
  nothing; the count never goes down by itself.
- **Stages** advance each time the count reaches **18** *(tune)* (about one in-game day of exposure), and the
  count restarts:

  | Stage | Look | Effect |
  | --- | --- | --- |
  | **Clean** | as crafted | none |
  | **Spotty** | orange specks | none |
  | **Rusty** | brown, flaking | none |
  | **Leaking** | dark, with drips or a jet | section 6 |

  So a part in water leaks after about **three in-game days**, a part in steam after about one and a half, and a
  part only in rain after however many days of rain that adds up to (rain falls a fraction of the time, so
  typically one to two weeks).
- Random ticks are random, so parts placed together drift apart by a stage now and then; the overlay gives the
  expected time (section 9). Random ticks happen only in chunks within the simulation distance, so **nothing
  rusts while nobody is around** (design doc 9) without any pause logic.
- **`/gamerule randomTickSpeed`** scales rusting, which is how the exit criteria speed it up.

### 5.2 Settings

On the FRAS settings page (Options > Mods > FRAS), next to explosion mode (spec 2, 14.5):

| Setting | Values | Default |
| --- | --- | --- |
| **Rust** | on / off | on |
| **Rust speed** | 0.5×, 1×, 2×, 4× (divides the count per stage: 36, 18, 9, 5) | 1× |

Off: no part gains rust; existing stages stay and still act (a leaking part keeps leaking until scraped).
Settings are client-side in the pinned Horizon build, as the 0.2 explosion setting is: a dedicated server stays
on the defaults (`docs/development.md`).

---

## 6. Leaking

Only the leaking stage has an effect, and what it does depends on the part (design doc 5: "a pipe loses fluid
and pressure and sprays steam; a machine jams"):

| Part | Leaking does |
| --- | --- |
| Iron pipe, valves, governor body, heat exchanger on a **steam** network; iron boiler block, steam tank block, steam cylinder | **Leaks steam:** a consumer on the routing tree venting 10 mB/s *(tune)*, route-limited, with a jet that scalds the block around it (the burst of spec 3, 4.4 at a lower rate). Pressure sags. |
| The same on a **water, hot water or exhaust** network; water tank block | **Drips:** a consumer taking 10 mB/s of what passes (or from the tank) and dumping it, with drip particles. Hot water and exhaust drips puff steam and scald like a leak. |
| Shaft, bearing, clutch, bevel gear, gearbox, flywheel, pump, governor (its spindle) | **Jams** (rotational spec 6.4) with the reason "rusted through". A rusted-through governor stops turning, so it opens wide (`specs/regulation.md`, 3.7). |
| Iron radiator | Gives no cooling. |
| Furnace damper | Sticks where it is. |

- The leak rate is per leaking block: a lying boiler with three leaking blocks vents 30 mB/s.
- A leak is also a steam trigger (4), so it rusts the iron next to it. Rust spreads through neglect.
- A leaking part never breaks further and never explodes. Its rated pressure is unchanged.
- **Repair** of a leaking part is scraping it (section 8), which also clears the jam.

---

## 7. Paint

### 7.1 Painting in place

- **Red oxide paint** is an item (a small tin, stacks to 16). **Right-click a clean iron part** with it: the part
  becomes **painted**, takes the red oxide look, and never rusts again (design doc 5, like waxing copper). One
  paint per block.
- **Only clean parts take paint** ("scrape it first"): paint over rust is refused, so a player can't freeze a
  rusty part in place.
- Paint is permanent; there is no way to strip it, and no reason to.
- Copper parts take honeycomb as in vanilla to stop their visual oxidation; paint does nothing on them.

### 7.2 Pre-painted parts

- **8 iron parts of one kind around 1 paint** in the crafting grid give 8 painted parts (design doc 5: "big
  builds never need painting in place"). One paint per eight parts instead of one per block, so painting at
  the crafting grid is the efficient way.
- Pre-painted parts are separate item variants (`fras:painted_iron_pipe` and so on, one per rusting item), so
  they stack apart and place as painted. Their tooltip says "Painted: never rusts".
- Multiblock parts (iron boiler, tank) paint per block, in place or pre-painted; a painted boiler block and an
  unpainted one still join (paint is not a material).

### 7.3 Look

Each rusting block has five looks: clean, spotty, rusty, leaking, painted. Painted is a flat red oxide with the
rivets picked out. The stage lives in the block state (`rust` = clean / spotty / rusty / leaking / painted)
where the model needs it; for the boiler and tank shells the `fras:shell` loader picks the sprite per block.

---

## 8. Scraping and repair

- **Wrench right-click on a rusted part scrapes it back to clean**, with a rasping sound and flakes falling. On a
  part that is spotty, rusty or leaking, scraping takes priority over the wrench's other right-click uses
  (connecting a pipe face, rotating, settings), so the click after it does the usual thing. On a clean or
  painted part the wrench works as before.
- **Drops rust flakes:** spotty 1, rusty 2, leaking 3 *(tune)*.
- **A leaking part also needs an iron plate** from the inventory (design doc 5). Without one, nothing happens
  and the overlay says "Needs 1 iron plate". Scraping a leaking part clears its leak or jam at once.
- Free in creative (no plate needed, flakes still drop).
- A rusty part that is also jammed for another reason (overspeed) is scraped first, repaired with the next
  click.
- Scraping resets the rust count: a part left where it rusted starts rusting again. The fix for a part that
  keeps rusting is to paint it once it is clean, or move it out of the water.

---

## 9. Wrench overlay and tooltips

- **Every rusting part:** its stage and, while exposed, what rusts it and the expected time to the next stage
  ("Rusty. Rusting: in water. Leaks in about 1 day"); "Painted: never rusts"; "Not exposed" otherwise. A leaking
  part: "Leaking 10 mB/s. Scrape with an iron plate to repair" or "Rusted through: jammed".
- **Any network part** (spec 2, 11 and spec 3, 9): leaks on this network and their total.
- Tooltips on iron items: "Iron: rusts in water, rain or steam unless painted." On painted items: "Painted:
  never rusts." On paint: "Right-click a clean iron part. Eight parts around one tin paint all eight."
- Sounds (UX spec): a drip for water leaks, a thin hiss for steam leaks, the scrape rasp.

---

## 10. Paint supply

| Step | How | Result |
| --- | --- | --- |
| Rust flakes | Scraping (8) | 1 to 3 per scrape |
| Red oxide pigment | Grinder: 1 rust flake → 1 pigment (a `fras:grinding` recipe, `specs/grinder.md` 11) | |
| Red oxide paint | Shapeless: 2 pigment + 1 water bottle → 2 paint | the bottle is used up |

- In 0.3 flakes come **only from scraping**, so the first paint comes from rust: a player who wants paint early
  rusts something on purpose (an iron part in water rusts to "rusty", 2 flakes, in about two days), then
  scrapes it.
- In 0.4 the washer slowly rusts iron nuggets in water into flakes, the hands-off supply (design doc 5).

---

## 11. On Horizon and tests

- **Random ticks** (5.1): declared on every rusting block. The `random_tick` hook gets the level writable, so it
  checks exposure, updates the count in the block entity and, on a stage change, the block state.
  Painted parts don't need random ticks; if `random_ticks` can't be limited per state, the hook returns at once
  for painted ones.
- **Exposure checks** are cheap reads: the part's own state, six neighbours' fluid state, `is_raining_at` and
  `can_see_sky` for the position above, and a lookup in the steam step's set of active scald positions (kept
  per level, rebuilt each steam step from vents, bursts, leaks, exhausts and clouds).
- **Leaks** are consumers on the routing tree like bursts (spec 3, 4.4); a stage change to or from leaking marks
  the network dirty. A leaking rotational part jams through the existing damage path with a new reason.
- **Saved:** rust count and stage (block entity and block state); painted in the block state.
- **Performance:** one random tick per block per ~68 s; 2,000 rusting parts make about 30 checks a second.
- **`sim::rust`**, game-free: exposure weight from the four triggers, count and stage advance, rust speed, what a
  leaking part does by kind, scraping (drops, plate, reset), paint rules. **Unit tests** at least for: each
  trigger alone and the highest weight winning; contents never counting; stage advance at 18 and at each rust
  speed; rust off; leak behaviour per part kind; paint only on clean; pre-painted variants; scraping drops and
  the plate rule; scraping a leaking shaft clears its jam; save and reload of stage and count.

---

## 12. Starting numbers, recipes and terms *(tune)*

Count per stage 18 (≈ 1 in-game day of exposure at `randomTickSpeed` 3); weights 1 (water, waterlogged, rain),
2 (steam). Rust speed 0.5× / 1× / 2× / 4× → 36 / 18 / 9 / 5. Leak 10 mB/s per leaking block. Flakes: spotty 1,
rusty 2, leaking 3; leaking needs 1 iron plate. Pigment 1 per flake. Paint 2 per 2 pigment + 1 water bottle;
1 per block in place, 1 per 8 pre-painted.

### 12.1 Recipes (0.3)

| Result | Recipe |
| --- | --- |
| Red oxide pigment | grinding: 1 rust flake → 1 pigment |
| 2 red oxide paint | shapeless: 2 red oxide pigment + 1 water bottle |
| 8 painted *part* | shaped `XXX` / `XPX` / `XXX` (8 of one iron part around 1 paint), one recipe per rusting item |

### 12.2 Terms for the translation dictionary

Proposed Polish names (**mod** terms): Rust: Rdza. Stages: czysty, nalot rdzy (spotty), zardzewiały, cieknący.
Rusted through: przerdzewiały. Rust flakes: Płatki rdzy. Red oxide pigment: Pigment żelazowy. Red oxide paint:
Minia żelazowa. Painted: pomalowany (Pomalowana żelazna rura). Scrape: zeskrob(ać). Leak: wyciek.

---

## 13. Changes to the design doc

- 5: rust time is counted in random ticks (one stage ≈ one in-game day of exposure at default speed; steam
  counts double); the leaking stage's effect is listed per part (steam parts leak 10 mB/s, water parts drip,
  rotational parts jam, radiators stop cooling, dampers stick); paint only on clean parts; pre-painted parts are
  8 around 1 paint; scraping takes priority over the wrench's other clicks on a rusted part; paint is 2 pigment +
  a water bottle.
- 9: rust needs no pause rule: random ticks only happen in simulated chunks.

---

## 14. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

1. **R1. Random ticks count time** (5.1), so rust is a little random per part and scales with
   `randomTickSpeed`. *Alternative:* a deterministic timer per part, visited by the level tick, so a whole pipe
   run rusts in step.
2. **R2. Timing:** about one in-game day of exposure per stage, leaking after about three days in water and
   about a day and a half in steam (5.1). Slower or faster?
3. **R3. What leaking does** per part (6): steam parts leak 10 mB/s and scald, water parts drip, rotational parts
   and the pump jam, iron radiators stop cooling, dampers stick. Is a leaking cylinder better as a jam
   (engine stops) than a leak?
4. **R4. Paint only on clean parts** (7.1), so rust must be scraped before painting.
5. **R5. Pre-painted:** 8 parts around 1 paint give 8 painted parts as separate item variants (7.2), eight times
   cheaper than painting in place.
6. **R6. Scraping takes priority on a rusted part** (8): the first wrench click on a rusty pipe scrapes it
   instead of toggling a face. *Alternative:* hold right-click for a second to scrape, as the crank is held.
7. **R7. No flakes before the washer** except from scraping (10): early paint means rusting something on purpose.
   *Alternative:* an interim recipe (iron nuggets + a water bucket → flakes) until the washer arrives in 0.4.
8. **R8. Paint recipe:** 2 pigment + 1 water bottle → 2 paint. Vanilla has no oil; any better binder?
9. **R9. Which parts rust** (3): the grinder (timber) and bellows (wood) don't; the steam dome is copper and
   doesn't. Anything to move between the columns?
