#!/usr/bin/env python3
"""Writes FRAS's block and item declarations, models, blockstates, loot tables,
tags and recipes (everything under data/ and assets/ that is regular enough to
generate). Run from the repository root: python3 tools/gen_content.py

Hand-written files (lang, textures, grinding recipes) are left alone.
"""
import copy
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
    g["rust_flakes"] = recipe(f"{NS}:rust_flakes", [out(f"{NS}:red_oxide_pigment")], 2.0, 3.0)
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
    "copper_boiler": ({"template": COPPER, "properties": {"hardness": 3.0, "occludes": False}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["copper_boiler"][0]),
    "iron_boiler": ({"template": IRON, "properties": {"hardness": 4.0, "occludes": False}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["iron_boiler"][0]),
    "storage_tank": ({"template": IRON, "properties": {"hardness": 4.0, "occludes": False}, "state_properties": SHELL_PROPS, "default_state": SHELL_DEFAULT}, "pickaxe", SHELLS["storage_tank"][0]),
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
        m[f"block/{name}/shell_damaged"] = {"textures": {"side": broken, "end": broken, "particle": broken}}
        for axis in "xyz":
            for mask in range(64):
                for suffix in ["", "_damaged"]:
                    m[f"block/{name}/f_{mask}_{axis}{suffix}"] = {"parent": f"{NS}:block/{name}/shell{suffix}", "loader": f"{NS}:shell", "mask": mask, "axis": axis}
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
                for dmg, suffix in [("false", ""), ("true", "_damaged")]:
                    v[f"formed=true,damaged={dmg},axis={axis},{joined}"] = {"model": f"{NS}:block/{name}/f_{mask}_{axis}{suffix}"}
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
        # Banked keeps the lit door; the light level is what dims.
        return {"variants": {f"banked={b},facing={d},lit={l}": dict({"model": f"{NS}:block/firebox{'_lit' if l == 'true' else ''}"}, **r) for d, r in NORTH_Y.items() for l in ["false", "true"] for b in ["false", "true"]}}
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
        # The pipe keeps it off the copper helmet's grid.
        "steam_dome": shaped(["CCC", "CPC"], {"C": c, "P": f"{NS}:copper_pipe"}, f"{NS}:steam_dome"),
        "storage_tank": shaped(["PIP", "I I", "PIP"], {"P": p, "I": i}, f"{NS}:storage_tank", 2),
        "steam_cylinder": shaped(["PPP", "K S", "PPP"], {"P": p, "K": "minecraft:piston", "S": f"{NS}:iron_shaft"}, f"{NS}:steam_cylinder"),
        # The iron blocks keep it off the large iron pulley's grid.
        "flywheel": shaped(["IBI", "ISI", "IBI"], {"I": i, "B": "minecraft:iron_block", "S": f"{NS}:iron_shaft"}, f"{NS}:flywheel"),
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
    write(f"data/{NS}/horizon/menu/firebox.json", {"slots": 1, "data_slots": 6})
    # Scalding (steam spec 8): heat, so fire resistance stops it; armour doesn't.
    write(f"data/{NS}/damage_type/scald.json", {"message_id": f"{NS}.scald", "exhaustion": 0.1, "scaling": "never", "effects": "burning"})
    write("data/minecraft/tags/damage_type/is_fire.json", {"replace": False, "values": [f"{NS}:scald"]})
    write("data/minecraft/tags/damage_type/bypasses_armor.json", {"replace": False, "values": [f"{NS}:scald"]})



# ---- 0.3: regulation, cooling and rust ------------------------------------------------

FLAP_ENUM = {"type": "enum", "values": ["0", "1", "2", "3"]}
RUST_STAGES = ["clean", "spotty", "rusty", "leaking", "painted"]
RUST_ENUM = {"type": "enum", "values": RUST_STAGES}
# Rust spec 3: the iron parts, each with a pre-painted item.
RUSTING = [
    "iron_shaft", "bearing", "clutch", "iron_bevel_gear", "gearbox", "iron_pipe", "manual_valve", "redstone_valve",
    "steam_release", "emergency_valve", "governor", "iron_boiler", "storage_tank", "steam_cylinder", "flywheel", "pump",
    "heat_exchanger", "iron_radiator", "furnace_damper", "iron_pulley", "large_iron_pulley",
]
# Vanilla sprites standing for bare iron in the placeholder art: a rusted
# part shows the stage's sprite in their place.
IRONISH = {
    "minecraft:block/iron_block", "minecraft:block/polished_andesite", "minecraft:block/anvil",
    "minecraft:block/polished_basalt_top",
}

REGULATION_BLOCKS = {
    "governor": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0, "occludes": False},
            "state_properties": {"axis": AXIS_ENUM, "socket": DIR_ENUM, "damaged": BOOL},
            "default_state": {"axis": "x", "socket": "up", "damaged": "false"},
            "shapes": {"default": CASING},
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "furnace_damper": (
        {
            "template": IRON,
            "properties": {"hardness": 2.5, "occludes": False},
            "state_properties": {"facing": DIR_ENUM, "flap": FLAP_ENUM, "powered": BOOL},
            "default_state": {"facing": "north", "flap": "3", "powered": "false"},
            "shapes": facing_boxes([[2, 0, 2, 14, 6, 14]]),
            "collision_shapes": facing_boxes([[2, 0, 2, 14, 6, 14]]),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "bellows": (
        {
            "template": WOOD,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "north"},
            "shapes": facing_boxes([[2, 0, 2, 14, 13, 14], [6, 13, 6, 10, 16, 10]]),
        },
        "axe",
        "minecraft:block/oak_planks",
    ),
    "heat_exchanger": (
        {
            # The chain is waterloggable: an exchanger in water cools better.
            "template": CHAIN,
            "properties": {"hardness": 3.0, "occludes": False, "sounds": IRON},
            "state_properties": {"facing": DIR_ENUM, "damaged": BOOL},
            "default_state": {"facing": "up", "damaged": "false"},
            "shapes": {"default": [box([1, 0, 1], [15, 16, 15])]},
            "collision_shapes": {"default": [box([1, 0, 1], [15, 16, 15])]},
        },
        "pickaxe",
        "minecraft:block/copper_block",
    ),
    "copper_radiator": (
        {
            "template": COPPER,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "north"},
            "shapes": facing_boxes([[1, 0, 1, 15, 10, 15]]),
        },
        "pickaxe",
        "minecraft:block/copper_block",
    ),
    "iron_radiator": (
        {
            "template": IRON,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM},
            "default_state": {"facing": "north"},
            "shapes": facing_boxes([[1, 0, 1, 15, 10, 15]]),
        },
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "drain": (
        {
            "template": COPPER,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": {"facing": DIR_ENUM, "damaged": BOOL},
            "default_state": {"facing": "north", "damaged": "false"},
            "shapes": facing_boxes([[3, 0, 3, 13, 6, 13], [5, 6, 5, 11, 16, 11]]),
        },
        "pickaxe",
        "minecraft:block/copper_block",
    ),
}
for _name, _decl in REGULATION_BLOCKS.items():
    _decl[0]["properties"]["push_reaction"] = "IMMOVEABLE"
    BLOCKS[_name] = _decl
    STEAM_BLOCKS[_name] = _decl
# The firebox banks: a dim glow while its dampers are shut.
BLOCKS["firebox"][0]["state_properties"]["banked"] = BOOL
BLOCKS["firebox"][0]["default_state"]["banked"] = "false"
BLOCKS["firebox"][0]["state_light"] = {"properties": ["lit", "banked"], "values": {
    "true,false": 13, "true,true": 7, "false,false": 0, "false,true": 0}}

# ---- 0.4: the factory (logistics and machines specs) ------------------------------------

SLOPE_ENUM = {"type": "enum", "values": ["flat", "rising", "falling"]}
DRIVE_ENUM = {"type": "enum", "values": ["none"] + HORIZONTAL}
STEP = {"north": (0, -1), "south": (0, 1), "west": (-1, 0), "east": (1, 0)}


def belt_shapes():
    """Collision and outline per facing and slope: a flat belt is 6 px high; a
    slope is a half-height block with a step up toward its high end."""
    values = {}
    for f, (dx, dz) in STEP.items():
        for sl in ["flat", "rising", "falling"]:
            if sl == "flat":
                values[f"{f},{sl}"] = [[0, 0, 0, 1, px(6), 1]]
                continue
            # The high end: ahead for rising, behind for falling.
            up = (dx, dz) if sl == "rising" else (-dx, -dz)
            x0, x1 = (0.5, 1) if up[0] > 0 else ((0, 0.5) if up[0] < 0 else (0, 1))
            z0, z1 = (0.5, 1) if up[1] > 0 else ((0, 0.5) if up[1] < 0 else (0, 1))
            values[f"{f},{sl}"] = [[0, 0, 0, 1, 0.5, 1], [x0, 0.5, z0, x1, 1, z1]]
    return {"properties": ["facing", "slope"], "values": values}


LINE_MACHINES = ["washer", "trip_hammer", "lathe", "sawmill", "inline_grinder"]
SOCKET_FACES = HORIZONTAL + ["down"]


def line_machine(template, hardness, tool, particle):
    return (
        {
            "template": template,
            "properties": {"hardness": hardness, "occludes": False},
            "state_properties": {
                "facing": HORIZ_ENUM,
                "half": {"type": "enum", "values": ["lower", "upper"]},
                "socket": {"type": "enum", "values": SOCKET_FACES},
            },
            "default_state": {"facing": "north", "half": "lower", "socket": "down"},
        },
        tool,
        particle,
    )


FACTORY_BLOCKS = {
    "conveyor_belt": (
        {
            "template": WOOD,
            "properties": {"hardness": 1.0, "occludes": False},
            "state_properties": {"facing": HORIZ_ENUM, "slope": SLOPE_ENUM, "drive": DRIVE_ENUM},
            "default_state": {"facing": "north", "slope": "flat", "drive": "none"},
            "shapes": belt_shapes(),
            "collision_shapes": belt_shapes(),
        },
        "axe",
        "minecraft:block/brown_wool",
    ),
    "diverter": (
        {
            "template": WOOD,
            "properties": {"hardness": 1.5, "occludes": False},
            "state_properties": {
                "facing": HORIZ_ENUM, "flap": {"type": "enum", "values": ["left", "right"]}, "powered": BOOL,
                "slope": SLOPE_ENUM, "drive": DRIVE_ENUM,
            },
            "default_state": {"facing": "north", "flap": "left", "powered": "false", "slope": "flat", "drive": "none"},
            "shapes": belt_shapes(),
            "collision_shapes": belt_shapes(),
        },
        "axe",
        "minecraft:block/brown_wool",
    ),
    "bucket_elevator": (
        {
            "template": WOOD,
            "properties": {"hardness": 2.0, "occludes": False},
            "state_properties": {
                "facing": HORIZ_ENUM, "part": {"type": "enum", "values": ["single", "boot", "casing", "head"]},
                "drive": DRIVE_ENUM,
            },
            "default_state": {"facing": "north", "part": "single", "drive": "none"},
        },
        "axe",
        "minecraft:block/spruce_planks",
    ),
    # Line machines (machines spec 2.3): two blocks, placed like a door; the
    # top is the machine with its built-in belt, the bottom the drive.
    "washer": line_machine(WOOD, 2.5, "axe", "minecraft:block/dark_oak_planks"),
    "trip_hammer": line_machine(WOOD, 2.5, "axe", "minecraft:block/stripped_spruce_log"),
    "lathe": line_machine(IRON, 3.0, "pickaxe", "minecraft:block/iron_block"),
    "sawmill": line_machine(WOOD, 2.5, "axe", "minecraft:block/spruce_planks"),
    "inline_grinder": line_machine(WOOD, 2.5, "axe", "minecraft:block/stripped_oak_log"),
    # Three in a row form a trommel (machines spec 8.2).
    "trommel": (
        {
            "template": IRON,
            "properties": {"hardness": 3.0, "occludes": False},
            "state_properties": {
                "part": {"type": "enum", "values": ["none", "end", "middle"]},
                "axis": {"type": "enum", "values": ["x", "z"]},
                "socket": {"type": "enum", "values": ["none"] + HORIZONTAL},
            },
            "default_state": {"part": "none", "axis": "x", "socket": "none"},
        },
        "pickaxe",
        "minecraft:block/iron_bars",
    ),
    "iron_pulley": (
        {"template": CHAIN, "properties": {"hardness": 3.0, "render_shape": ANIMATED, "occludes": False}, "shapes": by_axis(7, 5, 11)},
        "pickaxe",
        "minecraft:block/iron_block",
    ),
    "large_iron_pulley": (
        {"template": CHAIN, "properties": {"hardness": 3.0, "render_shape": ANIMATED, "occludes": False}, "shapes": by_axis(7, 5, 11)},
        "pickaxe",
        "minecraft:block/iron_block",
    ),
}
for _name, _decl in FACTORY_BLOCKS.items():
    _decl[0]["properties"]["push_reaction"] = "IMMOVEABLE"
    BLOCKS[_name] = _decl

for _name in RUSTING:
    _d = BLOCKS[_name][0]
    _d.setdefault("state_properties", {})["rust"] = RUST_ENUM
    _d.setdefault("default_state", {})["rust"] = "clean"
    _d["properties"]["random_ticks"] = {"property": "rust", "values": ["clean", "spotty", "rusty"]}


# ---- Creative (debugging) blocks: no recipes, a tab of their own, and a
# magenta and black hazard border so they never pass for real machines.
CREATIVE_MARK = {
    "creative_engine": "engine",
    "load_bank": "load",
    "creative_steam_source": "steam",
    "creative_water_source": "water",
    "creative_drain": "drain",
    "creative_firebox": "fire",
}
CREATIVE_BLOCKS = {}
for _name, _mark in CREATIVE_MARK.items():
    _decl = {"template": IRON, "properties": {"hardness": 1.0, "push_reaction": "IMMOVEABLE"}}
    if _name in ("creative_engine", "load_bank"):
        _decl["properties"]["occludes"] = False
        _decl["state_properties"] = {"facing": DIR_ENUM}
        _decl["default_state"] = {"facing": "north"}
        _decl["shapes"] = {"default": CASING}
    CREATIVE_BLOCKS[_name] = (_decl, "pickaxe", f"{NS}:block/creative_{_mark}")
    BLOCKS[_name] = CREATIVE_BLOCKS[_name]


def creative_models():
    m = {}
    for name, mark in CREATIVE_MARK.items():
        tex = f"{NS}:block/creative_{mark}"
        if name in ("creative_engine", "load_bank"):
            # A casing; its block entity draws the turning stub on the socket.
            m[f"block/{name}"] = block_model({"all": tex}, [element([2, 2, 2], [14, 14, 14], {"side": "#all"})], tex)
        else:
            m[f"block/{name}"] = {"parent": "minecraft:block/cube_all", "textures": {"all": tex}}
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}"}
    return m


