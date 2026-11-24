#!/usr/bin/env python3
"""Writes FRAS's block and item declarations, models, blockstates, loot tables,
tags and recipes (everything under data/ and assets/ that is regular enough to
generate). Run from the repository root: python3 tools/gen_content.py

Hand-written files (lang, textures, grinding recipes) are left alone.
"""
import json
import os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
NS = "fras"


def write(path, value):
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w") as f:
        json.dump(value, f, indent=2)
        f.write("\n")


def px(v):
    return round(v / 16.0, 5)


def along(axis, a, b, w):
    """A box along `axis` from a to b (texels), w texels each side of the centre."""
    lo, hi = px(8 - w), px(8 + w)
    a, b = px(a), px(b)
    return {"x": [a, lo, lo, b, hi, hi], "y": [lo, a, lo, hi, b, hi], "z": [lo, lo, a, hi, hi, b]}[axis]


def by_axis(w, a=0, b=16):
    return {"properties": ["axis"], "values": {ax: [along(ax, a, b, w)] for ax in "xyz"}}


DIRS = ["down", "up", "north", "south", "west", "east"]
HORIZONTAL = ["north", "south", "west", "east"]
DIR_ENUM = {"type": "enum", "values": DIRS}

WOOD = "minecraft:oak_planks"
IRON = "minecraft:iron_block"
CHAIN = "minecraft:iron_chain"
ANIMATED = "ENTITYBLOCK_ANIMATED"


def tach_shapes():
    plate = {
        "up": [2, 0, 2, 14, 2, 14],
        "down": [2, 14, 2, 14, 16, 14],
        "north": [2, 2, 14, 14, 14, 16],
        "south": [2, 2, 0, 14, 14, 2],
        "west": [14, 2, 2, 16, 14, 14],
        "east": [0, 2, 2, 2, 14, 14],
    }
    return {"properties": ["facing"], "values": {d: [[px(v) for v in b]] for d, b in plate.items()}}


CASING = [[px(2), px(2), px(2), px(14), px(14), px(14)]]

