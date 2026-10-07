---
description: Current capabilities and intended role of the SunTDU Asset Lab viewer.
---

# SunTDU Asset Lab

The GUI crate is an asset lab for both original TDU resources and deterministic procedural material work.

## Procedural lab
The default view renders the asphalt v7 prototype from tdu-assets as one continuous 3x3 chunk region.

Controls currently expose:
- world seed;
- center chunk coordinates;
- 128, 256, 512 and 1024 px per-chunk resolutions;
- aggregate grain strength;
- road tone from light grey to dark asphalt;
- macro variation;
- dirt;
- crack darkness;
- crack-zone coverage;
- crack density inside damaged zones;
- optional chunk-boundary overlay;
- zoom;
- preview PNG export to assets/generated/previews/.

The v7 asphalt preview keeps the scattered aggregate and biome-ready road tone, removes Tint drift, lengthens crack segments, raises crack-source probability, and widens the damage-zone mapping so maximum Crack zones and Crack density produce substantially more visible cracking while remaining seamless between chunks.

Viewer sliders are constrained to the tuned realistic ranges: Aggregate grains 0.70-1.00, Macro variation 0.00-0.30, Dirt 0.00-0.40 and Crack darkness 0.25-0.50. Crack zones and Crack density remain normalized 0.00-1.00 controls.

To avoid UI stalls during editing, sliders and coordinate changes mark the preview dirty instead of regenerating on every mouse movement. Press Regenerate to apply changed settings. Next seed deliberately regenerates immediately.

Default preview resolution is 256 px per chunk. The complete 768x768 3x3 v7 region measured about 28.5 ms in the optimized dev CLI path and 28.6 ms in release on this machine. A Zen3-specific build is not currently required.

The viewer does not implement private procedural logic. It calls the canonical tdu-assets procedural API.

## TDU texture inspection
The .2DB view opens standalone TDU .2DB textures through tdu-formats.

Default validation sample:
game/Euro/Bnk/FX/setgrass.2DB

It includes:
- zoom presets and a 0.25x through 8x slider;
- checkerboard background;
- RGBA and alpha-only views.

Validated standalone textures remain:
- SETGRASS: 1024x128, 6 mips, BC3/DXT5;
- SETBUSH: 1024x256, 7 mips, BC3/DXT5.

## Next viewer expansion
The next data-driven expansion should connect confirmed TDU world or sector identities to the stable procedural seed contract and a world/environment control layer. That layer should derive road tone, dirt, crack-zone coverage/density and related material parameters from location/biome, then feed the same canonical tdu-assets recipe used by the viewer and future asset host.