def creative_blockstate(name):
    if name in ("creative_engine", "load_bank"):
        return {"variants": {f"facing={d}": {"model": f"{NS}:block/{name}"} for d in DIRS}}
    return {"variants": {"": {"model": f"{NS}:block/{name}"}}}


def regulation_models():
    m = {}
    iron, andesite, dark = "minecraft:block/iron_block", "minecraft:block/polished_andesite", "minecraft:block/anvil"
    copper, cut = "minecraft:block/copper_block", "minecraft:block/cut_copper"
    broken = "minecraft:block/cracked_deepslate_tiles"
    # The governor: a valve body on a pipe along y with a brass cap (the
    # spindle and balls are drawn by its block entity).
    for b in ["", "_damaged"]:
        m[f"block/governor{b}"] = block_model(
            {"pipe": copper, "body": broken if b else iron, "top": "minecraft:block/gold_block"},
            [
                el([5, 0, 5], [11, 4, 11], "#pipe", skip=("up",)),
                el([5, 12, 5], [11, 16, 11], "#pipe", skip=("down",)),
                el([4, 4, 4], [12, 12, 12], {"side": "#body", "east": "#top", "west": "#top"}),
            ],
            iron,
        )
    # The damper, facing up: a riveted box on the boiler and a flap whose
    # angle shows the opening (0 shut, 3 open).
    for f in range(4):
        flap = {"from": [3, 6, 7.5], "to": [13, 15, 8.5], "faces": {d: {"texture": "#flap"} for d in ["north", "south", "east", "west", "up", "down"]}}
        if f < 3:
            angle = [-45.0, -22.5, 0.0][f] if f > 0 else -45.0
            flap = {"from": [3, 6, 7.5], "to": [13, 15, 8.5], "rotation": {"origin": [8, 6, 8], "axis": "x", "angle": angle},
                    "faces": {d: {"texture": "#flap"} for d in ["north", "south", "east", "west", "up", "down"]}}
        if f == 0:
            flap = el([3, 6, 3], [13, 7, 13], "#flap")
        m[f"block/furnace_damper_{f}"] = block_model(
            {"box": iron, "flap": dark},
            [el([2, 0, 2], [14, 6, 14], "#box"), flap],
            iron,
        )
    # Bellows, nozzle up: a wooden frame, leather folds, an iron nozzle.
    m["block/bellows"] = block_model(
        {"wood": "minecraft:block/oak_planks", "leather": "minecraft:block/brown_terracotta", "nozzle": iron},
        [
            el([2, 0, 2], [14, 2, 14], "#wood"),
            el([3, 2, 3], [13, 10, 13], "#leather"),
            el([2, 10, 2], [14, 12, 14], "#wood"),
            el([5, 12, 5], [11, 13, 11], "#wood"),
            el([6, 13, 6], [10, 16, 10], "#nozzle", skip=("down",)),
        ],
        "minecraft:block/oak_planks",
    )
    # The heat exchanger, outlet up: a copper shell with iron bands.
    for b in ["", "_damaged"]:
        m[f"block/heat_exchanger{b}"] = block_model(
            {"shell": broken if b else copper, "band": iron, "end": cut},
            [
                el([2, 2, 2], [14, 14, 14], {"side": "#shell", "up": "#end", "down": "#end"}),
                el([1, 4, 1], [15, 6, 15], "#band"),
                el([1, 10, 1], [15, 12, 15], "#band"),
                el([5, 14, 5], [11, 16, 11], "#end", skip=("down",)),
                el([5, 0, 5], [11, 2, 11], "#end", skip=("up",)),
            ],
            copper,
        )
    # Radiators, facing up (attached below): a plate and four fins.
    for name, mat in [("copper_radiator", copper), ("iron_radiator", iron)]:
        m[f"block/{name}"] = block_model(
            {"fin": mat},
            [el([1, 0, 1], [15, 2, 15], "#fin")] + [el([1 + 4 * i, 2, 1], [3 + 4 * i, 10, 15], "#fin") for i in range(4)],
            mat,
        )
    # The drain, pipe face up: a stub and a grate below.
    for b in ["", "_damaged"]:
        m[f"block/drain{b}"] = block_model(
            {"pipe": broken if b else copper, "grate": "minecraft:block/iron_trapdoor"},
            [el([5, 6, 5], [11, 16, 11], "#pipe", skip=("down",)), el([3, 0, 3], [13, 6, 13], {"side": "#pipe", "down": "#grate", "up": "#pipe"})],
            copper,
        )
    for name in ["governor", "bellows", "heat_exchanger", "copper_radiator", "iron_radiator", "drain"]:
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}"}
    m["item/furnace_damper"] = {"parent": f"{NS}:block/furnace_damper_3"}
    for name in ["rust_flakes", "red_oxide_pigment", "red_oxide_paint"]:
        m[f"item/{name}"] = {"parent": "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{name}"}}
    return m


