# KoiArt: snow-related feature inventory

Goal: list every snow-related behaviour KoiArt (koiart.app, Spacerr LLC) advertises, so Snowfall can match the **behaviour** on Windows. KoiArt is closed source and paid, so this list comes only from its public site. Its name, artwork, wallpapers and branding are off-limits (see `AGENTS.md`).

Collected 2026-11-14. KoiArt's Windows build is 1.8.0 (`/download/windows` redirects to `dl.koiscribe.com/koiart/windows/1.8.0/KoiArt-Setup.exe`). The site and blog describe only the Mac app. The landing page says "The page doesn't describe Windows-specific feature differences". On Windows, read **Dock** as **taskbar**.

## Sources

- [L] https://koiart.app/ (landing, FAQ)
- [S] https://koiart.app/blog/make-it-snow-on-mac
- [B] https://koiart.app/blog/heavy-snow-blizzard-mac
- [D] https://koiart.app/blog/snow-on-mac-dock
- [W] https://koiart.app/blog/snow-behind-windows-mac
- [M] https://koiart.app/blog/snow-on-macbook
- [P] https://koiart.app/blog/mac-desktop-effects-performance
- [SM] https://koiart.app/blog/snowman-on-mac
- [X] https://koiart.app/blog/christmas-mac-desktop

## Core snow

| # | Behaviour | Source |
|---|-----------|--------|
| C1 | Snow falls over the whole screen and starts as soon as you pick "Snow". | S |
| C2 | Intensity levels: **"Snow"** and **"Heavy snow"** (a blizzard). | B, D |
| C3 | Snow settles on **window tops**, the **Dock** (→ taskbar) and **desktop widgets**, and builds up over time. | S, L |
| C4 | Dragging a window slides its snow off. **Flinging** a window fast sends the snow flying. | L, S |
| C5 | **Wind**: "No wind", a gentle wind, and **"Strong wind to the left"** / **"Strong wind to the right"**. | B, S |
| C6 | Clicks always pass through the snow. | L, P |
| C7 | The app reads window **positions** only, never window contents. | S, D |
| C8 | Snow also gathers along the bottom of the screen (the plough clears "the Dock first, then the bottom of the screen"). | D |

## Options

| # | Option | Behaviour | Source |
|---|--------|-----------|--------|
| O1 | **Behind windows** | Weather stays on the wallpaper, under every app. When off, snow piles on windows. | W |
| O2 | **Keep windows clear** | Snow still falls, but it melts on windows and the Dock instead of piling up, leaving a thin layer at the bottom of the screen. | W, D |
| O3 | **Per screen** | You choose which monitors get weather. | W, S |
| O4 | **Pause on battery** | Stops when unplugged and resumes when plugged in. | M |
| O5 | **Open at login** | Starts with the OS. | P |
| O6 | Turn weather off/on from the menu-bar icon (→ tray icon). | S, P |
| O7 | **"Snowballs"** toggle | When off, only plain snow piles up. | S, SM |
| O8 | **"Plough the Dock"** | The plough clears the Dock first. | D |
| O9 | Main-screen choice (the plough works on the main screen). | D |

## Power and performance

| # | Behaviour | Source |
|---|-----------|--------|
| P1 | Rendered on the GPU. | L, P |
| P2 | Runs at **half strength** on battery and in Low Power Mode (→ Windows Battery Saver / Energy Saver). | L, M |
| P3 | Pauses while a **full-screen app** covers the screen. | L, P |
| P4 | Pauses while the **screen sleeps**. | L, P |
| P5 | No CPU, memory or FPS figures are published. | P |

## Snow play (interactive extras)

| # | Behaviour | Source |
|---|-----------|--------|
| E1 | **Snowplough**: drives in when snow gets deep and clears it. **"Send snowplough"** in the menu summons it. It also removes snowballs. Works on the main screen. | D, B, SM |
| E2 | **Snowballs**: sweep the pointer through settled snow to roll a snowball, which grows as it rolls. | S, SM |
| E3 | **Throw**: release a ball mid-swing to throw it. A hard hit breaks both balls, or a snowman. | S, SM |
| E4 | **Snowman**: stack three balls. Snowmen ride along with their window, hop when it is shaken, can be dragged, fight when two are left alone, and **persist across restarts**. | SM, X |
| E5 | **Yeti**: appears now and then and goes after a snowman. You can drag the snowman away. Can be summoned from the menu. With no snowman around, it chops the newest Christmas tree. | SM, X |

## Christmas content (snow-adjacent, probably out of scope)

| # | Behaviour | Source |
|---|-----------|--------|
| X1 | **Santa** with reindeer along the Dock every few hours, one gift a day, **"Send Santa"**. Gifts show a quote. | X |
| X2 | Up to 3 **Christmas trees**: persistent, collect snow, customizable (height, top, decorations, "Surprise me"). | X |

## Out of scope (not snow)

Leaves, Starfall, Rain, Sakura, Fireflies, the wallpapers and Day & night, and licence keys and payment.

## Not stated anywhere (we need to decide)

- Whether snow appears over full-screen apps if the user wants it to (KoiArt only says it pauses).
- Whether screenshots and screen sharing include the snow. Snowhush hides it by default.
- Melting when "Keep windows clear" is off: does snow grow forever? (Snowhush has "Melt Snow".)
- What closing or minimizing a window does to its snow. (Snowhush drops it at once.)
