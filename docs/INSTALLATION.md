# Installation guide

## Requirements

Windows x64 and Steam Global Umamusume. Close the game before installing, updating or disabling RaceDirector.

## Install

1. Download **RaceDirector-v0.1.23-windows-x64.zip** from Releases and extract it to a separate folder. GitHub's source-code ZIP does not contain the built plugin.
2. In Steam, choose the game's **Manage > Browse local files**. Use the folder containing **UmamusumePrettyDerby.exe**.
3. Open PowerShell in the extracted release folder and run, replacing the path:

```powershell
.\Install-RaceDirector.ps1 -GameDirectory 'D:\SteamLibrary\steamapps\common\UmamusumePrettyDerby'
```

If scripts are blocked, review the downloaded scripts and unblock them using file Properties or Unblock-File. If execution policy still blocks the reviewed installer, run it in a separate process:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\Install-RaceDirector.ps1 -GameDirectory 'D:\SteamLibrary\steamapps\common\UmamusumePrettyDerby'
```

This does not permanently change execution policy. If copying into Program Files fails with access denied, reopen PowerShell as administrator and retry.

The installer adds these files beside the executable:

| File | Purpose |
| --- | --- |
| version.dll | RaceDirector loader |
| racedirector.dll | Camera/overlay plugin |
| racedirector_version.dll | Genuine Windows version library copied from this PC |

All three are required. The package does not redistribute Windows DLLs. The installer refuses to overwrite existing files. Use Update for an existing RaceDirector installation.

Launch the game, enter a race and press **Insert**. Enable your preferred toggles once; they then save automatically. Start model clarity at **125% + 2x MSAA** and reduce it if FPS suffers.

## Hachimi

The owner confirmed RaceDirector works with their existing Hachimi installation. No Hachimi configuration changes are required for that setup. If Hachimi uses another entry point and the root version.dll is free, install normally.

If version.dll already belongs to Hachimi or another mod, the installer stops. **Do not overwrite, rename or delete that DLL to force installation.** This release does not automatically migrate or chain loaders. Keep a setup that already loads both mods unchanged. Compatibility is not verified for every Hachimi build or plugin combination.

Avoid enabling both mods' free-camera controls simultaneously. Overlapping frame-rate/rendering overrides can also conflict.

## Update

Close the game, extract the new release separately, then run:

```powershell
.\Update-RaceDirector.ps1 -GameDirectory 'D:\SteamLibrary\steamapps\common\UmamusumePrettyDerby'
```

The updater backs up and replaces only racedirector.dll, verifies its hash, and preserves loaders and settings. Without GameDirectory, it uses the default Steam folder under Program Files (x86).

## Disable and uninstall

```powershell
.\Disable-RaceDirector.ps1 -GameDirectory 'D:\SteamLibrary\steamapps\common\UmamusumePrettyDerby'
```

This renames the plugin while leaving its loader and settings in place. The updater can re-enable it.

To uninstall, close the game and remove only the three RaceDirector-installed files listed above. Do not remove version.dll if another mod has since replaced it. Preserve Hachimi files. Optionally remove `%LOCALAPPDATA%\RaceDirector\settings.json`; old versions may also have racedirector-settings.json beside the executable.

## Troubleshooting

- **Existing DLL:** use Update for RaceDirector; preserve other mods' loaders.
- **No panel:** confirm the three files are beside the correct executable, wait for initialisation and press Insert. Check `%LOCALAPPDATA%\RaceDirector\startup.log`.
- **Locked mouse:** F8 or Escape releases it; F10 restores camera and visuals.
- **Settings not saved:** check Help/status for an auto-save error and try Save settings. Older versions did not save toggle states, so set them once after updating.
- **Bad camera/rendering:** use F10 and disable overlapping mod camera controls. Re-enable features individually.
- **Low FPS:** reduce supersampling/MSAA and disable overhead views.
- **Battle camera stays in normal follow:** enable Camera and Follow mode, select an eligible rival, and check the maximum gap. Finished runners are not paired. Camera aspect and race telemetry must be available.
- **No spotlight:** enable Runner spotlight under On air and follow a runner with the camera enabled. Freecam has no followed-runner card.
- **Wrong distance remaining:** review the length override under On air > Event details; use 0 for game-supplied length when available. The director panel reports the value and source.
- **No winner card yet:** results are shown only after the game confirms runners have finished. Enable Finish and results under On air.

Report the plugin/game versions, other mods, reproduction steps and relevant log lines. Remove personal information from logs before sharing.

## Airframe (v0.1.18)

Use the Light / Dark buttons at the top of the panel. Both theme and feature choices save automatically. Help contains Save settings now. Tower > Trainers provides optional manual names by gate; clear or review names when switching races. No automatic trainer lookup is claimed.

## Broadcast coverage (v0.1.19)

After updating, open **On air** and choose **Show coverage graphics**, or enable individual graphics. For battle framing, enable the camera with **F6**, choose **Follow**, select a runner, then press **F9** or use Battle camera under Camera > Mode. Auto selects a nearby rival; the partner menu can pin a specific gate.

**Tower > Appearance** controls row animation and position-change indicators. **On air > Event details** holds event/course names and the optional race-length override. Review these and trainer names for each new race. F8 keeps enabled graphics visible during clean capture; F10 disables them and remembers that choice.

See [Broadcast guide](BROADCAST.md) for a complete setup walkthrough and data limitations.

## Automatic trainers (v0.1.20)

Enable Show trainer names and Automatic trainer names under Tower > Trainers. Clear any old manual overrides to use game names. Missing game names remain blank; race-mode availability needs live verification. Lookup is optional and does not prevent the plugin loading if the getter is unavailable.


## Automatic directing and runner labels (v0.1.23)

Enable **Camera > Automatic director > Automatic directing**. This enables Follow camera, finish coverage and (by default) automatic battle pairing. Leave **Race length override** at **0** for Auto; the panel shows the detected value or explains what is missing. The tour still runs at 200 m if length is unavailable, while finish coverage requires a known length. Use **Runner tour > Start / restart runner tour now** to check a tour without waiting for its configured start.

Under **Finish coverage**, automatic placement uses actual travel direction to estimate a fixed spot in the last 120 m. An armed saved Freecam position takes priority. These features require the updated DLL and a game restart; replacing files does not update a loaded plugin.

**On air > Runner focus markers** toggles the custom labels above the selected runner and battle rival. This works independently of the native arrow-hiding setting, and remains visible during clean capture. F10 turns it off with the other graphics. Phase changes are recorded in `%LOCALAPPDATA%\RaceDirector\startup.log` with the applied camera coordinates.


**Continuous movement > Gentle camera movement** now starts immediately and continues between runner changes, including while paused. Disable it for a still paused shot. Tour holds are at least 5 viewing seconds (new default 6), even in fast-forward, and automatic target transitions last at least 1.8 seconds.

**Finish camera side > Left of travel** is the new default, moving the estimated finish view to the opposite side from v0.1.22. Right of travel restores the prior side; armed saved positions retain their chosen location. Focus labels sit closer to the runners, with a saved **Marker height above ground** adjustment under On air.