def regulation_blockstate(name):
    dmg = lambda b: "_damaged" if b == "true" else ""
    if name == "governor":
        return {"variants": {f"axis={a},damaged={b}": dict({"model": f"{NS}:block/governor{dmg(b)}"}, **r) for a, r in AXIS_ROT.items() for b in ["false", "true"]}}
    if name == "furnace_damper":
        return {"variants": {f"facing={d},flap={f}": dict({"model": f"{NS}:block/furnace_damper_{f}"}, **r) for d, r in END_ROD.items() for f in "0123"}}
    if name in ("heat_exchanger", "drain"):
        return {"variants": {f"facing={d},damaged={b}": dict({"model": f"{NS}:block/{name}{dmg(b)}"}, **r) for d, r in END_ROD.items() for b in ["false", "true"]}}
    return {"variants": {f"facing={d}": dict({"model": f"{NS}:block/{name}"}, **r) for d, r in END_ROD.items()}}


def rusted_model(model, stage, models):
    """A copy of `model` with bare iron shown at `stage` (textures swapped,
    parents of this mod followed)."""
    out = json.loads(json.dumps(model))
    sprite = f"{NS}:block/rust_{stage}"
    for k, v in list(out.get("textures", {}).items()):
        if v in IRONISH:
            out["textures"][k] = sprite
    parent = out.get("parent", "")
    if parent.startswith(f"{NS}:"):
        path = parent.split(":", 1)[1]
        if path in models:
            out["parent"] = f"{parent}_rust_{stage}"
    elif parent == "minecraft:block/cube_all" and out.get("textures", {}).get("all") in IRONISH:
        out["textures"]["all"] = sprite
    return out


