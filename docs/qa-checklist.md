# Manual QA checklist

Run before each release, on the release build (`cargo build --release`, or the installer from the release workflow). The engine has headless tests; this list covers what only a real Windows desktop can show. Tick each item, and note the Windows version, displays and scaling used.

Start from a clean profile where an item says so: quit Snowfall, delete `HKCU\Software\Snowfall`, and remove the `Snowfall` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

## Performance

- [ ] `powershell -ExecutionPolicy Bypass -File scripts\measure.ps1` passes: Snow on a 1080p 60 Hz display stays within 5% of one core and 80 MB while running. Record the numbers in the release notes.
- [ ] With Snowfall off and the snow settled, Task Manager shows Snowfall at 0% CPU most of the time.

## Staying out of the way

- [ ] Away from a Snowball, clicks, drags, scrolling and right-clicks all go through the snow to the windows and desktop under it.
- [ ] Snowfall never takes keyboard focus: type in an editor while toggling settings from the tray.
- [ ] Snowfall doesn't appear in Alt+Tab, Win+Tab or on the taskbar; only the tray icon shows.
- [ ] Screenshots (Win+Shift+S, PrtScn), the Snipping Tool, a screen recording and a Teams/Zoom/browser screen share don't show the snow by default.
- [ ] With **Show in screenshots** on, they do.
- [ ] The Start menu, notification centre, quick settings, context menus, tooltips and the tray's own menu all appear above the snow.

## Taskbar and Ground

- [ ] Snow collects on top of the taskbar, at the bottom, top or side, wherever the taskbar is.
- [ ] With the taskbar set to auto-hide, snow collects at the screen edge instead, and the taskbar slides out over it.
- [ ] Moving or resizing the taskbar moves the Ground with it, without restarting Snowfall.

## Full screen

- [ ] Play a video full screen in a browser and in a video player: snow on that screen fades out in about a quarter second, and other screens keep snowing. Leave full screen: it fades back in where it was.
- [ ] The same with a full-screen game, and with a slideshow (PowerPoint, or a browser presentation).
- [ ] Turn on **Show over full screen** and play a video full screen: the snow stays over the video and settles at the very bottom of the screen. Leave full screen: the Ground is back on top of the taskbar.
- [ ] A maximized window that isn't full screen doesn't hide the snow.

## Behind windows

- [ ] With **Behind windows** on, the snow is on the wallpaper, under every window and above the desktop icons' background, and still settles on window tops.
- [ ] **Show over full screen** is greyed out, and **Show in screenshots** reads "always, behind windows".
- [ ] With a live wallpaper app (such as Lively) running, the snow stays above the wallpaper.
- [ ] Turn it off: the snow comes back on top of everything.

## Windows and the pointer

- [ ] Snow piles up on window tops where they are visible, and not under other windows, on tiny windows or on transparent widgets.
- [ ] Dragging a window slides its snow off; flinging or shaking it throws it into the air; closing or minimizing it drops its snow.
- [ ] Switching virtual desktops and back keeps each window's snow.
- [ ] Snapping or maximizing a window with Win+arrows drops its snow rather than carrying it.
- [ ] **Snowballs**: sweep through deep snow to roll a round, shaded ball with turning dimples and a contact shadow; shallow snow makes none. Let go: it rests, follows its window and allows idle CPU to return to 0%. Check Ledge ends, window shedding/closing, Clear snow, Opacity, fade-out and the saved EN/RU toggle.
- [ ] **Grabbing Snowballs**: the hand cursor and left press catch only the ball's visible circle, including when another window covers part of it. Click beside it and through its bounding-box corners: the app beneath receives the click. Drag and release over a Ledge and empty space: the ball falls onto that Ledge or the Ground. Move, minimize, close, fling and maximize its supporting window: the ball rides or falls intact.
- [ ] Keep an editor focused while grabbing and dropping a ball: Snowfall never takes keyboard focus. Move quickly outside the ball before releasing, and interrupt a drag with Alt+Tab: the ball never stays stuck to the pointer.
- [ ] Grabbed, falling and resting balls and their hit windows are absent from screenshots and recordings by default; **Show in screenshots** includes them. Full screen, sleep, lock, **Pause on battery**, **Screens** off and **Snowballs** off leave no invisible click targets. **Behind windows** disables grabbing. Quit leaves no hit windows behind.
- [ ] Sweeping the pointer through a Pile parts it; a fast sweep sends snow flying; falling flakes swirl in the pointer's wake.

