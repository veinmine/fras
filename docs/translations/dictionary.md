# Translation dictionary: English → Polish

The terms FRAS uses, with the Polish word to use for each. Reuse these in every new or changed Polish
text, so the mod reads the same everywhere and matches the game.

Rule of thumb, as in Kid Mode: when vanilla Minecraft has an official Polish name for a thing, use exactly
that name (from the game's `assets/minecraft/lang/pl_pl.json`, game version 26.3; the key is given so it
can be checked after an update). Terms vanilla has no word for are the mod's own choices, marked **mod**;
for those, prefer the word a Polish mechanic would use.

## How the mod is translated

- The texts live in `assets/fras/lang/en_us.json` and `pl_pl.json`. `cargo test` fails when a language
  lacks a key English has, or when the code uses a `fras.` key no language has.
- Keys: `block.fras.<id>`, `item.fras.<id>`, `tooltip.fras.<id>` (and `.details`, shown with Shift),
  `fras.jam.*`, `fras.repair.*`, `fras.wrench.*`, `fras.inspect.*`, `fras.belt.*`, `fras.grinder.*`.
- A resource pack with `assets/fras/lang/<code>.json` can change any text or add a language.

## Style

- Sentence case for everything but proper names: "Drewniany wał", "Koło pasowe".
- Units: `Nm` as in English; RPM is **obr./min** (with a space before it: "32 obr./min").
- Decimal comma in prose ("×1,5"); numbers the code prints keep the dot.
- Adjectives agree with the noun: drewniany wał, drewniana przekładnia, żelazna blacha. A status line about
  the grinder is feminine (Kruszarka): "Bezczynna", "Zablokowana".
- Imperative, second person singular, as vanilla: "Kliknij", "Przytrzymaj", "Napraw kluczem".

## Blocks and items

| English | Polish | Source / note |
|---|---|---|
| Wooden / Iron Shaft | Drewniany / Żelazny wał | **mod** |
| Bearing | Łożysko | **mod** |
| Clutch | Sprzęgło | **mod** |
| Pulley | Koło pasowe | **mod**, the mechanical term |
| Large Pulley | Duże koło pasowe | **mod** |
| Bevel Gear | Przekładnia kątowa | **mod** (Drewniana / Żelazna przekładnia kątowa) |
| Gearbox | Przekładnia zębata | **mod** |
| Crank | Korba | **mod** |
| Water Wheel | Koło wodne | **mod** |
| Grinder | Kruszarka | **mod**. Not "Młynek" (a kitchen grinder) |
| Tachometer | Obrotomierz | **mod** |
| Wrench | Klucz | **mod** |
| Leather Belt | Skórzany pas | **mod** |
| Hand Hammer | Młotek | **mod** |
| Crushed Iron / Copper / Gold | Kruszona ruda żelaza / miedzi / złota | **mod** |
| Iron / Copper Plate | Żelazna / Miedziana blacha | **mod** |
| Conveyor Belt | Przenośnik taśmowy | **mod** |
| Bucket Elevator | Przenośnik kubełkowy | **mod** |
| Diverter | Zwrotnica | **mod** |
| Washer | Płuczka | **mod** |
| Trip Hammer | Młot ogoniasty | **mod**, the historical name |
| Lathe | Tokarka | **mod** |
| Sawmill | Trak | **mod**, the period word |
| Trommel | Przesiewacz bębnowy | **mod** |
| Iron Pulley | Żelazne koło pasowe | **mod** |
| Iron Rod / Iron Gear / Rivet | Pręt żelazny / Żelazne koło zębate / Nit | **mod** |
| Iron / Copper / Gold Dust | Pył żelaza / miedzi / złota | **mod** |

## Network terms