# name: declaration, tool tag, particle texture
BLOCKS = {
    "wooden_shaft": (
        {"template": CHAIN, "properties": {"hardness": 1.5, "sounds": WOOD}, "shapes": by_axis(2)},
        "axe",
        "minecraft:block/stripped_oak_log",
    ),
    "iron_shaft": ({"template": CHAIN, "properties": {"hardness": 3.0}, "shapes": by_axis(1.5)}, "pickaxe", "minecraft:block/iron_block"),
    "bearing": (
        {
            "template": CHAIN,
            "properties": {"hardness": 3.0},
            "state_properties": {"mount": DIR_ENUM},
            "shapes": by_axis(5),
        },
        "pickaxe",
        "minecraft:block/polished_andesite",
    ),
    "clutch": (
        {
            "template": CHAIN,
            "properties": {"hardness": 3.0},
            "state_properties": {"powered": {"type": "boolean"}},
            "shapes": by_axis(4),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "pulley": (
        {"template": CHAIN, "properties": {"hardness": 1.5, "sounds": WOOD}, "shapes": by_axis(7, 5, 11)},
        "axe",
        "minecraft:block/spruce_planks",
    ),
    "large_pulley": (
        {"template": CHAIN, "properties": {"hardness": 1.5, "sounds": WOOD}, "shapes": by_axis(7, 5, 11)},
        "axe",
        "minecraft:block/spruce_planks",
    ),
    "wooden_bevel_gear": (
        {
            "template": WOOD,
            "properties": {"hardness": 2.0},
            "state_properties": {"input": DIR_ENUM, "output": DIR_ENUM, "tee": {"type": "boolean"}},
            "default_state": {"input": "west", "output": "south"},
            "shapes": {"default": CASING},
        },
        "axe",
        "minecraft:block/spruce_planks",
    ),
    "iron_bevel_gear": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0},
            "state_properties": {"input": DIR_ENUM, "output": DIR_ENUM, "tee": {"type": "boolean"}},
            "default_state": {"input": "west", "output": "south"},
            "shapes": {"default": CASING},
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "gearbox": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0},
            "state_properties": {"facing": DIR_ENUM, "powered": {"type": "boolean"}},
            "default_state": {"facing": "north"},
            "shapes": {"default": CASING},
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "crank": (
        {
            "template": WOOD,
            "properties": {"hardness": 1.0, "render_shape": ANIMATED, "occludes": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "north"},
            "shapes": {"default": [[px(3), px(3), px(3), px(13), px(13), px(13)]]},
        },
        "axe",
        "minecraft:block/oak_planks",
    ),
    "water_wheel": (
        {
            "template": WOOD,
            "properties": {"hardness": 2.0, "render_shape": ANIMATED, "occludes": False},
            "state_properties": {"axis": {"type": "enum", "values": ["x", "z"]}},
            "default_state": {"axis": "x"},
            "shapes": {"properties": ["axis"], "values": {ax: [along(ax, 0, 16, 4)] for ax in "xz"}},
        },
        "axe",
        "minecraft:block/oak_planks",
    ),
    "grinder": (
        {
            "template": WOOD,
            "properties": {"hardness": 2.5, "occludes": False},
            "state_properties": {"facing": {"type": "enum", "values": HORIZONTAL}},
            "default_state": {"facing": "north"},
        },
        "axe",
        "minecraft:block/stripped_oak_log",
    ),
    "tachometer": (
        {
            "template": "minecraft:stone",
            "properties": {"hardness": 1.0, "sounds": "minecraft:iron_block", "occludes": False, "requires_tool": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "up"},
            "shapes": tach_shapes(),
            "collision_shapes": tach_shapes(),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
}

# Blocks whose block entity draws them: the chain-shaped ones get the animated render shape too.
for name in ["wooden_shaft", "iron_shaft", "bearing", "clutch", "pulley", "large_pulley"]:
    BLOCKS[name][0]["properties"].update({"render_shape": ANIMATED, "occludes": False})

for name, (decl, _, _) in BLOCKS.items():
    decl["properties"]["push_reaction"] = "IMMOVEABLE"


def element(frm, to, textures):
    """A model element; `textures` maps face groups ('side', 'end' or face names) to texture refs."""
    faces = {}
    for f in ["north", "south", "east", "west", "up", "down"]:
        faces[f] = {"texture": textures.get(f, textures.get("side"))}
    return {"from": frm, "to": to, "faces": faces}


def bar(axis, a, b, w, side, end):
    lo, hi = 8 - w, 8 + w
    frm, to = {"x": ([a, lo, lo], [b, hi, hi]), "y": ([lo, a, lo], [hi, b, hi]), "z": ([lo, lo, a], [hi, hi, b])}[axis]
    ends = {"x": ["east", "west"], "y": ["up", "down"], "z": ["north", "south"]}[axis]
    tex = {"side": side}
    for e in ends:
        tex[e] = end
    return element(frm, to, tex)


def block_model(parent_textures, elements, particle):
    textures = dict(parent_textures)
    textures["particle"] = particle
    return {"parent": "minecraft:block/block", "textures": textures, "elements": elements}


def models():
    m = {}
    log, log_end = "minecraft:block/stripped_oak_log", "minecraft:block/stripped_oak_log_top"
    iron, spruce = "minecraft:block/iron_block", "minecraft:block/spruce_planks"
    andesite, oak = "minecraft:block/polished_andesite", "minecraft:block/oak_planks"
    # Particle-only block models for the parts the block entity draws.
    for name in ["wooden_shaft", "iron_shaft", "bearing", "clutch", "pulley", "large_pulley", "crank", "water_wheel"]:
        m[f"block/{name}"] = {"textures": {"particle": BLOCKS[name][2]}, "elements": []}
    # The gearbox: a large box on its slow face (north in the model) and a
    # small one on its fast face, so its ratio shows.
    m["block/gearbox"] = block_model(
        {"all": iron, "fast": andesite},
        [element([1, 1, 0], [15, 15, 9], {"side": "#all"}), element([4, 4, 9], [12, 12, 16], {"side": "#fast"})],
        iron,
    )
    for name, tex in [("wooden_bevel_gear", spruce), ("iron_bevel_gear", andesite)]:
        m[f"block/{name}"] = block_model({"all": tex}, [element([2, 2, 2], [14, 14, 14], {"side": "#all"})], tex)
    m["block/grinder"] = block_model(
        {"side": log, "top": "minecraft:block/smooth_stone", "post": "minecraft:block/dark_oak_log"},
        [element([0, 0, 0], [16, 10, 16], {"side": "#side", "up": "#top", "down": "#top"})]
        + [
            element([x, 10, z], [x + 2, 16, z + 2], {"side": "#post"})
            for x in (0, 14)
            for z in (0, 14)
        ],
        log,
    )
    m["block/tachometer"] = block_model(
        {"side": iron, "face": "minecraft:block/quartz_block_top"},
        [element([2, 0, 2], [14, 2, 14], {"side": "#side", "up": "#face"})],
        iron,
    )
    # Item models for the drawn parts.
    m["item/wooden_shaft"] = block_model({"side": log, "end": log_end}, [bar("y", 0, 16, 2, "#side", "#end")], log)
    m["item/iron_shaft"] = block_model({"side": iron}, [bar("y", 0, 16, 1.5, "#side", "#side")], iron)
    m["item/bearing"] = block_model(
        {"axle": iron, "housing": andesite},
        [bar("x", 0, 16, 1.5, "#axle", "#axle"), bar("x", 4, 12, 5, "#housing", "#housing"), element([5, 0, 6], [11, 3, 10], {"side": "#housing"})],
        andesite,
    )
    m["item/clutch"] = block_model(
        {"side": log, "end": log_end, "collar": "minecraft:block/redstone_block"},
        [bar("x", 0, 16, 2, "#side", "#end"), bar("x", 6, 10, 4, "#collar", "#collar")],
        iron,
    )
    m["item/pulley"] = block_model(
        {"side": log, "end": log_end, "disc": spruce},
        [bar("z", 0, 16, 2, "#side", "#end"), bar("z", 5, 11, 7, "#disc", "#disc")],
        spruce,
    )
    m["item/large_pulley"] = block_model(
        {"side": log, "end": log_end, "disc": "minecraft:block/dark_oak_planks"},
        [bar("z", 0, 16, 2, "#side", "#end"), bar("z", 5, 11, 8, "#disc", "#disc")],
        spruce,
    )
    m["item/crank"] = block_model(
        {"side": log, "end": log_end, "arm": oak},
        [bar("z", 6, 16, 1.5, "#side", "#end"), element([6.5, 6.5, 3], [15, 9.5, 6], {"side": "#arm"}), element([12, 6.5, 0], [15, 9.5, 3], {"side": "#side"})],
        oak,
    )
    m["item/water_wheel"] = block_model(
        {"side": log, "end": log_end, "paddle": oak},
        [
            bar("x", 0, 16, 2, "#side", "#end"),
            element([6, 0, 7], [10, 16, 9], {"side": "#side"}),
            element([6, 7, 0], [10, 9, 16], {"side": "#side"}),
            element([3, 0, 2], [13, 2, 14], {"side": "#paddle"}),
            element([3, 14, 2], [13, 16, 14], {"side": "#paddle"}),
            element([3, 2, 0], [13, 14, 2], {"side": "#paddle"}),
            element([3, 2, 14], [13, 14, 16], {"side": "#paddle"}),
        ],
        oak,
    )
    for name in ["wooden_bevel_gear", "iron_bevel_gear", "gearbox", "grinder", "tachometer"]:
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}"}
    for name in ["wrench", "leather_belt", "hand_hammer", "crushed_iron", "crushed_copper", "crushed_gold", "iron_plate", "copper_plate"]:
        m[f"item/{name}"] = {"parent": "minecraft:item/handheld" if name in ("wrench", "hand_hammer") else "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{name}"}}
    return m