def rust_everything(models, states):
    """Rust variants of every rusting block's models and blockstates, and the
    painted items' models."""
    extra = {}

    def need(path):
        # A model and the mod parents it inherits from, for each stage.
        todo = [path]
        while todo:
            p = todo.pop()
            if p not in models:
                continue
            for st in RUST_STAGES[1:]:
                key = f"{p}_rust_{st}"
                if key not in extra:
                    extra[key] = rusted_model(models[p], st, models)
            par = models[p].get("parent", "")
            if par.startswith(f"{NS}:"):
                todo.append(par.split(":", 1)[1])

    for name in RUSTING:
        bs = states[name]
        if "variants" in bs:
            v2 = {}
            for key, var in bs["variants"].items():
                path = var["model"].split(":", 1)[1]
                need(path)
                for st in RUST_STAGES:
                    k = f"{key},rust={st}" if key else f"rust={st}"
                    mv = dict(var)
                    if st != "clean":
                        mv["model"] = f"{var['model']}_rust_{st}"
                    v2[k] = mv
            states[name] = {"variants": v2}
        else:
            parts = []
            for part in bs["multipart"]:
                path = part["apply"]["model"].split(":", 1)[1]
                need(path)
                for st in RUST_STAGES:
                    pp = json.loads(json.dumps(part))
                    pp.setdefault("when", {})["rust"] = st
                    if st != "clean":
                        pp["apply"]["model"] = f"{part['apply']['model']}_rust_{st}"
                    parts.append(pp)
            states[name] = {"multipart": parts}
        # The painted item's model.
        item = models.get(f"item/{name}")
        if item is None:
            continue
        if item.get("parent", "").startswith(f"{NS}:block/"):
            need(item["parent"].split(":", 1)[1])
            extra[f"item/painted_{name}"] = {"parent": f"{item['parent']}_rust_painted"}
        else:
            extra[f"item/painted_{name}"] = rusted_model(item, "painted", models)
    models.update(extra)


def regulation_recipes():
    p, i, c = f"{NS}:iron_plate", "minecraft:iron_ingot", f"{NS}:copper_pipe"
    r = {
        "governor": shaped(["I I", " S ", "PVP"], {"I": i, "S": f"{NS}:iron_shaft", "P": p, "V": f"{NS}:manual_valve"}, f"{NS}:governor"),
        "furnace_damper": shaped(["PPP", "PRP"], {"P": p, "R": "minecraft:redstone"}, f"{NS}:furnace_damper"),
        "bellows": shaped(["WWW", "LLL", "WSW"], {"W": "#minecraft:planks", "L": "minecraft:leather", "S": f"{NS}:wooden_shaft"}, f"{NS}:bellows"),
        "heat_exchanger": shaped(["PCP", "C C", "PCP"], {"P": p, "C": c}, f"{NS}:heat_exchanger"),
        "copper_radiator": shaped(["BBB", "BBB"], {"B": "minecraft:copper_bars"}, f"{NS}:copper_radiator", 2),
        "iron_radiator": shaped(["BBB", "BBB"], {"B": "minecraft:iron_bars"}, f"{NS}:iron_radiator", 2),
        "drain": shapeless([c, "minecraft:iron_trapdoor"], f"{NS}:drain"),
        # Rust spec 10 and 12.1.
        "red_oxide_paint": shapeless([f"{NS}:red_oxide_pigment", f"{NS}:red_oxide_pigment", "minecraft:potion"], f"{NS}:red_oxide_paint", 2),
    }
    for name in RUSTING:
        r[f"painted_{name}"] = shaped(["XXX", "XPX", "XXX"], {"X": f"{NS}:{name}", "P": f"{NS}:red_oxide_paint"}, f"{NS}:painted_{name}", 8)
    return r


def regulation_items():
    items = {
        "rust_flakes": {"template": "minecraft:gunpowder"},
        "red_oxide_pigment": {"template": "minecraft:redstone"},
        "red_oxide_paint": {"template": "minecraft:honeycomb", "max_stack_size": 16},
    }
    for name in RUSTING:
        items[f"painted_{name}"] = {
            "template": "minecraft:iron_ingot",
            "block": f"{NS}:{name}",
            "components": {"minecraft:block_state": {"rust": "painted"}},
        }
    return items


def loot(name):
    entry = {"type": "minecraft:item", "name": f"{NS}:{name}"}
    if name in LINE_MACHINES:
        # The drive carries the drop (machines spec 2.3).
        entry["condition"] = {"type": "minecraft:block_state_property", "block": f"{NS}:{name}", "properties": {"half": "lower"}}
    if name in RUSTING:
        entry = {"type": "minecraft:alternatives", "children": [
            {"type": "minecraft:item", "name": f"{NS}:painted_{name}",
             "condition": {"type": "minecraft:block_state_property", "block": f"{NS}:{name}", "properties": {"rust": "painted"}}},
            entry,
        ]}
    return {
        "type": "minecraft:block",
        "pools": [{"rolls": 1, "entries": [entry], "condition": {"type": "minecraft:survives_explosion"}}],
        "random_sequence": f"{NS}:blocks/{name}",
    }


