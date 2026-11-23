# What FRAS waits for upstream

The Horizon watch (`.github/workflows/horizon-watch.yml`, every Monday and on demand) opens a
`horizon-api` issue when the mod API adds, changes or removes an item matching a rule in
[`.github/horizon-watchlist.txt`](../.github/horizon-watchlist.txt), and a `horizon-compat` issue when the
pinned mod stops fitting the newest Nightly. The rules are the gaps below: drop a rule once its gap is
closed.

| Gap | What FRAS does now | Rule |
| --- | --- | --- |
| A recipe viewer for the `fras:grinding` recipes (the Recipes viewer port, horizon-mods #1214) | Recipes are listed in [grinder.md](grinder.md) | `recipe_?viewer…` |
| FRAS's own recipe pages in the item panel, as a fallback | Same | `panel_recipes\|recipe_page` |
| Rotating parts drawn on the animation clock | `draw.spin` with a phase kept per block | `block_entity_(submit\|render\|spin\|keep)…` |
| Network membership without ticking, pausing in unloaded chunks | `block_entity_on_load` and `on_chunk_unloaded` | `on_chunk_unloaded\|block_entity_on_load\|should_tick_blocks` |
| The crank's copper golem (0.4, optional) | Players only | `copper_golem\|golem` |

Also wanted, not yet a rule: a per-side block container (the grinder's socket side and recipe-only
insertion) and a hook for items landing on a block (tossing ore onto the grinder).
