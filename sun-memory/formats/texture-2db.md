---
description: Confirmed first-pass layout and decoding path for standalone TDU .2DB BMAP textures.
---

# Standalone .2DB texture findings

## Confirmed local samples
The copied 1.66A install contains exactly two standalone `.2DB` files:
- `game/Euro/Bnk/FX/setbush.2DB` — 349600 bytes
- `game/Euro/Bnk/FX/setgrass.2DB` — 174816 bytes

Both start with a TDU BMAP header whose bytes at offset 12 are ASCII `.2DBBMAP`.

For both local samples:
- bytes 32..40 contain the texture name;
- little-endian u16 width is at offset 40;
- little-endian u16 height is at offset 42;
- byte 46 is the mip count;
- byte 47 currently mirrors the mip count;
- compressed image data starts at byte 88;
- the file ends with an 8-byte footer.

Observed metadata:
- `SETBUSH`: 1024x256, 7 mips
- `SETGRASS`: 1024x128, 6 mips

The payload length between the 88-byte header and 8-byte footer exactly matches a complete BC3/DXT5 mip chain for both samples:
- `SETBUSH`: 349504 bytes
- `SETGRASS`: 174720 bytes

This is stronger evidence than guessing from a format byte, so the initial parser infers BC1/BC3 from dimensions, mip count and exact payload size.

## Implemented path
`tdu-formats::texture_2db` parses this layout and decodes the base mip for BC1/BC3.
`tdu-cli texture-export` writes the decoded base mip as PNG.
`tdu-viewer` uses the same parser/decoder for interactive display.

## Boundary
Only the standalone `.2DB BMAP` path above is confirmed. Do not assume BNK-contained textures share the same outer 88-byte layout; BNK extraction is separate work.