# Fire, Rust and Steam

A native [Horizon](https://github.com/horizon-rust/horizon) technology mod for Minecraft 26.3: shaft power
first, steam later. Mod id `fras`. Licence: GPL-3.0-or-later.

**0.1 — Mechanical** (this version): turn things by hand or with water, carry the power along shafts,
gears, gearboxes, clutches and belts, and grind ore with it. Push too hard and parts overspeed, shear or
jam; the wrench repairs them.

## What's in 0.1

| Block / item | What it does |
| --- | --- |
| Crank | Hold right-click to turn it (sneak for the other way). 16 Nm, up to 32 RPM. |
| Water wheel | Turns in flowing water: 8 Nm per flowing block among its paddles, up to 32 Nm, 24 RPM. |
| Wooden / iron shaft | Carries power. Torque limit 32 / 128 Nm; needs a support every 8 / 16 blocks. |
| Bearing | A shaft segment that supports the line. Hangs on a sturdy face, fence, wall, bars or a chain above. |
| Wooden / iron bevel gear | Turns the line 90°; the wrench toggles a T. Max 56 / 192 RPM. |
| Gearbox | 2:1 (the wrench flips it). Max 192 RPM on the fast side. |
| Clutch | Disengages while powered by redstone. |
| Pulley, large pulley, leather belt | Link parallel shafts up to 16 blocks apart; large:small is 2:1. Belts slip above 24 Nm. |
| Grinder | Raw ores → 1.5 crushed ore, cobblestone → gravel, gravel → sand, wool → string, bone → bone meal. 12 to 96 RPM. |
| Tachometer | Shows a part's speed against the red line; comparator output 0–15. |
| Wrench | Right-click: configure or repair. Sneak-right-click: inspect. |
| Hand hammer | Ingots → plates in the crafting grid. |

How the power model works, what jams and why: [docs/rotational-network.md](docs/rotational-network.md).
Grinder recipes: [docs/grinder.md](docs/grinder.md). Building and testing: [docs/development.md](docs/development.md).

Translations: English and Polish ([dictionary](docs/translations/dictionary.md)).

The design document and the specs the code follows are in this repository: `Fire, rust and steam -
design.md` and `specs/`.
