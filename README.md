# RaceDirector

Race camera and broadcast overlay for **Umamusume: Pretty Derby — Steam Global, Windows x64 / D3D11**.

[Download the latest release](https://github.com/daftuyda/RaceDirector/releases/latest) · [Installation instructions](docs/INSTALLATION.md)

## Features

- Airframe inspector with saved light/dark appearance, top navigation and collapsible camera controls.
- Follow camera and pack-relative freecam, smoothing, target transitions and a ground-height guard.
- Selective HUD, photo button, skill popup/sidebar and character-marker hiding.
- Unique-cutin, speed-line, overhead-view and tree suppression options.
- Timing tower with placements, speed and distance/estimated time gaps.
- 60 FPS request, 2x/4x MSAA and up to 200% supersampling.
- Saved shots and automatic persistence of settings and enabled features.
- F8 clean capture hides the cursor and settings panel; the timing tower remains visible.

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
| F10 | Restore game camera and visuals |
| Right-drag / wheel | Orbit / zoom |
| WASD + Q/E | Freecam movement |
| Shift / Ctrl | Freecam fast / slow movement |
| [ / ] | Switch runner |

Settings save automatically approximately once per second when changed under `%LOCALAPPDATA%\RaceDirector\settings.json`. **Help > Save settings now** writes immediately. F10 also remembers the disabled state. Cursor capture remains manual.

## Compatibility

The owner confirmed Hachimi coexistence on their installation. The installer never overwrites another mod's version.dll. Avoid simultaneous camera control by multiple mods. Other combinations and future game versions may require changes.

Version 0.1.18 passed 44 automated tests. Native graphics allocation was checked in the running engine; full race-image acceptance across tracks and graphics settings remains incomplete. A 60 FPS request does not guarantee that frame rate under GPU load.

## Build

Requires Rust stable (Windows MSVC) and Visual Studio C++ Build Tools. Run:

```powershell
.\Build-RaceDirector.ps1
```

Output is in `dist`. Dependencies must be available locally or downloadable.

## Automated builds and releases

GitHub Actions tests and builds Windows x64 packages on pushes to main, pull requests to main, and manual runs from the Actions tab. Download the ZIP and SHA-256 checksum from the run's RaceDirector-windows-x64 artifact (retained for 14 days).

After a successful push to main, Actions reads the version from `native/Cargo.toml`, automatically creates its tag (for example, v0.1.18) at the built commit, and publishes the install ZIP, checksum, and generated release notes. No manual tag command is needed. A manual workflow run on main also publishes.

For the next release, update the version in `native/Cargo.toml`, run `cargo check --workspace` to update Cargo.lock, then commit and push both files with your changes. Version numbers are not automatically incremented.

If that version is already tagged at another commit, publishing is skipped and the existing tag and release assets are preserved. Bump the version to publish the new changes. Rerunning the same tagged commit can replace its release assets or retry an interrupted release.

Manual version-tag pushes are also supported and must match the native crate version. Versions with a suffix such as v0.1.18-beta.1 become prereleases. Pull requests and manual runs on other branches only build; they never publish.

The workflow uses GitHub's automatic token; no personal access token or extra secret is required. GitHub Actions must be enabled for the repository.

## License

Licensed under the [MIT License](LICENSE).

## Airframe appearance and trainer labels (0.1.18)

Choose **Light** or **Dark** at the top of the panel. The preference saves automatically. Airframe uses top navigation and collapsible Mode, Framing and Motion controls; Capture (F8) and Restore (F10) stay in the footer. An immediate **Save settings now** action is under Help.

Under **Tower > Trainers**, enable **Show trainer names** and enter names by gate. Names appear on a smaller second line; blank names keep compact rows. Names are manual in this release; automatic trainer lookup has not been verified. Labels persist by gate, so review them for each new race or use Clear trainer names. The tower scales to fit a full field.

Validation: 44 automated tests pass; actual ImGui previews were rendered and inspected in both themes with 18 named runners. In-game appearance after installation still needs checking.