## Settings

- [ ] Every tray menu item works and is remembered after Quit and a restart: Snowfall (Off, Snow, Heavy snow), Wind (5), Screens, Opacity (5), Clear snow, Behind windows, Keep windows clear, Snowballs, Melt, Pause on battery, Show over full screen, Show in screenshots, Open at login.
- [ ] Off lets the falling snow finish; nothing new comes in.
- [ ] Strong wind blows flakes and spray sideways and drifts window Piles toward the downwind end; light wind only slants the snowfall.
- [ ] **Keep windows clear**: existing window snow melts within seconds, windows never build up more than a trace, and the Ground keeps only a thin layer.
- [ ] **Opacity**: falling flakes and Snowballs fade at lower settings; Piles don't.
- [ ] **Clear snow** blows all settled snow away in a gust.
- [ ] With Windows' display language set to Russian, the menu is in Russian; otherwise English.

## Several displays and scaling

- [ ] The **Screens** submenu lists each display by name (or "Screen N"), marks the primary one, and updates when a display is plugged in or out.
- [ ] Unchecking a screen stops its snow at once; checking it brings snow back. The choice survives a restart and unplugging and replugging that display.
- [ ] Turning every screen off is allowed and behaves like Off.
- [ ] Dragging a snowy window to another display carries its Pile along, subject to the usual sliding.
- [ ] Grab a Snowball and drag it across monitors in both directions, including different DPI settings and vertically arranged displays: one ball follows the pointer with the grab offset, releases on the destination screen and falls onto its Ledge or Ground. Disable or unplug either screen during a drag: no stuck capture or invisible click target remains. Balls on a carried Pile currently return to its snow.
- [ ] A window resting across two displays shows one Pile along its whole top, with no seam or doubled snow at the boundary, and snow builds up on both parts.
- [ ] With displays at different scaling (such as 100% and 150%), flakes and Piles look the same physical size on both, and a carried Pile keeps its size.
- [ ] Changing resolution, scaling or arrangement while running needs no restart.

## Power and sessions

- [ ] Sleep, lock (Win+L) and remote disconnect stop the snow; it carries on afterwards.
- [ ] Turning the display off stops the snow.
- [ ] Laptop on battery: the snowfall halves. Plug in: it comes back to full.
- [ ] Battery saver (Windows 10) or energy saver (Windows 11), plugged in or not: the snowfall halves.
- [ ] **Pause on battery** on: unplugging hides the snow within about a second and stops it; plugging in brings it back where it was. With it on but plugged in, saver only halves the snow.

## Resilience

- [ ] Restart Explorer (Task Manager → Windows Explorer → Restart): the tray icon comes back, and with Behind windows on, so does the snow.
- [ ] Update the GPU driver, or trigger a device reset (Win+Ctrl+Shift+B): the snow comes back by itself.
- [ ] Starting Snowfall a second time doesn't start a second copy.
- [ ] Switching between light and dark taskbar recolours the tray snowflake.

## First run and updates

- [ ] On a clean profile, the first launch shows one notification ("Snowfall is running — settings are in the snowflake icon"), and registers start at login for the current exe. The second launch shows none.
- [ ] Open at login off removes the start-at-login entry; on adds it back. Signing out and in starts Snowfall only when it is on.
- [ ] **Check for updates** from the menu says "You have the latest Snowfall" when current, "Couldn't reach GitHub…" when offline, and offers a newer release when there is one; clicking that notification opens the release page.

## Installer

- [ ] On a standard (non-admin) account, Snowfall-Setup.exe installs without a UAC prompt, in English or Russian to match Windows, adds a Start-menu shortcut and starts Snowfall.
- [ ] Snowfall appears in Settings → Apps → Installed apps.
- [ ] Installing again over a running Snowfall closes it first and starts the new one.
- [ ] Uninstalling closes a running Snowfall and leaves no `%LOCALAPPDATA%\Programs\Snowfall`, no `%LOCALAPPDATA%\Snowfall`, no `HKCU\Software\Snowfall` and no start-at-login entry.
