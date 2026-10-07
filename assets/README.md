# SunTDU assets

source/ is for small authored procedural inputs such as reusable masks, base materials and layer resources.

generated/ is a disposable deterministic cache. It is not a source of truth and is ignored by Git. Generated outputs must be reproducible from recipe version, world or object identity, seed and source-layer inputs.