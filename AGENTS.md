# Snowfall

Open-source snow overlay for the Windows desktop, written in Rust. Snow falls over the screen, settles on window tops and the taskbar, reacts to windows and the cursor, and stays out of the way (click-through, hidden from screenshots, low CPU).

Target: Windows 10 2004+ / Windows 11, x64.

## Licensing rule (hard constraint)

Reference project Snowhush (github.com/mbezruchko/Snowhush) is MIT only up to and including tag `v0.6.1`. Everything after is PolyForm Noncommercial.

- Code may be copied/adapted **only** from Snowhush `<= v0.6.1`, keeping its MIT copyright notice (see `THIRD_PARTY_NOTICES.md` once it exists).
- Snowhush `> v0.6.1` may be read for ideas and techniques only. Never paste code from it.
- Never use KoiArt's name, artwork, wallpapers, or branding. Matching its snow behaviour is fine; the visuals are our own.

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues, driven by the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Research

Background research notes live in `docs/research/`.