# ---- 0.4: models, states, items, recipes and data --------------------------------------

FACTORY_ITEMS = {
    "iron_rod": {"template": "minecraft:iron_ingot"},
    "iron_gear": {"template": "minecraft:iron_ingot"},
    "rivet": {"template": "minecraft:iron_nugget"},
    "iron_dust": {"template": "minecraft:redstone"},
    "copper_dust": {"template": "minecraft:redstone"},
    "gold_dust": {"template": "minecraft:redstone"},
}


def slotted(model):
    """Face textures must name a slot (`#t0`), not a texture: a literal path renders as the missing texture."""
    model = copy.deepcopy(model)  # models share element dicts; rewriting them in place leaks slots across models
    slots = dict(model.get("textures", {}))
    names = {v: k for k, v in slots.items() if not v.startswith("#")}
    for e in model.get("elements", []):
        for face in e.get("faces", {}).values():
            t = face.get("texture", "")
            if t and not t.startswith("#"):
                if t not in names:
                    i = 0
                    while f"t{i}" in slots:
                        i += 1
                    names[t] = f"t{i}"
                    slots[names[t]] = t
                face["texture"] = "#" + names[t]
    if slots:
        model["textures"] = slots
    return model


def factory_models():
    m = {}
    log, log_end = "minecraft:block/stripped_oak_log", "minecraft:block/stripped_oak_log_top"
    iron, spruce, dark = "minecraft:block/iron_block", "minecraft:block/spruce_planks", "minecraft:block/dark_oak_planks"
    wool, bars, stripe = "minecraft:block/brown_wool", "minecraft:block/iron_bars", "minecraft:block/red_terracotta"
    frame = "minecraft:block/stripped_spruce_log"
    # Belts (north = the heading): a frame, the belt surface 6 px up, a red
    # oxide stripe across so movement reads; rollers are drawn.
    belt_flat = [
        el([0, 0, 0], [1, 5, 16], frame),
        el([15, 0, 0], [16, 5, 16], frame),
        el([1, 5, 0], [15, 6, 16], {"side": wool, "up": "#belt"}),
        el([1, 5.02, 7], [15, 6.02, 9], stripe, skip=("down",)),
    ]
    m["block/conveyor_belt"] = block_model({"belt": wool}, belt_flat, wool)
    # Slopes: the surface tilts 45° up toward the heading (north).
    slope = {
        "from": [1, 5, 0], "to": [15, 6, 22.6],
        "rotation": {"angle": 45, "axis": "x", "origin": [8, 5.5, 16]},
        "faces": {f: {"texture": "#belt"} for f in ["north", "south", "east", "west", "up", "down"]},
    }
    m["block/conveyor_belt_rising"] = block_model({"belt": wool}, [el([0, 0, 0], [1, 8, 16], frame), el([15, 0, 0], [16, 8, 16], frame), slope], wool)
    m["block/conveyor_belt_falling"] = m["block/conveyor_belt_rising"]
    # The diverter: a belt with an iron hinge on its flap side (the flap is drawn).
    m["block/diverter"] = block_model({"belt": wool, "hinge": iron}, belt_flat + [el([0, 5, 2], [1, 8, 14], "#hinge")], wool)
    m["block/diverter_right"] = block_model({"belt": wool, "hinge": iron}, belt_flat + [el([15, 5, 2], [16, 8, 14], "#hinge")], wool)
    # The bucket elevator: a timber shaft with a window of buckets.
    casing = [
        el([2, 0, 2], [14, 16, 3], {"side": spruce}),
        el([2, 0, 13], [14, 16, 14], {"side": spruce}),
        el([2, 0, 3], [3, 16, 13], {"side": spruce}),
        el([13, 0, 3], [14, 16, 13], {"side": spruce}),
        el([4, 0, 6], [12, 16, 10], {"side": "minecraft:block/iron_chain"}),
    ]
    m["block/bucket_elevator"] = block_model({}, casing, spruce)
    m["block/bucket_elevator_boot"] = block_model({}, casing + [el([1, 0, 1], [15, 4, 15], {"side": spruce})], spruce)
    m["block/bucket_elevator_head"] = block_model({}, casing + [el([1, 12, 1], [15, 16, 15], {"side": spruce}), el([5, 9, 0], [11, 14, 2], {"side": iron})], spruce)
    # Line machines: the top carries the built-in belt (north = the output)
    # under the machine's frame; moving parts are drawn.
    andesite = "minecraft:block/polished_andesite"
    belt = {"belt": wool}
    m["block/washer"] = block_model(belt, belt_flat + [
        el([1, 6, 1], [15, 14, 2], dark), el([1, 6, 14], [15, 14, 15], dark),
        el([0, 6, 2], [1, 14, 14], dark), el([15, 6, 2], [16, 14, 14], dark),
        el([1, 10, 0.8], [15, 11, 1], iron, skip=("down",)),
    ], dark)
    m["block/trip_hammer"] = block_model(belt, belt_flat + [
        el([0, 5, 6], [1, 16, 10], frame), el([15, 5, 6], [16, 16, 10], frame),
        el([0, 14, 6], [16, 16, 10], frame),
    ], frame)
    m["block/lathe"] = block_model(belt, belt_flat + [
        el([0, 5, 4], [1, 12, 12], iron), el([15, 5, 4], [16, 12, 12], iron),
        el([1, 9, 3], [4, 13, 13], andesite),
        el([12, 9, 4], [15, 12, 12], andesite),
    ], iron)
    m["block/sawmill"] = block_model(belt, belt_flat + [
        el([0, 5, 0], [1, 12, 16], {"side": spruce}), el([15, 5, 0], [16, 12, 16], {"side": spruce}),
        el([0, 12, 0], [16, 13, 5], spruce), el([0, 12, 11], [16, 13, 16], spruce),
    ], spruce)
    m["block/inline_grinder"] = block_model(belt, belt_flat + [
        el([0, 5, 2], [1, 14, 14], log), el([15, 5, 2], [16, 14, 14], log),
        el([1, 12, 2], [15, 14, 14], {"side": log, "up": "minecraft:block/smooth_stone"}),
    ], log)
    # The drive: a timber frame; the socket's hub faces north (or down).
    drive = [
        el([0, 0, 0], [2, 16, 2], frame), el([14, 0, 0], [16, 16, 2], frame),
        el([0, 0, 14], [2, 16, 16], frame), el([14, 0, 14], [16, 16, 16], frame),
        el([2, 13, 2], [14, 16, 14], {"side": spruce, "up": spruce, "down": spruce}),
        el([5, 5, 5], [11, 11, 11], iron),
    ]
    m["block/machine_drive"] = block_model({}, drive + [el([6, 6, 0], [10, 10, 5], andesite)], frame)
    m["block/machine_drive_down"] = block_model({}, drive + [el([6, 0, 6], [10, 5, 10], andesite)], frame)
    # The trommel (drum along x): a loose block, an end with a mouth, the middle.
    cradle = [el([0, 0, 0], [16, 3, 2], frame), el([0, 0, 14], [16, 3, 16], frame), el([4, 0, 4], [12, 2, 12], bars)]
    drum = [el([0, 4, 3], [16, 13, 4], bars), el([0, 4, 12], [16, 13, 13], bars), el([0, 12, 4], [16, 13, 12], bars), el([0, 3, 4], [16, 4, 12], bars)]
    hub = [el([6, 6, 0], [10, 10, 3], andesite)]
    m["block/trommel"] = block_model({}, cradle + [el([2, 3, 2], [14, 14, 14], bars)], bars)
    m["block/trommel_end"] = block_model({}, cradle + drum + [el([7, 4, 4], [9, 13, 12], iron)], bars)
    m["block/trommel_middle"] = block_model({}, cradle + drum, bars)
    m["block/trommel_end_socket"] = block_model({}, cradle + drum + [el([7, 4, 4], [9, 13, 12], iron)] + hub, bars)
    m["block/trommel_middle_socket"] = block_model({}, cradle + drum + hub, bars)
    for name in ["iron_pulley", "large_iron_pulley"]:
        m[f"block/{name}"] = {"textures": {"particle": iron}, "elements": []}
    m["item/iron_pulley"] = block_model(
        {"side": iron, "disc": iron},
        [bar("z", 0, 16, 2, "#side", "#side"), bar("z", 5, 11, 7, "#disc", "#disc")],
        iron,
    )
    m["item/large_iron_pulley"] = block_model(
        {"side": iron, "disc": "minecraft:block/polished_andesite"},
        [bar("z", 0, 16, 2, "#side", "#side"), bar("z", 5, 11, 8, "#disc", "#disc")],
        iron,
    )
    for name in ["conveyor_belt", "diverter", "bucket_elevator", "trommel"] + LINE_MACHINES:
        m[f"item/{name}"] = {"parent": f"{NS}:block/{name}"}
    for name in FACTORY_ITEMS:
        m[f"item/{name}"] = {"parent": "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{name}"}}
    return {k: slotted(v) for k, v in m.items()}


