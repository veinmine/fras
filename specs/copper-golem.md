# Copper golem crank: prototype spec

An optional part of milestone 0.4. Version 1.0 (2026-11-26). Builds on `Fire, rust and steam - design.md` v2.1
(section 3.6), `specs/rotational-network.md` (section 8.1, the crank) and `spike/FINDINGS.md` (the entity hooks
row, risk 5).

**Reviewed by Marcin on 2026-11-26** (decisions in the last section). Section 9 lists the open questions with the assumption this spec makes for each.

**This is a time-boxed prototype.** The design doc says: prototype the arm animation for a day; drop the golem if it
needs a mixin. Section 7 says what "done" and "dropped" mean.

---

## 1. What it is

A copper golem can be **assigned to a crank**. The crank holds it in place, its arm turns the handle, and the crank
produces torque without a player. A golem in the weather oxidises (vanilla behaviour) and, once it turns into a statue,
stops; waxing prevents that. It's a cute, weak, permanent power source for a pump or a small belt: the reward for
building one, and a reason to wax it.

---

## 2. What the pinned SDK offers

Checked in the pinned Horizon build's API (`.horizon/sdk`) while writing this spec, not yet in game:

- **Behaviour:** `EntityPart::register_behaviors` adds brain behaviours and goals to a species; `entity_interact` hears a
  right-click on an entity, and leads are entity interactions too (the spike's findings).
- **Drawing:** `render_living_pre` runs before any living entity is drawn and can **hide a named part** of the vanilla
  model for that frame (`hidden.hide_named(name)`, the doc's example is a villager's nose), or cancel the whole entity and
  draw it itself. The same hook gets the block entity drawing helpers, including `model_part`, which draws a model part
  with its pivot and rotation. So the golem's own arm can be hidden and a turning arm drawn in its place, every frame,
  posed from the crank's phase.
- **Keyframe animations** (`register_json_animation_types`, the `AnimationHolder` type) exist too, for a mod's own model;
  likely not needed.

This is better than the spike knew: the uncertain part (animating the arm on the vanilla model) looks doable with hooks.
It is still unproven until it runs, hence the prototype.

---

## 3. Assigning a golem

- **Lead the golem to the crank and right-click the crank** with the lead in hand: the golem is assigned (the lead is
  used up into a hitch, as on a fence) and walks to the crank's **working spot**, the block in front of the crank's handle
  side, facing it.
- **Or right-click the golem with a wrench, then the crank,** for a golem that is already standing next to it. *(Only if
  leads turn out awkward in the hooks.)*
- One golem per crank. The crank's working spot must be a block a golem can stand in.
- **Unassign:** break the hitch (punch it) or break the crank; the golem goes back to its normal behaviour.
- While assigned, a brain behaviour keeps it at the working spot, facing the crank, and stops its other activities
  (sorting items into chests, its vanilla job). If something pushes it off the spot it walks back.

---

## 4. Turning the crank

- An assigned golem at its spot turns the crank **continuously** as if a player held it, at a lower torque:
  **stall 8 Nm, top 16 RPM** *(tune)* (a player gives 16 Nm and 32 RPM, rotational spec 8.1). It always turns
  the crank forward (a player turns it backwards by sneaking, rotational spec 8.1).
- A golem gets **no hunger** and never tires. It's slow, permanent and free: enough for a pump (minimum 8 RPM), a short belt,
  or a washer at the bottom of its range, not for a grinder line.
- **Several golems on several cranks** on one network add up like any sources.
- **Oxidation:** vanilla's copper golem oxidises through its stages over time and becomes a **statue** at the last stage. FRAS
  follows vanilla: full torque until it is a statue, then **nothing** (the crank stops, the overlay says "Golem oxidised:
  scrape it with an axe or wax it"). A **waxed** golem never oxidises. Scraping a statue back with an axe (vanilla) restores it
  and it carries on. The design doc's "in the rain" is vanilla's general oxidation; FRAS doesn't add a rain rule (question CG2).

---

## 5. Look and sound

- The golem stands at the crank, body facing it; its **right arm** (the one nearest the handle) is hidden from the vanilla
  model and drawn by FRAS rotating about the shoulder, following the crank's angle, so the hand stays on the handle. The other
  arm and the head keep their vanilla animation (it looks around).
- The crank already draws its handle from its phase (rotational spec 10.6); the arm reads the same phase on the client.
- Its vanilla footstep and copper sounds, plus the crank's own creak.

---

## 6. On Horizon

- **Server:** a brain behaviour (`register_behaviors` on `minecraft:copper_golem`) that holds an assigned golem at its spot; the
  crank's block entity stores the golem's UUID and asks each tick whether it is present, at its spot and not a statue; the crank
  becomes a source with the golem's curve while it is.
- **Client:** the crank's update tag carries the golem's entity id and the crank's speed; `render_living_pre` for that golem
  hides its right arm and draws a posed copy with `model_part`.
- **Saved:** the golem's UUID on the crank; the assignment on the golem (an attachment), so both survive reload; a golem whose
  crank is gone on load is released.

---

## 7. The prototype: done or dropped

**Time box: one day** of work, at the start of 0.4 (before belts), so dropping it changes nothing else.

**Done** when, in game: a golem is assigned with a lead, walks to the crank, turns it (the network gets the golem's torque), its
arm visibly follows the handle without clipping through it, it stays through save and reload, and it stops as a statue. Then
it ships in 0.4 and exit criterion 11 (`specs/progression.md` 9) applies.

**Dropped** if any of these holds at the end of the day: hiding or drawing the arm needs a mixin; the brain behaviour can't keep
the golem in place without a mixin; or the arm can't be made to look right with `model_part`. Then: a note in `docs/development.md`
saying why, the hooks that were missing, and a line on the Horizon watch if an upstream hook would fix it. The design doc keeps
the golem as a later idea.

**A fallback**, if only the animation fails: the golem is hidden (`render_living_pre` cancel) and the crank block draws a golem-like
figure of its own with its block entity helpers. Only if that takes under half a day; otherwise drop (question CG3).

---

## 8. Changes to the design doc

- 3.6: an assigned golem gives 8 Nm stall and 16 RPM top; it stops when vanilla oxidation turns it into a statue; FRAS adds no
  rain rule of its own.

---

## 9. Open questions and assumptions

Each item states what this spec assumes. Reply by number; anything skipped stays as proposed.

**Reviewed by Marcin, 2026-11-26:** all accepted as proposed.

1. **CG1. A golem gives half a player's crank** (8 Nm, 16 RPM, section 4). *Alternative:* the same as a player, or a quarter.
2. **CG2. Oxidation is vanilla's,** not a FRAS rain rule (4). *Alternative:* golems also oxidise faster in rain, as the design
   doc's wording suggests.
3. **CG3. If only the arm animation fails,** try a half-day fallback where the crank draws its own golem figure (7); otherwise
   drop. *Alternative:* drop as soon as the vanilla model can't be posed.
4. **CG4. Do the prototype first** in 0.4, before belts (7). *Alternative:* last, if time allows.