def blockstate(name):
    if name == "gearbox":
        rot = {"north": {}, "south": {"y": 180}, "east": {"y": 90}, "west": {"y": 270}, "up": {"x": 270}, "down": {"x": 90}}
        return {"variants": {f"facing={d}": dict({"model": f"{NS}:block/gearbox"}, **r) for d, r in rot.items()}}
    if name == "tachometer":
        rot = {"up": {}, "down": {"x": 180}, "north": {"x": 90}, "south": {"x": 90, "y": 180}, "east": {"x": 90, "y": 90}, "west": {"x": 90, "y": 270}}
        return {"variants": {f"facing={d}": dict({"model": f"{NS}:block/tachometer"}, **r) for d, r in rot.items()}}
    return {"variants": {"": {"model": f"{NS}:block/{name}"}}}


ITEMS = {
    "wrench": {"template": "minecraft:stick", "max_stack_size": 1, "components": {
        "minecraft:tool": {"rules": [{"blocks": f"#{NS}:parts", "speed": 12.0, "correct_for_drops": True}], "default_mining_speed": 1.0, "damage_per_block": 0},
    }},
    "hand_hammer": {"template": "minecraft:stick", "max_stack_size": 1, "crafting_remainder": f"{NS}:hand_hammer"},
    "leather_belt": {"template": "minecraft:leather"},
    "crushed_iron": {"template": "minecraft:raw_iron"},
    "crushed_copper": {"template": "minecraft:raw_copper"},
    "crushed_gold": {"template": "minecraft:raw_gold"},
    "iron_plate": {"template": "minecraft:iron_ingot"},
    "copper_plate": {"template": "minecraft:copper_ingot"},
}


