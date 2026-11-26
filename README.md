# Fire, Rust and Steam

A native [Horizon](https://github.com/horizon-rust/horizon) technology mod for Minecraft 26.3: shaft power
first, steam later. Mod id `fras`. Licence: GPL-3.0-or-later.

**0.3 — Regulation and cooling** (this version): a flyball governor holds an engine's speed, a furnace
damper holds a boiler's pressure and bellows blow a firebox hotter; heat exchangers turn exhaust back into
hot feed water and cool the grinder; and iron rusts in water, rain and steam until it leaks, unless it is
scraped and painted. **0.2 — Steam**: fire a boiler, pipe its steam to an engine and drive the shaft network of
0.1 with it; pump water back in. Overfire it and it hisses, shakes and bursts unless an emergency valve
lets the steam go. **0.1 — Mechanical**: turn things by hand or with water, carry the power along shafts,
gears, gearboxes, clutches and belts, and grind ore with it. Push too hard and parts overspeed, shear or
jam; the wrench repairs them.

## What's in 0.3

| Block / item | What it does |
| --- | --- |
| Governor | A pipe part with a shaft socket: throttles the steam to every engine downstream to hold a target speed (8 to 192 RPM). Right-click for its screen: target and sensitivity (1 steady and droopy to 5 tight, which can hunt). |
| Furnace damper | On a boiler: turns its fireboxes down as pressure nears the set pressure (the wrench sets 1 to 8 bar). Shut, the fire banks; redstone shuts it. |
| Bellows | Against a firebox, turned at 16 to 128 RPM: up to 50% more heat from the same fuel. |
| Heat exchanger | Exhaust in, hot water out (condensing), or hot water in, water out: 250 kW, 500 kW waterlogged. Up to 4 radiators add to it. |
| Copper / iron radiator | On a heat exchanger: +125 / +75 kW. |
| Drain | Dumps water or hot water nothing else takes. |
| Rust flakes, red oxide pigment, red oxide paint | Scraped off rusted iron with the wrench, ground into pigment, mixed with a water bottle into paint. Paint a clean iron part, or 8 parts around 1 paint for 8 painted parts. |

Steam cylinders give exhaust from the top; piped to an exchanger it comes back as hot water a boiler takes
for less fuel. A grinder fed water (opposite its socket) works up to 128 RPM. Iron parts rust in stages
(spotty, rusty, leaking) while waterlogged, touching water, rained on or in steam; leaking parts lose
10 mB/s or jam. The client settings "Rust" and "Rust speed" turn it off or change its pace.

## What's in 0.2

| Block | What it does |
| --- | --- |
| Firebox | Burns any fuel (coal 1000 kW) and heats the boiler above or beside it. |
| Copper / iron boiler | Blocks that fill a cuboid form one boiler: up to 4 / 8 blocks, rated 4 / 6 bar. Fill by bucket or pump. |
| Steam dome | Goes on top of a boiler and lets the steam out (100 mB/s). A boiler without one is sealed and bursts when heated. |
| Storage tank | Up to 27 blocks; holds water or steam (an accumulator). |
| Copper / iron pipe | 80 / 200 mB/s, rated 4 / 8 bar. One content per network; the wrench connects or cuts a face. |
| Valve, redstone valve | Opens in 10% steps by hand / while powered. Closed, it splits the network. |
| Steam release, emergency valve | Vent steam by hand / at a set pressure (the wrench sets it in 0.5 bar steps). |
| Pump | On a shaft, 8 to 128 RPM: draws 25 mB/s per water source it touches. |
| Steam cylinder, flywheel | Two blocks apart they make an engine: 16 Nm per bar stall, 32 RPM per bar top. Two cylinders make a twin. |
| Pressure gauge, sight glass | Pressure against the red line, water level; both drive a comparator. |

Burst boilers explode; the client setting "Explosions" picks full, contained (only the boiler breaks) or
off (the boiler is only damaged). Escaping steam scalds; fire resistance protects.

## What's in 0.1

| Block / item | What it does |
| --- | --- |
| Crank | Hold right-click to turn it (sneak for the other way). 16 Nm, up to 32 RPM. |
| Water wheel | Turns in flowing water (only liquids go into its paddles): 8 Nm per flowing block among its paddles, up to 32 Nm, 24 RPM. |
| Wooden / iron shaft | Carries power. Torque limit 32 / 128 Nm; needs a support every 8 / 16 blocks. |
| Bearing | A shaft segment that supports the line. Hangs on a sturdy face, fence, wall, bars or a chain above. |
| Wooden / iron bevel gear | Turns the line 90°; the wrench toggles a T. Max 56 / 192 RPM. |
| Gearbox | 2:1 (the wrench or a change of redstone signal flips it). Max 192 RPM on the fast side. |
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
