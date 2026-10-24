# RaceDirector

Race camera and broadcast overlay for **Umamusume: Pretty Derby — Steam Global, Windows x64 / D3D11**.

[Download the latest release](https://github.com/daftuyda/RaceDirector/releases/latest) · [Installation instructions](docs/INSTALLATION.md)

This repository is private. Sign into an account with repository access to download releases.

## Features

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

Settings save automatically approximately once per second when changed under `%LOCALAPPDATA%\RaceDirector\settings.json`. **Save settings** writes immediately. F10 also remembers the disabled state. Cursor capture remains manual.

## Compatibility

The owner confirmed Hachimi coexistence on their installation. The installer never overwrites another mod's version.dll. Avoid simultaneous camera control by multiple mods. Other combinations and future game versions may require changes.

Version 0.1.17 passed 41 automated tests. Native graphics allocation was checked in the running engine; full race-image acceptance across tracks and graphics settings remains incomplete. A 60 FPS request does not guarantee that frame rate under GPU load.

## Build

Requires Rust stable (Windows MSVC) and Visual Studio C++ Build Tools. Run:

```powershell
.\Build-RaceDirector.ps1
```

Output is in `dist`. Dependencies must be available locally or downloadable.

## Automated builds and releases

GitHub Actions tests and builds Windows x64 packages on pushes to main, pull requests to main, and manual runs from the Actions tab. Download the ZIP and SHA-256 checksum from the run's RaceDirector-windows-x64 artifact (retained for 14 days).

To publish a release:

1. Update the version in `native/Cargo.toml` and run `cargo check --workspace` to update Cargo.lock.
2. Commit and push the version change and any release changes to main.
3. Create and push a matching tag, for example:

```powershell
git tag v0.1.18
git push origin v0.1.18
```

The tag must exactly match the native crate version. After tests and the build pass, Actions publishes a GitHub release with the install ZIP, checksum, and generated release notes. Versions with a suffix such as v0.1.18-beta.1 become prereleases. Rerunning a tag workflow replaces its release assets. Manual runs on a branch only build; they do not publish a release.

The workflow uses GitHub's automatic token; no personal access token or extra secret is required. GitHub Actions must be enabled for the repository.

## License

Licensed under the [MIT License](LICENSE).
