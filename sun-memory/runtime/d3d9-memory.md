---
description: Confirmed TDU 1.66A Direct3D9 texture allocation, address-space, large-texture, and cross-process sharing findings on the current Windows 11 machine.
---

# D3D9 texture memory findings

## Original executable
The tested executable is `game/TestDriveUnlimited.exe` version MC 1.66A.

PE inspection confirms:
- x86 / PE32 (`Machine=0x014C`, optional-header magic `0x010B`);
- PE characteristics `0x010F`;
- `IMAGE_FILE_LARGE_ADDRESS_AWARE` is **not** set.

The executable imports standard `d3d9.dll` `Direct3DCreate9` and `D3DPERF_SetOptions`. It does not import `Direct3DCreate9Ex`. No D3DX texture-loader imports were found in the executable.

The 32-bit Windows 11 system D3D9 DLL does export both `Direct3DCreate9` and `Direct3DCreate9Ex`, so D3D9Ex exists on the host but TDU does not currently request it.

## Live TDU device observation
A temporary x86 `d3d9.dll` proxy was used only for instrumentation and removed after the test.

In the working `-w` launch path, TDU requested a standard D3D9 device with:
- windowed = 1;
- backbuffer = 1280x720;
- format = 22;
- refresh = 0;
- presentation interval = `0x80000000`.

This independently confirms that the current borderless launcher only stretches a 1280x720 D3D backbuffer; it does not change the render resolution.

A real TDU texture creation observed after startup was:
- 256x256;
- 1 mip level;
- usage 0;
- format 26;
- pool 1 (`D3DPOOL_MANAGED`);
- no shared-handle pointer.

This single observation does not prove every TDU texture uses MANAGED, but it proves the production path uses MANAGED for at least some textures.

## Managed-texture memory cost inside the real TDU device
Controlled DXT5 textures with a full mip chain were created on TDU's own live D3D9 device, touched through `LockRect`, measured, then released.

Measured process-private-memory deltas:
- 1024x1024: 1,400,832 bytes;
- 2048x2048: 5,611,520 bytes;
- 4096x4096: 22,421,504 bytes;
- 8192x8192: 89,665,536 bytes.

All creations and locks succeeded. The four textures retained together increased private memory by 119,099,392 bytes. After release, private memory returned to within about 20-40 KiB of the pre-test baseline.

These deltas closely match the compressed DXT5 full-mip-chain size, confirming that `D3DPOOL_MANAGED` consumes 32-bit process private address space for approximately the complete compressed texture chain.

## DEFAULT vs MANAGED process-memory comparison
A standalone x86 D3D9 probe held four 8192x8192 DXT5 full-mip-chain textures for measurement.

Baseline:
- private = 31.41 MiB;
- virtual = 389.07 MiB.

Four textures in pool 0 (`D3DPOOL_DEFAULT`):
- private = 31.45 MiB;
- virtual = 389.04 MiB.

Four textures in pool 1 (`D3DPOOL_MANAGED`):
- private = 373.46 MiB;
- virtual = 730.64 MiB.

Therefore DEFAULT did not materially consume process private/virtual memory merely by retaining these texture objects, while MANAGED added about 342 MiB, approximately the expected four compressed full mip chains. DEFAULT resources still consume GPU/driver resources when actually made resident/used; this test only establishes the 32-bit process-memory behavior.

## Large texture acceptance
On this machine, standard D3D9 `CreateTexture` accepted full-mip-chain DXT5 textures in both DEFAULT and MANAGED pools at:
- 2048x2048;
- 4096x4096;
- 8192x8192;
- 16384x16384.

This proves the current driver/device path can create textures at least up to 16384x16384. It does not by itself prove every TDU asset format, material path, BNK path, or shader accepts 16K assets.

## Shared GPU texture test
On an ordinary `IDirect3DDevice9`, a 1024x1024 DEFAULT-pool DXT5 texture creation with a non-null shared-handle pointer failed with HRESULT `0x8876086C` and returned a null handle.

Therefore the current standard-D3D9 TDU device cannot simply use the D3D9Ex shared-resource path. Cross-process GPU texture hosting would require a renderer/proxy change such as moving the relevant device/resource path to D3D9Ex or another interop design.

## Observed TDU process memory
Memory depends on startup duration and current game state. Two observed working `-w` runs were:
- about 369 MiB private / 795 MiB virtual after ~15 seconds in one run;
- about 552 MiB private / 976 MiB virtual after a longer probe run.

Treat these as observations, not fixed baselines. Combined with the original EXE lacking Large Address Aware, they make process address-space pressure a concrete concern for large MANAGED texture packs.

## Large Address Aware validation
A standalone x86 D3D9 probe was used to isolate the PE Large Address Aware bit while holding 8192x8192 DXT5 full-mip-chain MANAGED textures.

The normal Rust-built probe had characteristics `0x0122` and all 40 requested 8192x8192 MANAGED textures succeeded. A temporary copy with the LAA bit cleared had characteristics `0x0102`: 18 textures succeeded and the 19th failed with `0x8007000E`.

A temporary copy of the actual TDU executable was patched from `0x010F` to `0x012F`. It launched with `-w`, remained responsive, showed the normal game window, and created the same 1280x720 D3D9 device successfully. The temporary copy was removed after the test.

## DEFAULT upload-path tests on the real TDU device
A 4096x4096 single-level DXT5 upload path was tested on TDU's own live D3D9 device.

Confirmed:
- SYSTEMMEM source creation and LockRect/fill succeeded;
- DEFAULT destination creation succeeded;
- directly locking a normal DEFAULT destination failed with `0x8876086C`;
- `UpdateTexture` from SYSTEMMEM to DEFAULT succeeded.

A DYNAMIC + DEFAULT 4096x4096 DXT5 texture also succeeded and LockRect with DISCARD succeeded, but it increased process private memory by about 16.8 MiB.

Static DEFAULT destinations were retained after SYSTEMMEM upload. On the first device, additional retained 4096x4096 destinations increased private memory approximately linearly: two retained added 16,777,216 bytes, three added 33,562,624 bytes, and four added 50,483,200 bytes. Releasing the resources did not immediately return that cached private allocation. A second device largely reused the already-reserved driver memory.

Therefore merely creating unused DEFAULT textures is cheap, but populated DEFAULT textures can still cause the AMD D3D9 path to reserve private process memory roughly proportional to texture data. An external 64-bit asset process can remove parsing, decoding, and cache pressure from TDU, but cannot guarantee zero 32-bit address-space cost for final rendered textures.

## Production texture observation status
The proxy now has optional SetTexture/bound-texture instrumentation. Unattended/title-screen tests, including one activated-window run with a single Enter key, still showed only the previously observed 256x256 MANAGED CreateTexture call and no bound-texture events. World and vehicle texture allocation policy therefore remains unconfirmed until a probe is run during actual gameplay/world loading.

## Design implication
For high-resolution texture work:
1. LAA is now validated for startup and materially raises the 32-bit texture allocation ceiling.
2. Characterize real world and vehicle textures during gameplay before changing their allocation lifecycle.
3. Keep decoding, BNK work, transcoding, metadata, and large caches in a 64-bit asset process.
4. Use staged uploads and streaming selectively while measuring driver-side private-memory caching.
5. Revisit deeper renderer changes only if LAA plus streaming and eviction are insufficient.

The instrumentation proxy was removed from `game/` after testing.