def shaped(pattern, key, result, count=1):
    return {"type": "minecraft:crafting_shaped", "category": "misc", "pattern": pattern, "key": key, "result": {"id": result, "count": count}}


def recipes():
    planks, logs = "#minecraft:planks", "#minecraft:logs"
    r = {
        "wooden_shaft": shaped(["L", "L"], {"L": logs}, f"{NS}:wooden_shaft", 8),
        "iron_shaft": shaped(["I", "I"], {"I": "minecraft:iron_ingot"}, f"{NS}:iron_shaft", 4),
        "bearing": shaped([" N ", "NPN", " N "], {"N": "minecraft:iron_nugget", "P": planks}, f"{NS}:bearing", 2),
        "clutch": shaped(["SRS"], {"S": f"{NS}:iron_shaft", "R": "minecraft:redstone"}, f"{NS}:clutch"),
        "pulley": shaped([" P ", "PSP", " P "], {"P": planks, "S": f"{NS}:wooden_shaft"}, f"{NS}:pulley"),
        "large_pulley": shaped(["PPP", "PSP", "PPP"], {"P": planks, "S": f"{NS}:wooden_shaft"}, f"{NS}:large_pulley"),
        "leather_belt": shaped(["LLL"], {"L": "minecraft:leather"}, f"{NS}:leather_belt", 2),
        "wooden_bevel_gear": shaped(["P P", " S ", "P P"], {"P": planks, "S": f"{NS}:wooden_shaft"}, f"{NS}:wooden_bevel_gear", 2),
        "iron_bevel_gear": shaped(["I I", " S ", "I I"], {"I": "minecraft:iron_ingot", "S": f"{NS}:iron_shaft"}, f"{NS}:iron_bevel_gear", 2),
        "gearbox": shaped(["III", "S S", "III"], {"I": "minecraft:iron_ingot", "S": f"{NS}:iron_shaft"}, f"{NS}:gearbox"),
        "crank": shaped(["SS", "PS"], {"S": "minecraft:stick", "P": planks}, f"{NS}:crank"),
        "water_wheel": shaped(["PSP", "SLS", "PSP"], {"P": planks, "S": "minecraft:stick", "L": logs}, f"{NS}:water_wheel"),
        "grinder": shaped(["PFP", "IFI", "PCP"], {"P": planks, "F": "minecraft:flint", "I": "minecraft:iron_ingot", "C": "minecraft:cobblestone"}, f"{NS}:grinder"),
        "tachometer": shaped(["NRN", "NGN"], {"N": "minecraft:iron_nugget", "R": "minecraft:redstone", "G": "minecraft:glass_pane"}, f"{NS}:tachometer"),
        "wrench": shaped(["I I", " I ", " S "], {"I": "minecraft:iron_ingot", "S": "minecraft:stick"}, f"{NS}:wrench"),
        "hand_hammer": shaped([" I ", " SI", "S  "], {"I": "minecraft:iron_ingot", "S": "minecraft:stick"}, f"{NS}:hand_hammer"),
    }
    for metal in ["iron", "copper"]:
        r[f"{metal}_plate"] = {
            "type": "minecraft:crafting_shapeless",
            "category": "misc",
            "ingredients": [f"{NS}:hand_hammer", f"minecraft:{metal}_ingot"],
            "result": {"id": f"{NS}:{metal}_plate", "count": 1},
        }
    for metal in ["iron", "copper", "gold"]:
        for kind, time in [("smelting", 200), ("blasting", 100)]:
            r[f"{metal}_ingot_from_crushed_{metal}_{kind}"] = {
                "type": f"minecraft:{kind}",
                "category": "misc",
                "ingredient": f"{NS}:crushed_{metal}",
                "result": {"id": f"minecraft:{metal}_ingot"},
                "experience": 0.7,
                "cookingtime": time,
            }
    return r