def factory_blockstate(name):
    if name in ("conveyor_belt", "diverter"):
        v = {}
        for f, r in NORTH_Y.items():
            for sl in ["flat", "rising", "falling"]:
                for d in ["none"] + HORIZONTAL:
                    if name == "conveyor_belt":
                        model = "conveyor_belt" if sl == "flat" else f"conveyor_belt_{sl}"
                        rr = dict(r)
                        if sl == "falling":
                            rr["y"] = (rr.get("y", 0) + 180) % 360
                        v[f"drive={d},facing={f},slope={sl}"] = dict({"model": f"{NS}:block/{model}"}, **rr)
                    else:
                        for flap in ["left", "right"]:
                            for pw in ["false", "true"]:
                                model = "diverter" if flap == "left" else "diverter_right"
                                v[f"drive={d},facing={f},flap={flap},powered={pw},slope={sl}"] = dict({"model": f"{NS}:block/{model}"}, **r)
        return {"variants": v}
    if name == "bucket_elevator":
        v = {}
        for f, r in NORTH_Y.items():
            for part in ["single", "boot", "casing", "head"]:
                model = {"boot": "bucket_elevator_boot", "head": "bucket_elevator_head"}.get(part, "bucket_elevator")
                for d in ["none"] + HORIZONTAL:
                    v[f"drive={d},facing={f},part={part}"] = dict({"model": f"{NS}:block/{model}"}, **r)
        return {"variants": v}
    if name == "trommel":
        v = {}
        for part in ["none", "end", "middle"]:
            for axis in ["x", "z"]:
                for sock in ["none"] + HORIZONTAL:
                    model = "trommel" if part == "none" else f"trommel_{part}"
                    if part != "none" and sock != "none":
                        model += "_socket"
                        # The hub faces north in the model.
                        y = NORTH_Y[sock].get("y", 0)
                    else:
                        y = 0 if axis == "x" else 90
                    rr = {"y": y} if y else {}
                    v[f"axis={axis},part={part},socket={sock}"] = dict({"model": f"{NS}:block/{model}"}, **rr)
        return {"variants": v}
    if name in LINE_MACHINES:
        v = {}
        for f, r in NORTH_Y.items():
            for sock in SOCKET_FACES:
                v[f"facing={f},half=upper,socket={sock}"] = dict({"model": f"{NS}:block/{name}"}, **r)
                if sock == "down":
                    v[f"facing={f},half=lower,socket={sock}"] = {"model": f"{NS}:block/machine_drive_down"}
                else:
                    v[f"facing={f},half=lower,socket={sock}"] = dict({"model": f"{NS}:block/machine_drive"}, **NORTH_Y[sock])
        return {"variants": v}
    return {"variants": {"": {"model": f"{NS}:block/{name}"}}}


def machine_recipe(kind, ingredient, results, revolutions, load, water=0.0, count=1):
    r = {"type": f"{NS}:{kind}", "ingredient": ingredient}
    if count != 1:
        r["count"] = count
    r["results"] = results
    r["revolutions"] = revolutions
    r["load"] = load
    r["water"] = water
    return r


def res(item, count=1, chance=1.0):
    o = {"id": item}
    if count != 1:
        o["count"] = count
    if chance != 1.0:
        o["chance"] = chance
    return o


WOOD_KINDS = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak"]
STEM_KINDS = ["crimson", "warped"]


