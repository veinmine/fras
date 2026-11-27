#!/usr/bin/env python3
"""Writes tools/data/vanilla-crafting.json: vanilla's shaped and shapeless
crafting recipes and item tags, expanded to items, for the recipe clash test
(src/game/recipe_tree.rs). The data comes from misode/mcmeta, a dump of each
game version's data.

    python3 tools/vanilla_recipes.py [version]   # default: horizon-mod.toml's game
"""
import json
import os
import re
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def fetch(version, path):
    url = f"https://raw.githubusercontent.com/misode/mcmeta/{version}-summary/data/{path}/data.min.json"
    with urllib.request.urlopen(url) as r:
        return json.load(r)


def main():
    if len(sys.argv) > 1:
        version = sys.argv[1]
    else:
        toml = open(os.path.join(ROOT, "horizon-mod.toml")).read()
        version = re.search(r'^game = "([^"]+)"', toml, re.M).group(1)
    recipes = fetch(version, "recipe")
    tags = {f"minecraft:{k}": v["values"] for k, v in fetch(version, "tag/item").items()}

    def items(x):
        if isinstance(x, list):
            return sorted({i for y in x for i in items(y)})
        if isinstance(x, dict):
            x = x.get("item") or x.get("id") or "#" + x["tag"]
        if x.startswith("#"):
            return sorted({i for v in tags[x[1:]] for i in items(v if isinstance(v, str) else v["id"])})
        return [x]

    out = []
    for name, r in sorted(recipes.items()):
        if r["type"] == "minecraft:crafting_shaped":
            out.append({"id": f"minecraft:{name}", "pattern": r["pattern"], "key": {k: items(v) for k, v in r["key"].items()}})
        elif r["type"] == "minecraft:crafting_shapeless":
            out.append({"id": f"minecraft:{name}", "ingredients": [items(i) for i in r["ingredients"]]})
    path = os.path.join(ROOT, "tools/data/vanilla-crafting.json")
    with open(path, "w") as f:
        expanded = {k: items("#" + k) for k in sorted(tags)}
        json.dump({"version": version, "tags": expanded, "recipes": out}, f, separators=(",", ":"))
        f.write("\n")
    print(f"{path}: {len(out)} recipes from {version}")


if __name__ == "__main__":
    main()
