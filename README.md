# Snowfall

Snow that falls over your Windows desktop, piles up on your windows and the taskbar, slides off when you drag them, and flies when you fling or shake them. Clicks go straight through, it hides from screenshots, and it costs next to nothing.

> Work in progress: milestone M1 is tracked in [issue #1](https://github.com/dsahakyan15/snowfall/issues/1).

**Requires** Windows 10 version 2004 or later, or Windows 11.

## Building

Install [Rust](https://rustup.rs) (MSVC toolchain) and the Visual Studio C++ Build Tools, then:

```sh
cargo build --release
cargo test
```

## Licence

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Snowfall started as a fork of [Snowhush](https://github.com/mbezruchko/Snowhush) v0.6.1 (MIT); see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