def grinding():
    def out(item, count=1, chance=1.0):
        o = {"id": item}
        if count != 1:
            o["count"] = count
        if chance != 1.0:
            o["chance"] = chance
        return o

    def recipe(ingredient, results, revolutions, load=6.0):
        return {"type": f"{NS}:grinding", "ingredient": ingredient, "results": results, "revolutions": revolutions, "load": load}

    g = {}
    for metal in ["iron", "copper", "gold"]:
        crushed = f"{NS}:crushed_{metal}"
        g[f"raw_{metal}"] = recipe(f"minecraft:raw_{metal}", [out(crushed), out(crushed, 1, 0.5)], 4.0)
        g[f"raw_{metal}_block"] = recipe(f"minecraft:raw_{metal}_block", [out(crushed, 9), out(crushed, 9, 0.5)], 36.0)
    g["cobblestone"] = recipe("minecraft:cobblestone", [out("minecraft:gravel")], 2.0)
    g["cobbled_deepslate"] = recipe("minecraft:cobbled_deepslate", [out("minecraft:gravel")], 2.0)
    g["gravel"] = recipe("minecraft:gravel", [out("minecraft:sand"), out("minecraft:flint", 1, 0.1)], 2.0)
    g["wool"] = recipe("#minecraft:wool", [out("minecraft:string", 4)], 2.0, 3.0)
    g["bone"] = recipe("minecraft:bone", [out("minecraft:bone_meal", 4), out("minecraft:bone_meal", 1, 0.5)], 2.0, 3.0)
    return g


def main():
    for name, recipe in grinding().items():
        write(f"data/{NS}/recipe/grinding/{name}.json", recipe)
    for name, (decl, tool, _) in BLOCKS.items():
        # Every part drops by hand (a bearing whose mount goes drops too);
        # the tool tags only make mining faster.
        decl["properties"]["requires_tool"] = False
        write(f"data/{NS}/horizon/block/{name}.json", decl)
        write(f"assets/{NS}/blockstates/{name}.json", blockstate(name))
        write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}})
        write(
            f"data/{NS}/loot_table/blocks/{name}.json",
            # The 26.3 loot format: one `condition` object with a `type`.
            {
                "type": "minecraft:block",
                "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": f"{NS}:{name}"}], "condition": {"type": "minecraft:survives_explosion"}}],
                "random_sequence": f"{NS}:blocks/{name}",
            },
        )
    for name, decl in ITEMS.items():
        write(f"data/{NS}/horizon/item/{name}.json", decl)
        write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}})
    for path, model in models().items():
        write(f"assets/{NS}/models/{path}.json", model)
    axe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "axe"]
    pickaxe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "pickaxe"]
    write("data/minecraft/tags/block/mineable/axe.json", {"replace": False, "values": axe})
    write("data/minecraft/tags/block/mineable/pickaxe.json", {"replace": False, "values": pickaxe})
    write(f"data/{NS}/tags/block/parts.json", {"replace": False, "values": [f"{NS}:{n}" for n in BLOCKS]})
    write(f"data/{NS}/tags/item/crushed_ores.json", {"replace": False, "values": [f"{NS}:crushed_{m}" for m in ["iron", "copper", "gold"]]})
    for name, recipe in recipes().items():
        write(f"data/{NS}/recipe/{name}.json", recipe)
    order = list(BLOCKS) + list(ITEMS)
    write(f"data/{NS}/horizon/creative_tab/main.json", {"title": "Fire, Rust and Steam", "icon": f"{NS}:gearbox", "items": [f"{NS}:{n}" for n in order]})


if __name__ == "__main__":
    main()
