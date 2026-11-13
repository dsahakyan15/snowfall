# Start from Snowhush v0.6.1, not from scratch

Snowfall begins as a fork of Snowhush at tag `v0.6.1`, the last MIT-licensed release (later releases are PolyForm Noncommercial). v0.6.1 already has, in Rust for Windows, the click-through overlay, D3D11/DirectComposition rendering, window tracking, snow physics with piles on window tops, shaking and cursor interaction, melting, power handling and a tray menu. It builds and its 32 core tests pass. Writing all of that from scratch would cost months and repeat bugs upstream has already fixed. We strip everything that isn't snow, rebrand, and add what's missing to match KoiArt's behaviour ourselves.

**The yardstick is KoiArt, not Snowhush.** Where the two differ, KoiArt's behaviour wins (`docs/research/koiart-snow-parity.md`).

## Consequences

- The MIT notice from Snowhush v0.6.1 has to be kept in our distribution (`THIRD_PARTY_NOTICES.md`).
- Snowhush code newer than v0.6.1 is reference-only. Features that appeared upstream later (intensity sliders, carrying snow across displays, etc.) are written fresh from our own spec and Microsoft's docs.
