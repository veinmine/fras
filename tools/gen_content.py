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


# ---- 0.2: the steam plant ------------------------------------------------------

BOOL = {"type": "boolean"}
AXIS_ENUM = {"type": "enum", "values": ["x", "y", "z"]}
HORIZ_ENUM = {"type": "enum", "values": HORIZONTAL}
COPPER = "minecraft:copper_block"
BRICKS = "minecraft:bricks"

# Shell materials: side, end, damaged texture.
SHELLS = {
    "copper_boiler": ("minecraft:block/copper_block", "minecraft:block/cut_copper", "minecraft:block/weathered_cut_copper"),
    "iron_boiler": ("minecraft:block/iron_block", "minecraft:block/polished_basalt_top", "minecraft:block/cracked_deepslate_tiles"),
    "storage_tank": ("minecraft:block/polished_andesite", "minecraft:block/iron_block", "minecraft:block/cracked_stone_bricks"),
}
PIPES = {
    "copper_pipe": ("minecraft:block/copper_block", "minecraft:block/cut_copper", "minecraft:block/weathered_cut_copper"),
    "iron_pipe": ("minecraft:block/iron_block", "minecraft:block/polished_andesite", "minecraft:block/cracked_deepslate_tiles"),
}
END_ROD = {"up": {}, "down": {"x": 180}, "north": {"x": 90}, "south": {"x": 90, "y": 180}, "east": {"x": 90, "y": 90}, "west": {"x": 90, "y": 270}}
NORTH_Y = {"north": {}, "east": {"y": 90}, "south": {"y": 180}, "west": {"y": 270}}
AXIS_ROT = {"y": {}, "z": {"x": 90}, "x": {"x": 90, "y": 90}}


def box(a, b):
    return [px(v) for v in a] + [px(v) for v in b]


def facing_boxes(boxes_up):
    """Shapes for a block drawn facing up, turned to each facing (as END_ROD turns the models)."""
    def turn(b, d):
        x0, y0, z0, x1, y1, z1 = b
        if d == "up":
            return [x0, y0, z0, x1, y1, z1]
        if d == "down":
            return [x0, 16 - y1, 16 - z1, x1, 16 - y0, 16 - z0]
        # Up turned to north: +y becomes -z.
        n = [x0, z0, 16 - y1, x1, z1, 16 - y0]
        if d == "north":
            return n
        if d == "south":
            return [16 - n[3], n[1], 16 - n[5], 16 - n[0], n[4], 16 - n[2]]
        if d == "east":
            return [16 - n[5], n[1], n[0], 16 - n[2], n[4], n[3]]
        return [n[2], n[1], 16 - n[3], n[5], n[4], 16 - n[0]]
    return {"properties": ["facing"], "values": {d: [[px(v) for v in turn(b, d)] for b in boxes_up] for d in DIRS}}


def pipe_shapes():
    arms = {
        "down": [5, 0, 5, 11, 5, 11], "up": [5, 11, 5, 11, 16, 11],
        "north": [5, 5, 0, 11, 11, 5], "south": [5, 5, 11, 11, 11, 16],
        "west": [0, 5, 5, 5, 11, 11], "east": [11, 5, 5, 16, 11, 11],
    }
    values = {}
    for m in range(64):
        key = ",".join("true" if m >> i & 1 else "false" for i in range(6))
        boxes = [[5, 5, 5, 11, 11, 11]] + [arms[d] for i, d in enumerate(DIRS) if m >> i & 1]
        values[key] = [[px(v) for v in b] for b in boxes]
    return {"properties": list(DIRS), "values": values}


SHELL_PROPS = dict({"formed": BOOL, "axis": AXIS_ENUM, "damaged": BOOL}, **{f"joined_{d}": BOOL for d in DIRS})
SHELL_DEFAULT = dict({"formed": "false", "axis": "y", "damaged": "false"}, **{f"joined_{d}": "false" for d in DIRS})
VENT_UP = [[5, 12, 5, 11, 16, 11], [4, 5, 4, 12, 12, 12], [5, 0, 5, 11, 5, 11]]
GAUGE_UP = [[2, 0, 2, 14, 2, 14]]
GLASS_UP = [[5, 0, 0, 11, 2.5, 16]]

