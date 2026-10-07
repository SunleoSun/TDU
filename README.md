# SunTDU

Rust tooling, asset research and runtime experiments for Test Drive Unlimited 1.66A.

The original game copy stays under game/. Project code and helper programs live at the workspace root.

## Crates

- tdu-formats — binary file format parsing and decoding.
- tdu-assets — semantic asset assembly plus deterministic procedural material generation.
- tdu-cli — command-line inspection, conversion and procedural generation tools.
- tdu-viewer — interactive asset lab for TDU textures and procedural material previews.
- suntdu-runtime — future 32-bit in-process runtime mod/ASI code.
- suntdu-d3d9-probe — D3D9 instrumentation used to validate renderer and memory behavior.

## Assets

- assets/source/ — authored reusable source layers and masks.
- assets/generated/ — disposable deterministic generated cache; ignored by Git.
- test-assets/ — small synthetic fixtures for format tests.

Research reference checkouts belong under research/. Findings and decisions belong in sun-memory/.

## Procedural material lab

Start the GUI from the workspace root:

    Start TDU Texture Viewer.cmd

The default Procedural lab shows a continuous 3x3 asphalt region. Each chunk has a stable seed, but material fields are sampled in world space, so aggregate grains, weathering and cracks continue across chunk boundaries.

The asphalt v7 prototype adds:
- two deterministic world-space scatter layers with 0..3 grain candidates per spatial bucket, strong position jitter, roughly +/-30% size variation and per-stone grey/warm/dark colour variation;
- sparse world-space damage zones, so cracks appear in patches instead of uniformly;
- sparse deterministic crack polylines with angular segments, occasional gaps and short branches instead of Voronoi-like polygon boundaries;
- deterministic per-crack length variation from short to roughly three times longer paths, with longer segment lengths than v6 so long cracks read clearly at preview scale;
- separate crack darkness, crack-zone coverage and crack-density controls;
- higher crack-source probability and wider damage-zone mapping, so Crack zones=1 and Crack density=1 now produce substantially more cracking;
- biome-ready road tone from light grey to dark asphalt, applied to binder and aggregate together;
- weathering colour follows road tone instead of using one fixed brightness;
- material controls are limited in the viewer to the visually realistic ranges found during tuning: aggregate 0.70-1.00, macro 0.00-0.30, dirt 0.00-0.40 and crack darkness 0.25-0.50;
- macro/dirt modulation is applied after aggregate synthesis, so stone contrast does not hide those material controls;
- parallel CPU generation with Rayon.

The lab supports:
- deterministic world seed and chunk coordinates;
- 128, 256, 512 and 1024 px chunk resolutions;
- aggregate, road tone, macro, dirt, crack darkness, crack-zone and crack-density controls;
- optional chunk-boundary overlay and zoom;
- PNG preview export to assets/generated/previews/.

Changing recipe sliders now marks the preview dirty instead of regenerating on every mouse movement. Press Regenerate to apply the new recipe.

Generate one deterministic 1024x1024 chunk from the CLI:

cargo run -p tdu-cli -- procedural-asphalt 0x53554E5444550001 0 0 1024 assets/generated/asphalt-0-0.png

Generate the same continuous 3x3 region used by the viewer:

cargo run -p tdu-cli -- procedural-region 0x53554E5444550001 0 0 256 1 assets/generated/asphalt-3x3.png

## TDU texture path

Two standalone textures currently used for format validation are:

- game/Euro/Bnk/FX/setgrass.2DB
- game/Euro/Bnk/FX/setbush.2DB

Inspect one:

    cargo run -p tdu-cli -- texture-info game/Euro/Bnk/FX/setgrass.2DB

Export the base mip to PNG:

    cargo run -p tdu-cli -- texture-export game/Euro/Bnk/FX/setgrass.2DB local-output/setgrass.png

The TDU .2DB view now includes zoom presets, a zoom slider, checkerboard background and alpha-only inspection.