| English | Polish | Source / note |
|---|---|---|
| (shaft) line | linia wałów | **mod** |
| network | sieć | **mod** |
| part | część | **mod** |
| torque | moment (obrotowy) | **mod**; "Maks. moment" in tooltips |
| load | obciążenie | **mod** |
| support | podpora | **mod** |
| span | przęsło | **mod** |
| sagging | ugina się | **mod** |
| red line | czerwona linia | **mod** |
| jammed | zablokowane / zablokowana | **mod** |
| overspeed | nadmierna prędkość | **mod** ("Za szybko!" on the grinder screen) |
| overload / shears | przeciążenie / pęknie | **mod** |
| shock | wstrząs | **mod** |
| conflict | konflikt | **mod** |
| stalled | zatrzymane | **mod** |
| belt crossed / open | pas skrzyżowany / otwarty | **mod** |
| slips (a belt) | ślizga się | **mod** |
| T (second bevel output) | trójnik | **mod** |
| ratio | przełożenie | **mod** |
| paddles (water wheel) | łopatki | **mod** |
| paused (outside the simulated area) | wstrzymane | **mod** |
| Firebox | Palenisko | **mod** |
| Boiler (copper / iron) | Kocioł (miedziany / żelazny) | **mod** |
| Steam dome | Kołpak parowy | **mod** |
| Pipe (copper / iron) | Rura (miedziana / żelazna) | **mod** |
| Valve | Zawór | **mod** |
| Redstone valve | Zawór sterowany czerwonym kamieniem | **mod** |
| Steam release | Upust pary | **mod** |
| Emergency valve | Zawór bezpieczeństwa | **mod** |
| Pressure gauge | Manometr | **mod** |
| Sight glass | Wodowskaz | **mod** |
| Pump | Pompa | **mod** |
| Steam cylinder | Cylinder parowy | **mod** |
| Flywheel | Koło zamachowe | **mod** |
| Storage tank | Zbiornik | **mod** |
| steam | para | **mod** |
| pressure / rated pressure | ciśnienie / ciśnienie dopuszczalne | **mod** |
| rupture / burst | rozerwanie / pęknięcie | **mod** |
| scald, "was scalded" | poparzenie, "został poparzony parą" | **mod** |
| bar (unit) | bar | **mod** |
| simmering (a valve) | syczy | **mod** |
| starved (an engine) | niedobór pary | **mod** |
| Governor | Regulator odśrodkowy | **mod** |
| target speed | prędkość zadana | **mod** |
| sensitivity | czułość | **mod** |
| throttle | przepustnica | **mod** |
| hunting (a governor) | kołysanie (regulatora) | **mod** |
| Furnace damper | Szyber | **mod** |
| banked fire | przytłumiony ogień | **mod** |
| Bellows | Miech | **mod** |
| Heat exchanger | Wymiennik ciepła | **mod** |
| condensing | skraplanie | **mod** |
| Radiator (copper / iron) | Chłodnica (miedziana / żelazna) | **mod** |
| hot water | gorąca woda | **mod** |
| exhaust steam | para wylotowa | **mod** |
| Drain | Spust | **mod** |
| cooling | chłodzenie | **mod** |
| closed loop | obieg zamknięty | **mod** |
| Rust | Rdza | **mod** |
| clean / spotty / rusty / leaking (stages) | czysty / nalot rdzy / zardzewiały / cieknący | **mod** |
| rusted through | przerdzewiały | **mod** |
| Rust flakes | Płatki rdzy | **mod** |
| Red oxide pigment | Pigment żelazowy | **mod** |
| Red oxide paint | Minia żelazowa | **mod** |
| painted (Painted Iron Pipe) | pomalowany (Pomalowana żelazna rura) | **mod** |
| scrape | zeskrobać | **mod** |
| leak | wyciek | **mod** |
| belt line | ciąg przenośnika | **mod** |
| slope | pochylnia | **mod** |
| drive | napęd | **mod** |
| boot / head (elevator) | stopa / głowica (przenośnika) | **mod** |
| strike | uderzenie | **mod** |
| belt mode | praca na przenośniku | **mod** |
| fines / mediums / oversize | drobne / średnie / nadziarno | **mod** |

## Vanilla words FRAS uses

These follow the game; recheck the keys against `pl_pl.json` after an update.

| English | Polish | Key |
|---|---|---|
| Cobblestone | Bruk | `block.minecraft.cobblestone` |
| Gravel | Żwir | `block.minecraft.gravel` |
| Wool | Wełna | `block.minecraft.white_wool` (colour dropped) |
| Bone / Bone Meal | Kość / Mączka kostna | `item.minecraft.bone`, `item.minecraft.bone_meal` |
| Raw Iron (raw ores) | Surowe żelazo (surowe rudy) | `item.minecraft.raw_iron` |
| Iron Ingot | Sztabka żelaza | `item.minecraft.iron_ingot` |
| Iron Bars | Żelazne kraty | `block.minecraft.iron_bars` |
| Chain | Łańcuch | `block.minecraft.chain` |
| Fence / Wall | Płot / Murek | `block.minecraft.oak_fence`, `block.minecraft.cobblestone_wall` |
| Comparator | Komparator | `block.minecraft.comparator` |
| redstone (power) | czerwony kamień | as in `block.minecraft.redstone_torch` |
| crafting grid | siatka wytwarzania | after `container.crafting` "Wytwarzanie" |
| Sneak | skradaj się | `key.sneak` |