STEAM_BLOCKS = {
    "copper_boiler": ({"template": COPPER, "properties": {"hardness": 3.0}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["copper_boiler"][0]),
    "iron_boiler": ({"template": IRON, "properties": {"hardness": 4.0}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["iron_boiler"][0]),
    "storage_tank": ({"template": IRON, "properties": {"hardness": 4.0}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["storage_tank"][0]),
    "steam_dome": (
        {"template": COPPER, "properties": {"hardness": 3.0, "occludes": False}, "shapes": {"default": [box([2, 0, 2], [14, 16, 14])]}},
        "pickaxe",
        "minecraft:block/copper_block",
    ),
    "firebox": (
        {
            "template": BRICKS,
            "properties": {"hardness": 3.5},
            "state_properties": {"facing": HORIZ_ENUM, "lit": BOOL},
            "default_state": {"facing": "north", "lit": "false"},
            "state_light": {"properties": ["lit"], "values": {"true": 13, "false": 0}},
        },
        "pickaxe",
        "minecraft:block/bricks",
    ),
    "manual_valve": (
        {"template": IRON, "properties": {"hardness": 3.0, "occludes": False}, "state_properties": {"axis": AXIS_ENUM, "damaged": BOOL}, "default_state": {"axis": "x", "damaged": "false"}, "shapes": by_axis(4)},
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "redstone_valve": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0, "occludes": False},
            "state_properties": {"axis": AXIS_ENUM, "damaged": BOOL, "powered": BOOL},
            "default_state": {"axis": "x", "damaged": "false", "powered": "false"},
            "shapes": by_axis(4),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "steam_release": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM, "open": BOOL, "damaged": BOOL},
            "default_state": {"facing": "down", "open": "false", "damaged": "false"},
            "shapes": facing_boxes(VENT_UP),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "emergency_valve": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM, "open": BOOL, "damaged": BOOL},
            "default_state": {"facing": "down", "open": "false", "damaged": "false"},
            "shapes": facing_boxes(VENT_UP),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "pump": (
        {"template": IRON, "properties": {"hardness": 3.0, "occludes": False}, "state_properties": {"facing": DIR_ENUM}, "default_state": {"facing": "north"}},
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "steam_cylinder": (
        {
            "template": IRON,
            "properties": {"hardness": 3.5, "occludes": False},
            "state_properties": {"facing": DIR_ENUM, "formed": BOOL, "damaged": BOOL},
            "default_state": {"facing": "north", "formed": "false", "damaged": "false"},
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "flywheel": (
        {
            "template": IRON,
            "properties": {"hardness": 3.5, "render_shape": ANIMATED, "occludes": False},
            "state_properties": {"axis": {"type": "enum", "values": ["x", "z"]}},
            "default_state": {"axis": "x"},
            "shapes": {"properties": ["axis"], "values": {ax: [along(ax, 0, 16, 4)] for ax in "xz"}},
        },
        "pickaxe",
        "minecraft:block/anvil",
    ),
    "pressure_gauge": (
        {
            "template": "minecraft:stone",
            "properties": {"hardness": 1.0, "sounds": "minecraft:iron_block", "occludes": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "up"},
            "shapes": facing_boxes(GAUGE_UP),
            "collision_shapes": facing_boxes(GAUGE_UP),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "sight_glass": (
        {
            "template": "minecraft:stone",
            "properties": {"hardness": 1.0, "sounds": "minecraft:glass", "occludes": False},
            "state_properties": {"facing": HORIZ_ENUM},
            "default_state": {"facing": "north"},
            "shapes": {"properties": ["facing"], "values": {
                "north": [box([5, 0, 13.5], [11, 16, 16])], "south": [box([5, 0, 0], [11, 16, 2.5])],
                "west": [box([13.5, 0, 5], [16, 16, 11])], "east": [box([0, 0, 5], [2.5, 16, 11])],
            }},
        },
        "pickaxe",
        "minecraft:block/copper_block",
    ),
}
for _name, (_mat, _end, _broken) in PIPES.items():
    STEAM_BLOCKS[_name] = (
        {
            "template": COPPER if _name == "copper_pipe" else IRON,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": dict({d: BOOL for d in DIRS}, damaged=BOOL),
            "default_state": dict({d: "false" for d in DIRS}, damaged="false"),
            "shapes": pipe_shapes(),
        },
        "pickaxe",
        _mat,
    )
# Keep the creative tab in a sensible order.
STEAM_ORDER = [
    "firebox", "copper_boiler", "iron_boiler", "steam_dome", "storage_tank", "copper_pipe", "iron_pipe",
    "manual_valve", "redstone_valve", "steam_release", "emergency_valve", "pump", "steam_cylinder", "flywheel",
    "pressure_gauge", "sight_glass",
]
for _name in STEAM_ORDER:
    _decl = STEAM_BLOCKS[_name]
    _decl[0]["properties"]["push_reaction"] = "IMMOVEABLE"
    BLOCKS[_name] = _decl


def el(frm, to, tex, skip=()):
    """An element whose faces all use `tex` (a dict per face or one ref), leaving out `skip`."""
    faces = {}
    for f in ["north", "south", "east", "west", "up", "down"]:
        if f in skip:
            continue
        t = tex.get(f, tex.get("side")) if isinstance(tex, dict) else tex
        faces[f] = {"texture": t}
    return {"from": frm, "to": to, "faces": faces}


def steam_models():
    m = {}
    cube = lambda tex: {"parent": "minecraft:block/cube_all", "textures": {"all": tex}}
    # Shells: plain riveted cubes unformed, the shell loader formed.
    for name, (side, end, broken) in SHELLS.items():
        m[f"block/{name}/unformed"] = cube(side)
        m[f"block/{name}/damaged"] = cube(broken)
        m[f"block/{name}/shell"] = {"textures": {"side": side, "end": end, "particle": side}}
        for axis in "xyz":
            for mask in range(64):
                m[f"block/{name}/f_{mask}_{axis}"] = {"parent": f"{NS}:block/{name}/shell", "loader": f"{NS}:shell", "mask": mask, "axis": axis}
    # Pipes: a core and a flanged arm per connected face (drawn as north,
    # turned by the multipart).
    for name, (mat, flange, broken) in PIPES.items():
        t = {"pipe": mat, "flange": flange, "particle": mat}
        m[f"block/{name}/core"] = {"textures": dict(t), "elements": [el([5, 5, 5], [11, 11, 11], "#pipe")]}
        m[f"block/{name}/core_damaged"] = {"textures": dict(t, broken=broken), "elements": [el([5, 5, 5], [11, 11, 11], "#broken")]}
        m[f"block/{name}/arm"] = {
            "textures": dict(t),
            "elements": [
                el([5, 5, 2], [11, 11, 5], "#pipe", skip=("north", "south")),
                el([4, 4, 0], [12, 12, 2], "#flange"),
            ],
        }
        m[f"item/{name}"] = {
            "parent": "minecraft:block/block",
            "textures": dict(t),
            "elements": [el([5, 5, 2], [11, 11, 14], "#pipe", skip=("north", "south")), el([4, 4, 0], [12, 12, 2], "#flange"), el([4, 4, 14], [12, 12, 16], "#flange")],
        }
    # Domes: a stepped copper cap with the outlet on top.
    copper, cut = "minecraft:block/copper_block", "minecraft:block/cut_copper"
    m["block/steam_dome"] = block_model(
        {"side": copper, "top": cut},
        [
            el([2, 0, 2], [14, 3, 14], {"side": "#side", "up": "#top", "down": "#top"}),
            el([3, 3, 3], [13, 10, 13], {"side": "#side", "up": "#top"}, skip=("down",)),
            el([4.5, 10, 4.5], [11.5, 12, 11.5], {"side": "#side", "up": "#top"}, skip=("down",)),
            el([6, 12, 6], [10, 16, 10], {"side": "#side", "up": "#top"}, skip=("down",)),
        ],
        copper,
    )
    # The firebox: brick body, an iron door that glows while lit.
    for lit, door in [("", "minecraft:block/blast_furnace_front"), ("_lit", "minecraft:block/blast_furnace_front_on")]:
        m[f"block/firebox{lit}"] = {
            "parent": "minecraft:block/orientable",
            "textures": {"front": door, "side": "minecraft:block/bricks", "top": "minecraft:block/bricks"},
        }
    # Valves: a body on a pipe along y (turned by axis).
    iron, andesite, dark = "minecraft:block/iron_block", "minecraft:block/polished_andesite", "minecraft:block/anvil"
    for name, top in [("manual_valve", andesite), ("redstone_valve", "minecraft:block/redstone_block")]:
        for broken in ["", "_damaged"]:
            body = "minecraft:block/cracked_deepslate_tiles" if broken else iron
            m[f"block/{name}{broken}"] = block_model(
                {"pipe": "minecraft:block/copper_block", "body": body, "top": top},
                [
                    el([5, 0, 5], [11, 4, 11], "#pipe", skip=("up",)),
                    el([5, 12, 5], [11, 16, 11], "#pipe", skip=("down",)),
                    el([4, 4, 4], [12, 12, 12], {"side": "#body", "east": "#top"}),
                ],
                iron,
            )
    # Vents, drawn facing up: the inlet on top, the outlet below.
    for name, cap in [("steam_release", dark), ("emergency_valve", "minecraft:block/gold_block")]:
        for broken in ["", "_damaged"]:
            body = "minecraft:block/cracked_deepslate_tiles" if broken else iron
            m[f"block/{name}{broken}"] = block_model(
                {"pipe": "minecraft:block/copper_block", "body": body, "cap": cap},
                [
                    el([5, 12, 5], [11, 16, 11], "#pipe", skip=("down",)),
                    el([4, 5, 4], [12, 12, 12], "#body"),
                    el([5, 0, 5], [11, 5, 11], "#cap", skip=("up",)),
                ],
                iron,
            )
    # The pump, facing up: the shaft socket on top, the copper outlet below.
    m["block/pump"] = block_model(
        {"body": iron, "band": dark, "socket": andesite, "outlet": "minecraft:block/copper_block"},
        [
            el([3, 2, 3], [13, 14, 13], {"side": "#body", "up": "#band", "down": "#band"}),
            el([2, 6, 2], [14, 9, 14], "#band"),
            el([5, 14, 5], [11, 16, 11], "#socket", skip=("down",)),
            el([5, 0, 5], [11, 2, 11], "#outlet", skip=("up",)),
        ],
        iron,
    )
    # The cylinder, facing up: the steam end below, the cover the piston rod
    # leaves through on top.
    for broken in ["", "_damaged"]:
        body = "minecraft:block/cracked_deepslate_tiles" if broken else iron
        m[f"block/steam_cylinder{broken}"] = block_model(
            {"body": body, "flange": dark},
            [
                el([1, 0, 1], [15, 2, 15], "#flange"),
                el([2, 2, 2], [14, 13, 14], "#body", skip=("up", "down")),
                el([1, 13, 1], [15, 16, 15], "#flange"),
            ],
            iron,
        )
    m["block/flywheel"] = {"textures": {"particle": dark}, "elements": []}
    m["item/flywheel"] = block_model(
        {"axle": iron, "rim": dark},
        [
            bar("x", 0, 16, 1.5, "#axle", "#axle"),
            el([6, 0, 2], [10, 2, 14], "#rim"),
            el([6, 14, 2], [10, 16, 14], "#rim"),
            el([6, 2, 0], [10, 14, 2], "#rim"),
            el([6, 2, 14], [10, 14, 16], "#rim"),
            el([7, 2, 7], [9, 14, 9], "#rim"),
            el([7, 7, 2], [9, 9, 7], "#rim"),
            el([7, 7, 9], [9, 9, 14], "#rim"),
        ],
        dark,
    )
    # Gauges, facing up: the dial a plate on the part below.
    m["block/pressure_gauge"] = block_model(
        {"side": "minecraft:block/copper_block", "face": "minecraft:block/quartz_block_top"},
        [el([2, 0, 2], [14, 2, 14], {"side": "#side", "up": "#face"})],
        "minecraft:block/copper_block",
    )
    # The sight glass on a north face of its block (the part is south): two
    # copper rods and caps around a white backing, the water column drawn
    # by its block entity in front.
    m["block/sight_glass"] = block_model(
        {"rod": "minecraft:block/copper_block", "back": "minecraft:block/white_concrete"},
        [
            el([5.5, 0, 13.5], [10.5, 1, 16], "#rod"),
            el([5.5, 15, 13.5], [10.5, 16, 16], "#rod"),
            el([5.5, 1, 14], [6.5, 15, 15], "#rod", skip=("up", "down")),
            el([9.5, 1, 14], [10.5, 15, 15], "#rod", skip=("up", "down")),
            el([6.5, 1, 15], [9.5, 15, 16], "#back", skip=("up", "down", "east", "west")),
        ],
        "minecraft:block/copper_block",
    )
    for name in ["copper_boiler", "iron_boiler", "storage_tank"]:
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}/unformed"}
    for name in ["steam_dome", "firebox", "manual_valve", "redstone_valve", "steam_release", "emergency_valve", "pump", "steam_cylinder", "pressure_gauge", "sight_glass"]:
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}"}
    return m


def steam_blockstate(name):
    if name in SHELLS:
        v = {
            "formed=false,damaged=false": {"model": f"{NS}:block/{name}/unformed"},
            "formed=false,damaged=true": {"model": f"{NS}:block/{name}/damaged"},
        }
        for axis in "xyz":
            for mask in range(64):
                joined = ",".join(f"joined_{d}={'true' if mask >> i & 1 else 'false'}" for i, d in enumerate(DIRS))
                v[f"formed=true,axis={axis},{joined}"] = {"model": f"{NS}:block/{name}/f_{mask}_{axis}"}
        return {"variants": v}
    if name in PIPES:
        # The arm model points north; turn it to each face.
        turn = {"north": {}, "south": {"y": 180}, "east": {"y": 90}, "west": {"y": 270}, "up": {"x": 270}, "down": {"x": 90}}
        parts = [
            {"when": {"damaged": "false"}, "apply": {"model": f"{NS}:block/{name}/core"}},
            {"when": {"damaged": "true"}, "apply": {"model": f"{NS}:block/{name}/core_damaged"}},
        ]
        for d in DIRS:
            parts.append({"when": {d: "true"}, "apply": dict({"model": f"{NS}:block/{name}/arm"}, **turn[d])})
        return {"multipart": parts}
    if name == "firebox":
        return {"variants": {f"facing={d},lit={l}": dict({"model": f"{NS}:block/firebox{'_lit' if l == 'true' else ''}"}, **r) for d, r in NORTH_Y.items() for l in ["false", "true"]}}
    if name in ("manual_valve", "redstone_valve"):
        return {"variants": {f"axis={a},damaged={b}": dict({"model": f"{NS}:block/{name}{'_damaged' if b == 'true' else ''}"}, **r) for a, r in AXIS_ROT.items() for b in ["false", "true"]}}
    if name in ("steam_release", "emergency_valve", "steam_cylinder"):
        return {"variants": {f"facing={d},damaged={b}": dict({"model": f"{NS}:block/{name}{'_damaged' if b == 'true' else ''}"}, **r) for d, r in END_ROD.items() for b in ["false", "true"]}}
    if name in ("pump", "pressure_gauge"):
        return {"variants": {f"facing={d}": dict({"model": f"{NS}:block/{name}"}, **r) for d, r in END_ROD.items()}}
    if name == "sight_glass":
        return {"variants": {f"facing={d}": dict({"model": f"{NS}:block/sight_glass"}, **r) for d, r in NORTH_Y.items()}}
    if name == "flywheel":
        return {"variants": {"": {"model": f"{NS}:block/flywheel"}}}
    return {"variants": {"": {"model": f"{NS}:block/{name}"}}}


def shapeless(ingredients, result, count=1):
    return {"type": "minecraft:crafting_shapeless", "category": "misc", "ingredients": ingredients, "result": {"id": result, "count": count}}


def steam_recipes():
    c, p, i = "minecraft:copper_ingot", f"{NS}:iron_plate", "minecraft:iron_ingot"
    return {
        # Multiblocks spec 8.
        "copper_boiler": shaped(["CCC", "C C", "CCC"], {"C": c}, f"{NS}:copper_boiler"),
        "iron_boiler": shaped(["PPP", "P P", "PPP"], {"P": p}, f"{NS}:iron_boiler"),
        "steam_dome": shaped(["CCC", "C C"], {"C": c}, f"{NS}:steam_dome"),
        "storage_tank": shaped(["PIP", "I I", "PIP"], {"P": p, "I": i}, f"{NS}:storage_tank", 2),
        "steam_cylinder": shaped(["PPP", "K S", "PPP"], {"P": p, "K": "minecraft:piston", "S": f"{NS}:iron_shaft"}, f"{NS}:steam_cylinder"),
        "flywheel": shaped(["III", "ISI", "III"], {"I": i, "S": f"{NS}:iron_shaft"}, f"{NS}:flywheel"),
        # Fluids spec 13.1.
        "copper_pipe": shaped(["CCC"], {"C": c}, f"{NS}:copper_pipe", 6),
        "iron_pipe": shaped(["PPP"], {"P": p}, f"{NS}:iron_pipe", 6),
        "manual_valve": shapeless([f"{NS}:copper_pipe", i], f"{NS}:manual_valve"),
        "redstone_valve": shapeless([f"{NS}:manual_valve", "minecraft:redstone"], f"{NS}:redstone_valve"),
        "steam_release": shapeless([f"{NS}:manual_valve", "minecraft:iron_bars"], f"{NS}:steam_release"),
        "emergency_valve": shapeless([f"{NS}:manual_valve", i, c], f"{NS}:emergency_valve"),
        "pump": shaped(["I I", "PSP", "III"], {"I": i, "P": f"{NS}:copper_pipe", "S": f"{NS}:iron_shaft"}, f"{NS}:pump"),
        # Steam spec 17.1.
        "firebox": shaped(["BBB", "BFB", "BBB"], {"B": "minecraft:brick", "F": "minecraft:furnace"}, f"{NS}:firebox"),
        "pressure_gauge": shaped(["NRN", "NGN"], {"N": "minecraft:copper_nugget", "R": "minecraft:redstone", "G": "minecraft:glass_pane"}, f"{NS}:pressure_gauge"),
        "sight_glass": shaped(["N", "G", "N"], {"N": "minecraft:copper_nugget", "G": "minecraft:glass_pane"}, f"{NS}:sight_glass"),
    }


def steam_data():
    # The firebox's menu: one fuel slot, data [burn, total, kW, boilers].
    write(f"data/{NS}/horizon/menu/firebox.json", {"slots": 1, "data_slots": 4})
    # Scalding (steam spec 8): heat, so fire resistance stops it; armour doesn't.
    write(f"data/{NS}/damage_type/scald.json", {"message_id": f"{NS}.scald", "exhaustion": 0.1, "scaling": "never", "effects": "burning"})
    write("data/minecraft/tags/damage_type/is_fire.json", {"replace": False, "values": [f"{NS}:scald"]})
    write("data/minecraft/tags/damage_type/bypasses_armor.json", {"replace": False, "values": [f"{NS}:scald"]})


def main():
    for name, recipe in grinding().items():
        write(f"data/{NS}/recipe/grinding/{name}.json", recipe)
    for name, (decl, tool, _) in BLOCKS.items():
        # Every part drops by hand (a bearing whose mount goes drops too);
        # the tool tags only make mining faster.
        decl["properties"]["requires_tool"] = False
        write(f"data/{NS}/horizon/block/{name}.json", decl)
        write(f"assets/{NS}/blockstates/{name}.json", steam_blockstate(name) if name in STEAM_BLOCKS else blockstate(name))
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
    # The grinder's menu: input slot 0, results 1-4 (players only take from
    # those), data slots [progress, rpm, status, max].
    write(f"data/{NS}/horizon/menu/grinder.json", {"slots": 5, "data_slots": 4, "results": [1, 2, 3, 4]})
    for name, decl in ITEMS.items():
        write(f"data/{NS}/horizon/item/{name}.json", decl)
        write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}})
    for path, model in list(models().items()) + list(steam_models().items()):
        write(f"assets/{NS}/models/{path}.json", model)
    axe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "axe"]
    pickaxe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "pickaxe"]
    write("data/minecraft/tags/block/mineable/axe.json", {"replace": False, "values": axe})
    write("data/minecraft/tags/block/mineable/pickaxe.json", {"replace": False, "values": pickaxe})
    write(f"data/{NS}/tags/block/parts.json", {"replace": False, "values": [f"{NS}:{n}" for n in BLOCKS]})
    write(f"data/{NS}/tags/item/crushed_ores.json", {"replace": False, "values": [f"{NS}:crushed_{m}" for m in ["iron", "copper", "gold"]]})
    steam_data()
    for name, recipe in list(recipes().items()) + list(steam_recipes().items()):
        write(f"data/{NS}/recipe/{name}.json", recipe)
    order = list(BLOCKS) + list(ITEMS)
    write(f"data/{NS}/horizon/creative_tab/main.json", {"title": "Fire, Rust and Steam", "icon": f"{NS}:gearbox", "items": [f"{NS}:{n}" for n in order]})


if __name__ == "__main__":
    main()
