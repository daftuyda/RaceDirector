# RaceDirector

Race camera and broadcast overlay for **Umamusume: Pretty Derby — Steam Global, Windows x64 / D3D11**.

[Download the latest release](https://github.com/daftuyda/RaceDirector/releases/latest) · [Installation instructions](docs/INSTALLATION.md) · [Broadcast guide](docs/BROADCAST.md)

## Features

- Airframe inspector with saved light/dark appearance, top navigation and collapsible camera controls.
- Follow camera and pack-relative freecam, smoothing, target transitions and a ground-height guard.
- Two-runner battle camera with stable automatic pairing or a manually selected rival.
- Automatic runner tour with battle framing, moving shots and a fixed finish camera that pans and zooms.
- Focus/rival labels above runners, with an optional automatic finish-camera position.
- Selective HUD, photo button, skill popup/sidebar and character-marker hiding.
- Unique-cutin, speed-line, overhead-view and tree suppression options.
- Animated timing tower with placements, speed, distance/estimated time gaps and brief places-gained/lost indicators.
- Runner spotlight, battle card, race progress and confirmed finish/results graphics.
- 60 FPS request, 2x/4x MSAA and up to 200% supersampling.
- Saved shots and automatic persistence of settings and enabled features.
- F8 clean capture hides the cursor and settings panel; enabled coverage graphics remain visible.

## Install

Download and extract the release ZIP, close the game, then open PowerShell in the extracted folder:

```powershell
.\Install-RaceDirector.ps1 -GameDirectory 'C:\Program Files (x86)\Steam\steamapps\common\UmamusumePrettyDerby'
```

Launch the game, enter a race and press **Insert**. Use **Update-RaceDirector.ps1** for an existing installation. See the [full guide](docs/INSTALLATION.md) for custom folders, Hachimi, updates and removal.

## Controls

| Key | Action |
| --- | --- |
| Insert | Open/close settings |
| F6 | Toggle camera |
| F7 | Toggle selected UI hiding |
| F8 / Escape | Toggle / release capture lock |
| F9 | Stop automatic directing, or toggle battle camera while following |
| F10 | Restore game camera and visuals |
| Right-drag / wheel | Orbit / zoom |
| WASD + Q/E | Freecam movement |
| Shift / Ctrl | Freecam fast / slow movement |
| [ / ] | Switch runner |

Settings save automatically approximately once per second when changed under `%LOCALAPPDATA%\RaceDirector\settings.json`. **Help > Save settings now** writes immediately. F10 also remembers the disabled state. Cursor capture remains manual.

## Broadcast coverage (0.1.19)

Open **On air** and choose **Show coverage graphics** to enable the tower, runner spotlight, battle card, progress strip and finish/results graphics. Each graphic also has its own saved toggle. New graphics start disabled; tower animation and position-change indicators start enabled for an existing tower.

For a two-runner shot, enable the camera with **F6**, choose **Follow**, select a runner and toggle **Battle camera [F9]**. Auto pairing chooses a nearby rival immediately ahead or behind, holds the pair through overtakes and avoids frequent switches. You can instead pin a partner by gate. The defaults are a four-second minimum hold and a 30-metre maximum gap.

Enter the event, course and optional race-length override under **On air > Event details**. Labels and overrides persist between races; review them before each race. Leave race distance at **0** for Auto: use game-supplied course length when available, otherwise show distance covered. Winner and result graphics use confirmed finish information and do not display precomputed replay results before runners finish.

**F8** keeps coverage graphics on air while hiding settings and the cursor. **F10** disables camera control and coverage graphics, retaining event details and appearance preferences. See the [broadcast guide](docs/BROADCAST.md) for setup and display behaviour.

## Compatibility

The owner confirmed Hachimi coexistence on their installation. The installer never overwrites another mod's version.dll. Avoid simultaneous camera control by multiple mods. Other combinations and future game versions may require changes.

Version 0.1.23 passed 102 automated tests. The new broadcast features still require in-game validation. Native graphics allocation was checked in the running engine previously; full race-image acceptance across tracks and graphics settings remains incomplete. A 60 FPS request does not guarantee that frame rate under GPU load.

## Build

Requires Rust stable (Windows MSVC) and Visual Studio C++ Build Tools. Run:

```powershell
.\Build-RaceDirector.ps1
```

Output is in `dist`. Dependencies must be available locally or downloadable.

## Automated builds and releases

GitHub Actions tests and builds Windows x64 packages on pushes to main, pull requests to main, and manual runs from the Actions tab. Download the ZIP and SHA-256 checksum from the run's RaceDirector-windows-x64 artifact (retained for 14 days).

After a successful push to main, Actions reads the version from `native/Cargo.toml`, automatically creates its tag (for example, v0.1.19) at the built commit, and publishes the install ZIP, checksum, and generated release notes. No manual tag command is needed. A manual workflow run on main also publishes.

For the next release, update the version in `native/Cargo.toml`, run `cargo check --workspace` to update Cargo.lock, then commit and push both files with your changes. Version numbers are not automatically incremented.

If that version is already tagged at another commit, publishing is skipped and the existing tag and release assets are preserved. Bump the version to publish the new changes. Rerunning the same tagged commit can replace its release assets or retry an interrupted release.

Manual version-tag pushes are also supported and must match the native crate version. Versions with a suffix such as v0.1.19-beta.1 become prereleases. Pull requests and manual runs on other branches only build; they never publish.

The workflow uses GitHub's automatic token; no personal access token or extra secret is required. GitHub Actions must be enabled for the repository.

## License

Licensed under the [MIT License](LICENSE).

## Airframe appearance and trainer labels (0.1.18)

Choose **Light** or **Dark** at the top of the panel. The preference saves automatically. Airframe uses top navigation and collapsible Mode, Framing and Motion controls; Capture (F8) and Restore (F10) stay in the footer. An immediate **Save settings now** action is under Help.

Under **Tower > Trainers**, enable **Show trainer names** and enter names by gate. Names appear on a smaller second line; blank names keep compact rows. Names were manual in v0.1.18; v0.1.20 adds automatic lookup as described below. Labels persist by gate, so review them for each new race or use Clear trainer names. The tower scales to fit a full field.

For v0.1.18, 44 automated tests passed and actual ImGui previews were inspected in both themes with 18 named runners.

## Automatic trainer names (0.1.20)

Enable **Tower > Trainers > Show trainer names** and **Automatic trainer names**. Names are read from the current runner's game data and used by the tower, spotlight, battle and winner cards. Nonblank manual names override them; **Clear manual overrides** restores automatic labels.

Automatic names are never saved by gate. Missing names stay blank, and some race modes may not supply them. The optional getter was identified in game metadata; live population still needs testing. The editor displays each received game name and telemetry status reports how many names the game supplied.

## Automatic director and focus markers (0.1.23)

Enable **Camera > Automatic director > Automatic directing**. This also enables camera control, switches to Follow and enables finish coverage. **Include battles** defaults on and frames a nearby rival during both the tour and regular Follow, independently of the manual Battle toggle. Your selected runner, framing and manual pairing preferences are preserved. **Continuous movement** adds a slow orbit, dolly and lens movement as soon as directing starts, including while paused. It continues before, during and after the tour. Adjust its amount and cycle, or disable **Gentle camera movement** for a still shot.

**Race length override (0 = Auto)** uses game-supplied course length when available. The current value and source are shown in the panel. If the game supplies no length, the tour still starts at 200 m and the panel asks for an override for finish coverage. A known length starts the tour at 25% by default. **Runner tour** contains the start, time-per-runner and **Start / restart runner tour now** controls. It starts at your selected runner, then chooses nearby unvisited runners while retaining the same moving camera rig. Holds default to 6 seconds with a 5-second minimum; old shorter settings are raised to that minimum. Fast-forward does not accelerate the holds, and pausing stops runner changes while gentle movement continues. Automatic target transitions take at least 1.8 seconds. Completion returns to your selected runner and framing, with gentle movement and automatic pairing if enabled.

Under **Automatic director > Finish coverage**, the default trigger is 400 m remaining. Coverage moves to P1 and nearby P2, holding that pair at their first arrival. With **Fixed finish camera** and **Estimate finish position automatically** enabled, the final 120 m transitions to a stationary spot estimated from the leader's actual travel direction, remaining distance and world scale. The camera then pans and zooms to track the contest. **Finish camera side** defaults to **Left of travel**, the opposite side from v0.1.22, and controls both the moving finish angle and the estimated fixed position. **Right of travel** restores the former side. This estimates a straight finish; it does not locate an exact finish-line object or detect the final corner. Without sufficient data, moving finish coverage continues and the status explains why.

For precise placement, position Freecam beside and beyond the finish line and choose **Save current Freecam as finish camera**. Return to Follow. An armed saved position takes priority over the estimate and is used from the configured finish trigger. Saved positions persist but must be armed for each new race after checking the course.

**On air > Runner focus markers** adds a blue **Focus** nameplate above the selected runner and an amber **Rival** nameplate above its battle partner. Both identify battle coverage when paired. They use the final camera position, stay visible in F8 clean capture, ignore mouse input, and hide in Freecam or when the runner is off screen. Labels now sit lower, with a shorter connector. Close labels separate horizontally where possible, using vertical stacking only when space is limited. Adjust **Marker height above ground** under On air if needed. The option defaults on and is saved; F10 and **Hide coverage graphics** disable it.

F9 stops automatic directing and battle mode. Freecam suspends automation and cancels an active tour; **Resume automatic Follow** returns control to the director. F10 restores the game camera and disables coverage. Camera off, race exit and replay rewind reset the sequence; a new race or rewind disarms saved finish positions. Course metadata, estimated positions and projected marker locations are never saved across races.

The running game confirmed the course-length/scale getters, and marker projection was checked against Unity's viewport projection. The v0.1.23 motion, tour pacing and opposite-side finish view still require visual acceptance after updating and restarting the game.
