# Snowfall

Snow that falls over your Windows desktop, piles up on your windows and the taskbar, slides off when you drag them, and flies when you fling or shake them. Clicks go straight through, it hides from screenshots, and it costs next to nothing.

> Work in progress: milestone M1 is tracked in [issue #1](https://github.com/dsahakyan15/snowfall/issues/1).

**Requires** Windows 10 version 2004 or later, or Windows 11, 64-bit.

## Download

Get the latest release from [GitHub Releases](https://github.com/dsahakyan15/snowfall/releases/latest):

- **Snowfall-Setup.exe** installs Snowfall for your user account, with no administrator rights, adds it to the Start menu and starts it. Uninstall it from Settings → Apps; that also removes its settings and its start-at-login entry.
- **Snowfall.exe** is the same app without an installer. Run it from anywhere.

Snowfall lives in the notification area: click the snowflake for its settings. Once a day it checks GitHub for a newer release and tells you, and it never downloads anything by itself.

## Building

Install [Rust](https://rustup.rs) (MSVC toolchain) and the Visual Studio C++ Build Tools, then:

```sh
cargo build --release
cargo test
```

The app is `target\release\Snowfall.exe`. To build the installer too, install [Inno Setup 6](https://jrsoftware.org/isinfo.php) and run:

```sh
iscc /DVersion=0.1.0 installer\Snowfall.iss
```

which writes `dist\Snowfall-Setup.exe`.

## Releasing

Set the version in `windows/Cargo.toml`, then push a tag for it, such as `v0.1.0`. The release workflow builds and tests Snowfall, builds the installer, and publishes both `Snowfall-Setup.exe` and `Snowfall.exe` as a GitHub Release. A tag with a pre-release part, such as `v0.2.0-beta.1`, makes a pre-release.

## Contributing

`cargo test` runs the snow engine's headless tests and the app's own. What only a real desktop can show is in the [manual QA checklist](docs/qa-checklist.md), run before each release, together with the performance check:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\measure.ps1
```

which runs the release build for a minute and fails if it takes more than 5% of one core or 80 MB while it runs. Words for the snow and its places are in [GLOSSARY.md](GLOSSARY.md), and decisions in [docs/adr](docs/adr).

## Licence

Licensed under `MIT OR Apache-2.0`: either the [MIT licence](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option.

Snowfall started as a fork of [Snowhush](https://github.com/mbezruchko/Snowhush) v0.6.1 (MIT); see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
