---
description: Canonical SunTDU workspace architecture and ownership boundaries.
---

# SunTDU architecture

Rust is the primary implementation language.

## Workspace ownership
- `game/` is the copied Test Drive Unlimited installation and should stay as clean as practical.
- Project code and helper programs live outside `game/`.
- `crates/tdu-formats` owns binary serialization/parsing details.
- `crates/tdu-assets` will own confirmed semantic relationships between formats, such as model -> material -> texture. Do not duplicate binary parsing there.
- `crates/tdu-cli` exposes command-line workflows built on the libraries.
- `crates/tdu-viewer` is the interactive viewer/editor path and must consume canonical library APIs rather than implement private parsers.
- `crates/suntdu-runtime` is reserved for the 32-bit in-process mod/ASI path for the original game.
- `research/` is for external reference checkouts such as OpenTDU, TDUF and TDUF.next.
- Research findings and decisions belong in `sun-memory/`, not a `notes/` directory.
- `assets/source/` owns authored reusable procedural inputs such as masks and base layers.
- `assets/generated/` is a disposable deterministic cache and is ignored by Git.
- `test-assets/` is for small legal/synthetic fixtures. Do not commit copied game assets by default.

## Reference policy
OpenTDU, TDUF and TDUF.next are research references, not product/runtime dependencies unless a later explicit decision changes that.

When porting a discovered binary contract, validate it against real local 1.66A data before treating it as canonical.