def machine_recipes():
    """The four machine recipe types (machines spec 4.2 to 7.2); `sim::machine`
    holds the same tables and a test compares them."""
    out = {}
    for metal, by in [("iron", "minecraft:iron_nugget"), ("copper", "minecraft:gold_nugget"), ("gold", "minecraft:copper_nugget")]:
        dust = f"{NS}:{metal}_dust"
        out[f"washing/crushed_{metal}"] = machine_recipe(
            "washing", f"{NS}:crushed_{metal}", [res(dust), res(dust, 1, 0.3333), res(by, 1, 0.1)], 2.5, 4.0, 50.0)
    out["washing/iron_nugget"] = machine_recipe("washing", "minecraft:iron_nugget", [res(f"{NS}:rust_flakes")], 64.0, 2.0, 200.0)
    out["hammering/iron_ingot"] = machine_recipe("hammering", "minecraft:iron_ingot", [res(f"{NS}:iron_plate")], 2.0, 8.0)
    out["hammering/copper_ingot"] = machine_recipe("hammering", "minecraft:copper_ingot", [res(f"{NS}:copper_plate")], 2.0, 8.0)
    out["hammering/iron_rod"] = machine_recipe("hammering", f"{NS}:iron_rod", [res(f"{NS}:rivet", 8)], 3.0, 8.0)
    out["turning/iron_ingot"] = machine_recipe("turning", "minecraft:iron_ingot", [res(f"{NS}:iron_rod", 2)], 8.0, 4.0)
    out["turning/iron_plate"] = machine_recipe("turning", f"{NS}:iron_plate", [res(f"{NS}:iron_gear")], 12.0, 4.0)
    out["turning/copper_plate"] = machine_recipe("turning", f"{NS}:copper_plate", [res(f"{NS}:copper_pipe", 3)], 8.0, 3.0)
    for kind in WOOD_KINDS + STEM_KINDS:
        if kind in STEM_KINDS:
            logs = [f"{kind}_stem", f"stripped_{kind}_stem", f"{kind}_hyphae", f"stripped_{kind}_hyphae"]
        else:
            logs = [f"{kind}_log", f"stripped_{kind}_log", f"{kind}_wood", f"stripped_{kind}_wood"]
        for log in logs:
            out[f"sawing/{log}"] = machine_recipe("sawing", f"minecraft:{log}", [res(f"minecraft:{kind}_planks", 6)], 4.0, 6.0)
    out["sawing/bamboo_block"] = machine_recipe("sawing", "minecraft:bamboo_block", [res("minecraft:bamboo_planks", 3)], 4.0, 6.0)
    out["sawing/planks"] = machine_recipe("sawing", "#minecraft:planks", [res("minecraft:stick", 3)], 2.0, 3.0)
    return out


def factory_recipes():
    p, t, g, r, s, i = f"{NS}:iron_plate", f"{NS}:rivet", f"{NS}:iron_gear", f"{NS}:iron_rod", f"{NS}:iron_shaft", "minecraft:iron_ingot"
    planks, logs = "#minecraft:planks", "#minecraft:logs"
    c, d = f"{NS}:copper_pipe", "minecraft:redstone"
    belt = f"{NS}:conveyor_belt"
    out = {
        # Progression 4.2: replaced in place (same ids).
        "iron_shaft_from_rods": shaped(["R", "R"], {"R": r}, s, 4),
        "gearbox": shaped(["PTP", "G G", "PTP"], {"P": p, "T": t, "G": g}, f"{NS}:gearbox"),
        "iron_bevel_gear": shaped(["GSG"], {"G": g, "S": s}, f"{NS}:iron_bevel_gear", 2),
        "clutch": shaped(["SGS", " D "], {"S": s, "G": g, "D": d}, f"{NS}:clutch"),
        "governor": shaped(["I I", " G ", "PVP"], {"I": i, "G": g, "P": p, "V": f"{NS}:manual_valve"}, f"{NS}:governor"),
        "furnace_damper": shaped(["PTP", "PDP"], {"P": p, "T": t, "D": d}, f"{NS}:furnace_damper"),
        "iron_boiler": shaped(["PTP", "P P", "PTP"], {"P": p, "T": t}, f"{NS}:iron_boiler"),
        "storage_tank": shaped(["PTP", "I I", "PTP"], {"P": p, "T": t, "I": i}, f"{NS}:storage_tank", 2),
        "heat_exchanger": shaped(["PTP", "C C", "PTP"], {"P": p, "T": t, "C": c}, f"{NS}:heat_exchanger"),
        "tachometer": shaped(["NAN", "NGN", " D "], {"N": "minecraft:iron_nugget", "A": "minecraft:glass_pane", "G": g, "D": d}, f"{NS}:tachometer"),
        # Logistics 13.2.
        "conveyor_belt": shaped(["FFF", "PSP"], {"F": f"#{NS}:belt_fabrics", "P": planks, "S": f"{NS}:wooden_shaft"}, f"{NS}:conveyor_belt", 6),
        "bucket_elevator": shaped(["PBP", "PGP", "PBP"], {"P": planks, "B": "minecraft:bucket", "G": g}, f"{NS}:bucket_elevator", 4),
        "diverter": shapeless([f"{NS}:conveyor_belt", p, d], f"{NS}:diverter"),
        # Machines 13.2.
        "trip_hammer": shaped(["CS ", "LBL", "PPP"], {"C": belt, "S": f"{NS}:wooden_shaft", "L": logs, "B": "minecraft:iron_block", "P": p}, f"{NS}:trip_hammer"),
        "lathe": shaped(["PSP", "IFI", "LCL"], {"P": p, "S": s, "I": i, "F": "minecraft:flint", "L": logs, "C": belt}, f"{NS}:lathe"),
        "washer": shaped(["PGP", "KUK", "PCP"], {"P": planks, "G": g, "K": c, "U": "minecraft:bucket", "C": belt}, f"{NS}:washer"),
        "sawmill": shaped(["IGI", "PSP", "PCP"], {"I": i, "G": g, "P": planks, "S": s, "C": belt}, f"{NS}:sawmill"),
        "inline_grinder": shapeless([f"{NS}:grinder", belt, planks], f"{NS}:inline_grinder"),
        "trommel": shaped(["BBB", "G S", "BBB"], {"B": "minecraft:iron_bars", "G": g, "S": s}, f"{NS}:trommel", 3),
        # Progression 4.4.
        "iron_pulley": shaped([" I ", "ISI", " I "], {"I": i, "S": s}, f"{NS}:iron_pulley"),
        "large_iron_pulley": shaped(["III", "ISI", "III"], {"I": i, "S": s}, f"{NS}:large_iron_pulley"),
    }
    # Dust smelts in half the raw ore's time (machines 13.2).
    for metal in ["iron", "copper", "gold"]:
        for kind, time in [("smelting", 100), ("blasting", 50)]:
            out[f"{metal}_ingot_from_{metal}_dust_{kind}"] = {
                "type": f"minecraft:{kind}",
                "category": "misc",
                "ingredient": f"{NS}:{metal}_dust",
                "result": {"id": f"minecraft:{metal}_ingot"},
                "experience": 0.7,
                "cookingtime": time,
            }
    return out


