# Grinder: machine spec

Part of spec 6 (items and machines) in the design doc's section 12, written ahead of the rest for the one
machine milestone 0.1 needs. Version 1.0 (2026-11-23), scope: **milestone 0.1 (Mechanical)**.
Builds on `Fire, rust and steam - design.md` v2.1 (section 6), `specs/rotational-network.md` (sections 4.1, 6,
9 and 14, which already define the grinder's socket, speed range, load, inertia, jam and repair) and
`spike/FINDINGS.md`. Where this spec is more precise than the design doc, this spec wins; section 13 lists what
changes in the design doc and the rotational network spec.

Numbers marked *(tune)* are starting values for balancing, not commitments. Section 12 collects them.

---

## 1. Scope

**In 0.1:** the grinder block, its recipe type and recipes, three crushed ore items and their smelting
recipes, its GUI, hopper and in-world input, downward output, comparator output, look and sound.

**Designed for, built later (section 11):** water cooling (0.3), rust flakes → pigment (0.3), belt input and
output (0.4), the washer as the next step for crushed ore (0.4), Recipes viewer pages (when Horizon's port
lands).

**Already defined by the rotational network spec, not repeated here:** one shaft socket and no pass-through
(4.1), working range 12 to 96 RPM (6.1), overspeed and jam (6.2, 6.4), repair with 2 iron nuggets and 1 flint
(6.6), machine rules below the minimum and stalling (9.2), inertia 4 and friction (14).

---

## 2. Recipes (0.1)

| Input | Output | Revolutions | Load |
| --- | --- | --- | --- |
| Raw iron / raw copper / raw gold | 1 crushed iron / copper / gold, plus 50% chance of a 2nd | 4 | 6 Nm |
| Block of raw iron / copper / gold | 9 crushed ore, plus nine 50% rolls (avg 13.5) | 36 | 6 Nm |
| Cobblestone, cobbled deepslate | 1 gravel | 2 | 6 Nm |
| Gravel | 1 sand, plus 10% chance of 1 flint | 2 | 6 Nm |
| Any wool (`#minecraft:wool`) | 4 string | 2 | 3 Nm |
| Bone | 4 bone meal, plus 50% chance of a 5th (avg 4.5) | 2 | 3 Nm |

All values *(tune)*. The design rationale:

- **1.5× is the grinder's theme.** Raw ore averages 1.5 crushed ore, which **stacks with Fortune** because
  Fortune acts on the raw ore you mine. Bone averages 4.5 bone meal, 1.5× the crafting grid's 3.
- **Wool → 4 string** gives back what crafting the wool took: lossless, with no gain from looping.
- **Raw ore blocks** are accepted so players needn't uncraft their storage first. Same yield as 9 raw ore.
  *(Provisional: revisit after playtesting.)*
- **Not accepted in 0.1:** ore blocks mined with Silk Touch (they could only match unenchanted mining, and
  copper's 2 to 5 drop makes a fair yield awkward), nether gold ore (it drops nuggets). The grinder's tooltip
  says "raw ore only: mine ore blocks first".
- Rust flakes → pigment arrives with rust in 0.3 (section 11).

### 2.1 Crushed ore

- Three items: **crushed iron, crushed copper, crushed gold**.
- Each smelts to **1 ingot** in a furnace or blast furnace, with the same XP and cook time as the matching raw
  ore. The smelting recipes are FRAS's own data.
- No storage blocks: crushed ore is meant to be smelted, not stockpiled.
- From 0.4 the washer turns crushed ore into clean dust plus byproducts; smelting crushed ore directly keeps
  working.
- Item tag `fras:crushed_ores` (all three), for the washer and for datapacks.

### 2.2 Recipe type `fras:grinding`

Recipes are data, in `data/fras/recipe/*.json`, so datapacks and other mods can add or override them without
code (Horizon supports custom recipe types, per the spike). Every file is written by FRAS; no vanilla recipe
data is copied (Horizon's registry forbids it).

```json
{
  "type": "fras:grinding",
  "ingredient": { "item": "minecraft:raw_iron" },
  "count": 1,
  "results": [
    { "item": "fras:crushed_iron", "count": 1 },
    { "item": "fras:crushed_iron", "count": 1, "chance": 0.5 }
  ],
  "revolutions": 4,
  "load": 6
}
```

- `ingredient`: an item or a tag (`{ "tag": "minecraft:wool" }`). `count` (default 1) is how many are consumed.
- `results`: 1 to 4 entries; `count` default 1; `chance` (0 to 1, default 1) is rolled **per item** of the
  count, so `{ "count": 9, "chance": 0.5 }` makes nine rolls.
- `revolutions`: work per recipe (section 3). Required.
- `load`: Nm drawn while working; default **6**.
- Several recipes matching one input: the first by recipe id wins, and a warning is logged at load.

---

## 3. Work and load

- **Time is measured in revolutions of the grinder**, not seconds. Each tick in range, progress grows by
  `|rpm| / 1200` revolutions (60 s × 20 ticks). Direction doesn't matter.
- **Below 12 RPM:** no progress, full load while it has work (rotational spec 9.2).
- **Above 96 RPM** (overspeeding, before it jams): progress counts as if at 96 RPM. Overspeed buys nothing but
  a jam.
- Reference speeds: a water wheel's ~19 RPM grinds raw ore in ~12.6 s, roughly the pace of one vanilla furnace
  (10 s); at 96 RPM it takes 2.5 s.
- **"Has work"** means: the input holds a matching recipe's ingredient and count, every possible output fits
  (section 4), and the grinder isn't jammed. Only then does it pull the recipe's `load`; otherwise it is idle
  (friction only).
- The working range is read from the grinder's state through one function, not a constant, so cooling (0.3)
  can raise the maximum without touching the network code.

---

## 4. Inventory and progress

- **Slots:** 1 input (a full stack) and 4 outputs (2×2). No fuel or upgrade slots.
- **Space check:** a recipe only starts, and only counts as work, if all its results fit in the outputs with
  every chance roll at its maximum. Nothing is ever voided or dropped. Outputs full = idle, load released, status
  "Output full".
- **Chance rolls** happen when the recipe completes, using the level's random source.
- **Progress** (revolutions done, a float) is kept when the grinder runs too slow, loses power, jams, fills its
  outputs, or its network pauses; it is saved with the block. It **resets** only when the input is removed or
  replaced by something with a different recipe.
- **Jammed:** the GUI, hopper input and extraction, and the downward push all keep working; only grinding stops.
- **Breaking** drops the grinder and its contents, like a chest; progress is lost. Explosions likewise.

---

## 5. Input and output

### 5.1 Faces

The grinder has a **hopper mouth on top**, a **chute underneath** and its **shaft socket on one of the four
sides**.

- **Socket placement:** facing an adjacent shaft-like part whose axis points at the grinder, if there is one;
  otherwise facing the player. The wrench cycles it through the four sides (rotational spec 11.2). A vertical
  shaft drives a grinder through a bevel gear.
- **Input:** hoppers, droppers (and from 0.4, belts) insert through the **top or any side except the socket**.
  Insertion only reaches the input slot, and only items with a recipe are accepted.
- **Output:** the grinder **pushes down** out of the chute into the inventory below (chest, hopper, any block
  with item capability; from 0.4, a belt) at **1 item every 8 ticks** *(tune)*, the hopper's rate. If nothing
  below can take the items, they wait in the output slots. Nothing ever drops into the world.
- **Extraction:** a hopper below, or anything else pulling items, takes from the **output slots only**.
- There is no configurable output face and no wrench mode for one.

### 5.2 By hand and in the world

- **Right-click with anything except the wrench opens the GUI.** No direct hand insertion, so the GUI opens even
  when holding ore.
- **The wrench:** right-click rotates the socket or repairs a jam; sneak-right-click inspects (rotational spec
  11.2, 11.3).
- **Items tossed onto the top** are pulled into the input like a hopper does: every 8 ticks *(tune)*, item
  entities in the block space above whose item has a recipe and fits are taken in. The rest stay lying there.

### 5.3 Comparator and redstone

- **Comparator:** the standard vanilla container signal across all 5 slots (as a chest or furnace computes it),
  so a backing-up or starving grinder can be detected.
- **No redstone control:** a grinder can't be switched off by a signal. Stop it with a clutch, as in a real mill.

---

## 6. GUI

Opened by right-click (5.2). Spike lesson: the block's part opens the menu with `player.open_menu`.

- **Layout:** input slot on the left, a progress arrow, the 2×2 output grid on the right, the player inventory
  below. FRAS's own background.
- **Progress arrow:** fills by revolutions done ÷ revolutions needed.
- **Speed bar** under the arrow: a small horizontal scale from 0 to 128 RPM with the working band (12 to 96)
  green, red above the max, a needle at the current speed and the RPM as a number. A mini tachometer that
  teaches the range.
- **Status line**, one of (first match wins): "Jammed: repair with the wrench", "Paused: outside the simulated
  area", "Overspeed!" (red, blinking), "No power" (0 RPM), "Too slow: needs 12 RPM", "Output full",
  "Grinding", "Idle".
- **Data slots** (16-bit): progress in thousandths, signed RPM × 10, status code, maximum RPM (so the bar
  follows a cooled grinder later).
- Torque and load stay on the wrench overlay; the GUI doesn't show Nm.
- Shift-click moves items between the player inventory and the input; outputs shift-click to the player.

---

## 7. Look and sound

- **Form:** a roll crusher. A full block: timber frame with iron bands, hopper mouth on top, chute below, the
  socket marked by a gear on its side.
- **Animation:** two toothed iron rollers, visible through the mouth, turning toward each other at the
  grinder's speed, drawn with `spin` + `keep` like the shafts (no per-frame cost).
- **Particles:** while grinding, break particles of the input item at the mouth and a small dust puff at the
  chute. Client-side, from the synced "grinding" flag and input item.
- **Sound:** a crunching loop while grinding whose pitch follows the RPM; silent when idle apart from the
  network's gear whine; the rotational spec's rattle (overspeed), clunk (jam) and clank (repair).
- **Art:** placeholder in 0.1, original or freely licensed (credits list from the first asset).
- **Names:** Grinder, crushed iron/copper/gold; Polish names from the terminology dictionary (suggested:
  Kruszarka; Kruszona ruda żelaza / miedzi / złota).

---

## 8. Crafting and discovery

### 8.1 Crafting recipe

Hand-crafted from vanilla materials (an entry block, design doc 6.4):

```
plank   flint        plank
iron    flint        iron
plank   cobblestone  plank
```

2 iron ingots, 2 flint (the roller teeth), 4 planks (any, `#minecraft:planks`), 1 cobblestone. It unlocks in
the recipe book on picking up flint or an iron ingot. The repair cost (2 nuggets, 1 flint) stays near the
rotational spec's "about an eighth".

### 8.2 Finding out what it grinds

There is no Recipes viewer yet (design doc 7.2), so in 0.1:

- **The grinder's Shift tooltip** lists its inputs: "Grinds: raw ores (×1.5), raw ore blocks, cobblestone,
  gravel, wool, bones. Raw ore only: mine ore blocks first."
- **Refusal is feedback:** the input slot and hoppers accept only items with a recipe.
- **The docs page** in the repository lists every recipe.
- **No tooltip lines are added to vanilla items.**
- **Recipe pages in the engine's item panel** (`panel_recipes`) only if they take about a day; otherwise 0.4.

---

## 9. On Horizon

- **Block entity:** its own type (spike lesson 1); **ticks** (it is a machine). Push reaction "block"; structure
  rotation rotates the socket.
- **Each tick:** read the network speed from the server state (rotational spec 10.3), decide "has work", report
  the load for the tick, add progress, complete a recipe if due. Every 8 ticks: push one item down, pull item
  entities from above. Return `Unchanged` when nothing changed.
- **Inventory:** item slots with the item capability; sided rules from 5.1 (insert: top and non-socket sides →
  input; extract: any side → outputs).
- **Saved:** inventory, progress, socket side, plus the network fields the rotational spec saves (speed, jam
  state and reason, warning timers). The save format leaves room for a fluid tank (cooling, 0.3).
- **Client sync** (`block_entity_update_tag`): the rotational spec's fields plus a "grinding" flag and the input
  item id (particles, sound). Sent on change only.
- **Menu:** `block_container` with FRAS's screen layout; `screen_draw` draws the arrow, speed bar and status.

---

## 10. Code layout and tests

- **`sim::grind`**, game-free: recipe model and matching (item, tag, count), the space check with maximum rolls,
  progress per tick from RPM (in range, below the minimum, capped above the maximum), completion and chance
  rolls (with an injectable random source), progress kept/reset rules, the push and pull cadence, the status
  code. No Horizon types.
- **Recipe loading** in the game layer, turning JSON into the `sim::grind` model, with validation (1 to 4
  results, chance in range, revolutions > 0) and a logged warning for duplicate matches.
- **Unit tests** at least for: each 0.1 recipe's yield distribution (many seeded runs average 1.5×, 13.5, 4.5);
  revolutions to time at 12, 19, 48, 96 and 150 RPM; no progress below 12; load present only while it has work;
  output-full stops before starting, never voids; chance rolls only at completion; progress kept across stop,
  jam and pause, reset on input change; raw block uses 36 revolutions; tags match every wool colour; the
  status line's order; comparator value matches the vanilla formula.

---

## 11. Hooks for later

| When | What | What 0.1 does for it |
| --- | --- | --- |
| 0.3 | **Water cooling:** a water port on the side opposite the socket. While cooled, max ~128 RPM *(tune)*; hot water out in proportion to work done, pipeable to a boiler as preheated feed water (design doc 4.5). | Working range from a function of state (3); save format room for a tank (9); speed bar scaled to 128 (6). |
| 0.3 | **Rust flakes → pigment.** | Just another `fras:grinding` recipe file. |
| 0.4 | **Belts:** a belt below receives the push; belts insert into the top and sides. | The push and insertion go through the item capability, so belts plug in. |
| 0.4 | **Washer:** crushed ore → clean dust + byproducts. | `fras:crushed_ores` tag. |
| 0.4 | **Item panel recipe pages**, if not done in 0.1. | Recipe model in `sim::grind` is the source. |
| Upstream | **Recipes viewer** page for `fras:grinding` once horizon-mods #1214 lands. | Same. |

---

## 12. Starting numbers *(tune)*

Recipes: section 2 table (revolutions 4 for raw ore, 36 for raw blocks, 2 otherwise; chances 50% extra
crushed ore, 10% flint, 50% fifth bone meal). Loads: 6 Nm (ore, stone, gravel), 3 Nm (wool, bone).
Push and pull every 8 ticks, 1 item per push. Speed bar scale 0 to 128 RPM. Cooled maximum ~128 RPM (0.3).
From the rotational spec: range 12 to 96 RPM, inertia 4, friction `c` 0.05, repair 2 iron nuggets + 1 flint.

---

## 13. Changes to the design doc and the rotational network spec

- Design doc 6.1: the grinder also takes raw ore blocks (provisional); not Silk Touch ore blocks or nether gold
  ore. Exact yields and the gravel → flint byproduct are in section 2.
- Design doc 6.2 / 7.1 / 6.3: the grinder takes items in the world by tossing them onto its top, not by hand
  (right-click opens the GUI); it pushes output only downwards; no redstone control.
- Design doc 7.2: no tooltip lines on vanilla items.
- Rotational network spec 9.1 and 14: the grinder's load is per recipe, 6 Nm or 3 Nm (wool, bone); the water
  wheel balance still holds, since lighter loads only make it easier.

---

## 14. 0.1 exit criteria (in game)

Driven through the Horizon launcher and its MCP server, as in the spike:

1. Every 0.1 recipe works from the GUI; crushed ore smelts to one ingot in a furnace and a blast furnace.
2. Over 64 raw iron, the yield is close to 96 crushed iron; Fortune-mined raw ore is accepted like any other.
3. A water wheel grinder takes ~12.6 s per raw ore; a crank line sags less grinding wool (3 Nm) than grinding
   cobblestone (6 Nm).
4. A hopper above feeds it, a hopper or chest below receives the output; with the chest below full, the
   grinder stops with "Output full", drops its load, and resumes when space appears.
5. Items tossed onto the top are taken in; items without a recipe stay lying there and are refused by the slot.
6. Overspeed: the status blinks, the speed bar's needle sits in the red, it jams; while jammed its inventory can
   still be emptied; the wrench repairs it and grinding continues from the saved progress.
7. The comparator reads the vanilla container signal; a clutch stops and restarts the grinder.
8. Save and reload keep inventory and progress; breaking drops the block and contents.
