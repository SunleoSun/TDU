---
description: Known-good Test Drive Unlimited 1.66A startup path on this Windows 11 machine, including borderless launch and intro-video handling.
---

# game-startup

## Purpose
Keep the known-good Windows 11 startup path for this Test Drive Unlimited 1.66A copy.

## Confirmed environment
- Game executable: `game/TestDriveUnlimited.exe`
- Executable reports version: `MC 1.66 A`
- Host OS during diagnosis: Windows 11 Pro build 26200
- GPU during diagnosis: AMD Radeon RX 6800 XT

## Confirmed behavior
- Native fullscreen startup hangs on a black, non-responsive `Test Drive Unlimited` window.
- `TestDriveUnlimited.exe -w` starts successfully and produces a responsive game window.
- The old save directory was temporarily removed during diagnosis and native fullscreen still hung, so the old saves were not the cause of this startup failure.
- Win7/XP compatibility flags, disabling fullscreen optimizations, single-core affinity, and temporarily disabling the Realtek Digital Output endpoint did not fix native fullscreen.
- Legacy DirectX 9 runtime DLLs used by old games are present in `C:\Windows\SysWOW64`.

## Current startup workaround
- Launch with `Start TDU - Windows 11.cmd` from the TDU workspace root.
- The CMD starts `Start TDU - Windows 11.ps1`.
- The PowerShell launcher runs `game/TestDriveUnlimited.exe -w`, strips the standard window frame, and resizes the game to the monitor bounds while keeping the working windowed Direct3D path.
- Borderless validation on this machine confirmed a responsive `Test Drive Unlimited` process, no standard border, and a 1920x1080 game window exactly filling the 1920x1080 monitor.
- Keep helper scripts/programs at the TDU workspace root, not inside `game`.

## Intro videos
Startup intro videos are disabled by renaming:
- `game/Euro/Video/Atari.bik` -> `Atari.bik.disabled`
- `game/Euro/Video/Bumper_EDEN_TDU.bik` -> `Bumper_EDEN_TDU.bik.disabled`
- `game/Euro/Video/IntroTDU.bik` -> `IntroTDU.bik.disabled`

`Trailer.bik` is left unchanged because it is not part of the startup intro sequence.

The game was validated as responsive with these three startup videos disabled.

## Safety / rollback
To restore intro videos, rename the three `.disabled` files back to their original `.bik` names.
The launcher does not modify the executable.