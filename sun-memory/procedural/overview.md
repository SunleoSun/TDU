---
description: Canonical design and current implementation state for deterministic seamless procedural textures.
---

# Procedural texture pipeline

## Canonical model
Procedural material generation belongs to tdu-assets so the viewer, future 64-bit asset host and runtime integration use one implementation.

A texture recipe currently contains:
- recipe version;
- world seed;
- texels per chunk;
- asphalt/road tone, where 0 is light grey and 1 is dark asphalt;
- macro variation strength;
- dirt strength;
- crack darkness/strength;
- crack-zone coverage;
- crack density inside damaged zones;
- aggregate-grain strength.

Terrain-like materials are sampled in continuous world/chunk space rather than generated from isolated per-chunk random images. Hashed lattice anchors are smoothly interpolated across world space. This gives stable local variation without hard image discontinuities at chunk boundaries.

The texture resolution only changes sampling density. It must not change the underlying world-space pattern. Tests compare matching world positions across different chunk resolutions.

For discrete map objects, stable_object_seed derives a deterministic seed from world seed, object id, material slot and recipe version. The same semantic object therefore keeps the same variation after unloading, restarting or regenerating.

## Cache ownership
assets/source contains authored reusable layer inputs.

assets/generated is a disposable deterministic cache and is ignored by Git. Generated data is not canonical state. It must be reproducible from source layers plus recipe version and stable identity/seed.

chunk_cache_key includes the stable chunk seed, resolution and all current recipe parameters, including asphalt tone and crack density.

## Asphalt prototype v7
Recipe version 7 keeps the v6 world-space aggregate and biome-ready road tone, removes tint drift, and retunes cracks plus viewer control ranges around the visually realistic operating window.

The generated material composes:
- a neutral asphalt binder/base;
- two world-space scatter aggregate layers with zero to three jittered grain candidates per spatial bucket, roughly +/-30% size variation, irregular aspect/shear and per-stone grey, warm, bright and dark colour variation;
- a global asphalt tone applied to the composed binder plus aggregate: 0.0 is light grey and 1.0 is dark asphalt;
- lower-frequency macro brightness variation;
- dirt/weathering applied after tone and aggregate, with dirt base luminance derived from asphalt tone so dark roads receive darker dirt;
- high-frequency micro variation;
- a low-frequency damage field that decides where cracking is present (Crack zones);
- an independent crack-source density scalar controlling how many crack polylines appear inside damaged areas (Crack density), with higher source probability at the top of the range;
- sparse deterministic crack polylines with angular segments, occasional gaps and short branches.

Each main crack has a stable source-derived segment count from 4 through 9. V7 raises per-segment length from the v6 0.070-0.090 cell range to 0.095-0.120, preserving roughly 2-3x total-length variation while making long cracks visibly longer. Branch segments are also lengthened. Crack identity and geometry remain world-space deterministic, so independently generated chunks stay seamless.

Default v7 parameters use asphalt tone 0.50, aggregate 0.92, macro 0.18, dirt 0.18, crack darkness 0.38, crack-zone coverage 0.75 and crack density 0.70. The viewer constrains tuned material controls to aggregate 0.70-1.00, macro 0.00-0.30, dirt 0.00-0.40 and crack darkness 0.25-0.50. These values are intended to become biome/environment inputs rather than permanent global constants.

## CPU performance
Region generation is parallelized by rows with Rayon.

Measured on the current development machine using the CLI generation timer for the v7 3x3 region at 256 px/chunk (768x768 total):
- optimized dev path: 28.5 ms;
- release path with thin LTO: 28.6 ms.

The release path remains fast enough for the current asset-lab workflow. A Zen3-specific build is not currently required; GPU generation remains a later option if runtime-scale workloads require it.

The viewer does not regenerate continuously while a procedural slider is dragged. Parameter changes mark the preview dirty; Regenerate applies them. Next seed still regenerates deliberately.

## CLI
Generate one deterministic chunk:
cargo run -p tdu-cli -- procedural-asphalt <seed> <chunk-x> <chunk-y> <resolution> <output.png>

Generate a continuous multi-chunk region for seam/performance inspection:
cargo run -p tdu-cli -- procedural-region <seed> <center-x> <center-y> <resolution> <radius> <output.png>

Validated ignored previews include:
- assets/generated/previews/asphalt-v7-3x3-256-dev.png
- assets/generated/previews/asphalt-v7-3x3-256-release.png

## Next integration boundary
Do not bind this generator to guessed TDU sector IDs yet. First recover canonical world object/sector identities from actual TDU data. Once confirmed, map those IDs to stable seeds and a world/environment layer that derives road tone, dirt, crack coverage/density and related recipe parameters from location/biome while preserving the same canonical recipe API.