def factory_data():
    # Machine menus (machines spec 2.5): one hidden inbox slot, information
    # only; data [progress or drum count, rpm, status, max, tank, 0, min,
    # strikes needed or drum capacity].
    for name in LINE_MACHINES + ["trommel"]:
        write(f"data/{NS}/horizon/menu/{name}.json", {"slots": 1, "data_slots": 8})
    for path, recipe in machine_recipes().items():
        write(f"data/{NS}/recipe/{path}.json", recipe)
    tags = {
        "dusts": [f"{NS}:{m}_dust" for m in ["iron", "copper", "gold"]],
        "belt_fabrics": ["minecraft:leather", "#minecraft:wool"],
        "trommel/fine": ["minecraft:sand", "minecraft:red_sand", f"#{NS}:dusts"],
        "trommel/medium": [
            f"#{NS}:crushed_ores",
            "minecraft:gravel", "minecraft:flint", "minecraft:iron_nugget", "minecraft:gold_nugget",
            "minecraft:copper_nugget", "minecraft:redstone", "minecraft:bone_meal", "minecraft:clay_ball",
            "minecraft:coal", "minecraft:charcoal", f"{NS}:rust_flakes", f"{NS}:red_oxide_pigment",
            "minecraft:raw_iron", "minecraft:raw_copper", "minecraft:raw_gold",
        ],
        "plates/iron": [f"{NS}:iron_plate"],
        "plates/copper": [f"{NS}:copper_plate"],
        "rods/iron": [f"{NS}:iron_rod"],
        "gears/iron": [f"{NS}:iron_gear"],
        "rivets": [f"{NS}:rivet"],
    }
    for name, values in tags.items():
        write(f"data/{NS}/tags/item/{name}.json", {"replace": False, "values": values})
    # Machine hazards (machines 5.4, 7.3): plain damage, armour helps.
    write(f"data/{NS}/damage_type/machine.json", {"message_id": f"{NS}.machine", "exhaustion": 0.1, "scaling": "when_caused_by_living_non_player"})


def main():
    for name, recipe in grinding().items():
        write(f"data/{NS}/recipe/grinding/{name}.json", recipe)
    states = {}
    for name, (decl, tool, _) in BLOCKS.items():
        # Every part drops by hand (a bearing whose mount goes drops too);
        # the tool tags only make mining faster.
        decl["properties"]["requires_tool"] = False
        write(f"data/{NS}/horizon/block/{name}.json", decl)
        if name in CREATIVE_BLOCKS:
            states[name] = creative_blockstate(name)
        elif name in REGULATION_BLOCKS:
            states[name] = regulation_blockstate(name)
        elif name in STEAM_BLOCKS:
            states[name] = steam_blockstate(name)
        elif name in FACTORY_BLOCKS:
            states[name] = factory_blockstate(name)
        else:
            states[name] = blockstate(name)
        write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}})
        # The 26.3 loot format: one `condition` object with a `type`.
        write(f"data/{NS}/loot_table/blocks/{name}.json", loot(name))
    # The grinder's menu: input slot 0, results 1-4 (players only take from
    # those), data slots [progress, rpm, status, max].
    write(f"data/{NS}/horizon/menu/grinder.json", {"slots": 5, "data_slots": 4, "results": [1, 2, 3, 4]})
    # The governor's: one hidden slot (menus need one), data [target, sens, speed, ball, throttle, engines, swing].
    write(f"data/{NS}/horizon/menu/governor.json", {"slots": 1, "data_slots": 7})
    # Creative blocks: one hidden slot, data [setting 0, setting 1, status 0, status 1].
    for name in CREATIVE_BLOCKS:
        write(f"data/{NS}/horizon/menu/{name}.json", {"slots": 1, "data_slots": 4})
    # Readouts: one hidden slot, data [value, red line, extra, head, range, 120 samples].
    for name in ["tachometer", "pressure_gauge", "sight_glass"]:
        write(f"data/{NS}/horizon/menu/{name}.json", {"slots": 1, "data_slots": 125})
    # Set pressure screens (game/setpoint.rs): the damper's data [set, bar, opening, status, red line,
    # head, range, 120 samples], the emergency valve's [set, bar, red line, state, mB/s].
    write(f"data/{NS}/horizon/menu/furnace_damper.json", {"slots": 1, "data_slots": 127})
    write(f"data/{NS}/horizon/menu/emergency_valve.json", {"slots": 1, "data_slots": 5})
    items = dict(ITEMS)
    items.update(FACTORY_ITEMS)
    items.update(regulation_items())
    for name, decl in items.items():
        write(f"data/{NS}/horizon/item/{name}.json", decl)
        write(f"assets/{NS}/items/{name}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}})
    models = {}
    for path, model in list(models_all()):
        models[path] = model
    rust_everything(models, states)
    for name, bs in states.items():
        write(f"assets/{NS}/blockstates/{name}.json", bs)
    for path, model in models.items():
        write(f"assets/{NS}/models/{path}.json", model)
    axe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "axe"]
    pickaxe = [f"{NS}:{n}" for n, (_, t, _) in BLOCKS.items() if t == "pickaxe"]
    write("data/minecraft/tags/block/mineable/axe.json", {"replace": False, "values": axe})
    write("data/minecraft/tags/block/mineable/pickaxe.json", {"replace": False, "values": pickaxe})
    write(f"data/{NS}/tags/block/parts.json", {"replace": False, "values": [f"{NS}:{n}" for n in BLOCKS]})
    write(f"data/{NS}/tags/item/crushed_ores.json", {"replace": False, "values": [f"{NS}:crushed_{m}" for m in ["iron", "copper", "gold"]]})
    steam_data()
    factory_data()
    for name, recipe in list(recipes().items()) + list(steam_recipes().items()) + list(regulation_recipes().items()) + list(factory_recipes().items()):
        write(f"data/{NS}/recipe/{name}.json", recipe)
    order = [n for n in BLOCKS if n not in CREATIVE_BLOCKS] + list(items)
    write(f"data/{NS}/horizon/creative_tab/main.json", {"title": "Fire, Rust and Steam", "icon": f"{NS}:gearbox", "items": [f"{NS}:{n}" for n in order]})
    write(f"data/{NS}/horizon/creative_tab/creative.json", {"title": "Fire, Rust and Steam: debugging", "icon": f"{NS}:creative_engine", "items": [f"{NS}:{n}" for n in CREATIVE_BLOCKS]})


def models_all():
    return list(models().items()) + list(steam_models().items()) + list(regulation_models().items()) + list(creative_models().items()) + list(factory_models().items())


if __name__ == "__main__":
    main()
