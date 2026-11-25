# Cirno and Neco-Arc decals

These two give-only decals use the stock 0.26g flag cloth, specular and fresnel maps and the Snufkin decal builder. The two RGBA PNGs in `cutouts/` are the inputs for the diffuse textures and the lobby icons.

| Gallery | Item | Dictionary ID | Decal | Lobby cell | Artwork |
|---|---|---:|---|---|---|
| R77 | Bottled fumo | 2672 | 4_79 | [1, 21] | [Kame (kamepan44231)](https://danbooru.donmai.us/posts/5610210) |
| N3 | Neco-Arc | 2675 | 4_82 | [1, 24] | [Melty Blood: Type Lumina idle sprite, first frame of the selected sticker](https://tenor.com/view/neco-arc-melty-blood-type-lumina-idle-cat-gif-26528718) |

Build with the dependencies listed in [the Snufkin pack builder](../snufkin-decals/make_pack.py) and the same stock inputs.

```sh
python3 -I srp-026g/scripts/fumo-neco-decals/make_pack.py \
    --stock /path/to/extracted-4_41 --out /path/to/packs/fumo-neco-decals-3
```
