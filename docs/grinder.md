# The grinder

Spec: [specs/grinder.md](../specs/grinder.md).

- Works between **12 and 96 RPM**, twice as fast at 96 as at 48. Pulls **6 Nm** while it has work (3 Nm for
  wool and bones), even below 12 RPM. Above 96 RPM it overspeeds and jams.
- Right-click opens it (with anything but the wrench). Input on the left, four outputs on the right, a
  progress arrow, a speed bar (0–128 RPM, working band green) and a status line.
- Hoppers and droppers insert into the input; it **pushes** one item every 8 ticks into the inventory below;
  a hopper below can also pull the outputs. Comparator: the usual container signal.
- The wrench rotates its socket (the side the shaft connects to).

## Recipes (0.1)

Revolutions are turns of the grinder per item.

| Input | Output | Revolutions |
| --- | --- | --- |
| Raw iron / copper / gold | 1 crushed ore + 50% one more | 4 |
| Block of raw iron / copper / gold | 9 crushed ore + 50% nine more | 36 |
| Cobblestone, cobbled deepslate | Gravel | 2 |
| Gravel | Sand + 10% flint | 2 |
| Any wool | 4 string | 2 |
| Bone | 4 bone meal + 50% one more | 2 |

Crushed ore smelts (or blasts) into one ingot.

Recipes are data: `data/fras/recipe/grinding/*.json`, type `fras:grinding`. A data pack can add, change or
remove them:

```json
{
  "type": "fras:grinding",
  "ingredient": "minecraft:raw_iron",
  "results": [{ "id": "fras:crushed_iron" }, { "id": "fras:crushed_iron", "chance": 0.5 }],
  "revolutions": 4,
  "load": 6
}
```

`ingredient` is an item id or `#tag`; `count` (input items per recipe) defaults to 1; each result has `id`,
`count` (1) and `chance` (1.0); `load` defaults to 6 Nm. 1 to 4 results.
