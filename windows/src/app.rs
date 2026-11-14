use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{POWERBROADCAST_SETTING, RegisterPowerSettingNotification};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification};
use windows::Win32::System::SystemServices::{GUID_CONSOLE_DISPLAY_STATE, GUID_POWER_SAVING_STATUS};
use windows::Win32::System::Threading::{CreateEventW, SetEvent};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, Result, w};

use crate::log;
use crate::menu::{self, Action, Context, Item};
use crate::registry::{self, Registry};
use crate::screen;
use crate::settings::{Change, Settings, Store};
use crate::snow::{self, Command, Links};
use crate::strings::Language;
use crate::update as releases;

const CLASS: PCWSTR = w!("SnowfallController");
const WM_TRAY: u32 = WM_APP + 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const NIN_BALLOONUSERCLICK: u32 = WM_USER + 5;
/// A look for updates finished: `lparam` is a `Box<Lookup>`.
const WM_UPDATES: u32 = WM_APP + 2;
const TIMER_UPDATES: usize = 1;

static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

/// The tray glyph at the sizes the tray draws it at.
macro_rules! glyph {
    ($name:literal) => {
        [
            (16, include_bytes!(concat!("../assets/tray/", $name, "-16.png")).as_slice()),
            (20, include_bytes!(concat!("../assets/tray/", $name, "-20.png")).as_slice()),
            (24, include_bytes!(concat!("../assets/tray/", $name, "-24.png")).as_slice()),
            (32, include_bytes!(concat!("../assets/tray/", $name, "-32.png")).as_slice()),
            (40, include_bytes!(concat!("../assets/tray/", $name, "-40.png")).as_slice()),
            (48, include_bytes!(concat!("../assets/tray/", $name, "-48.png")).as_slice()),
            (64, include_bytes!(concat!("../assets/tray/", $name, "-64.png")).as_slice()),
        ]
    };
}

type Glyph = [(i32, &'static [u8]); 7];
const SNOWFLAKE_WHITE: Glyph = glyph!("snowflake-white");
const SNOWFLAKE_BLACK: Glyph = glyph!("snowflake-black");

/// Black on a light taskbar, white on a dark one, as the system's own tray icons.
fn glyph(light: bool) -> &'static Glyph {
    if light { &SNOWFLAKE_BLACK } else { &SNOWFLAKE_WHITE }
}

/// Whether the taskbar is light: the setting for "your default Windows mode".
fn light_taskbar() -> bool {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast::<c_void>()),
            Some(&mut size),
        )
    };
    status.is_ok() && value == 1
}

struct App {
    settings: Settings,
    commands: Sender<Command>,
    wake: HANDLE,
    snow: Option<JoinHandle<()>>,
    icon: HICON,
    light: bool,
    locked: bool,
    display_off: bool,
    away: bool,
    /// The page of a newer release the last notification told of, opened when it is clicked.
    release_page: Option<String>,
    /// A look for updates is under way.
    checking: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// The tray icon and its menu, and the system events that pause the snow, on the main thread;
/// the snow itself runs on its own thread so menus and other modal loops never stall it.
pub fn run() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
        let window = CreateWindowExW(WINDOW_EX_STYLE(0), CLASS, w!("Snowfall"), WS_OVERLAPPED, 0, 0, 0, 0, None, None, Some(instance.into()), None)?;
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);

        let mut settings = Settings::load(&Registry);
        // The first run registers autostart, as Open at login is on by default; later runs leave the entry alone,
        // so starting another copy, such as a development build, never takes it over.
        let first_run = !registry::has_settings();
        if first_run {
            settings.open_at_login = registry::set_open_at_login(settings.open_at_login);
            settings.save(&mut Registry);
        }
        let (commands, receiver) = mpsc::channel();
        let wake = CreateEventW(None, false, false, None)?;
        let links = Links { wake: wake.0 as isize, controller: window.0 as isize };
        let initial = settings.clone();
        let snow = thread::Builder::new().name("snow".into()).spawn(move || snow::run(receiver, links, initial)).ok();
        let light = light_taskbar();
        let icon = tray_icon(light);
        add_tray_icon(window, icon);
        if first_run {
            log!("first run");
            notify(window, Language::from_langid(GetUserDefaultUILanguage()).strings().first_run);
        }
        let _ = WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION);
        let _ = RegisterPowerSettingNotification(HANDLE(window.0), &GUID_CONSOLE_DISPLAY_STATE, DEVICE_NOTIFY_WINDOW_HANDLE);
        let _ = RegisterPowerSettingNotification(HANDLE(window.0), &GUID_POWER_SAVING_STATUS, DEVICE_NOTIFY_WINDOW_HANDLE);
        APP.set(Some(App {
            settings,
            commands,
            wake,
            snow,
            icon,
            light,
            locked: false,
            display_off: false,
            away: false,
            release_page: None,
            checking: false,
        }));
        // The first look for updates waits a little after start-up, then the hourly timer decides when one is due.
        SetTimer(Some(window), TIMER_UPDATES, 30_000, None);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_TRAY => {
            let event = (lparam.0 & 0xffff) as u32;
            if event == WM_CONTEXTMENU || event == NIN_SELECT || event == NIN_KEYSELECT {
                let x = (wparam.0 & 0xffff) as i16 as i32;
                let y = ((wparam.0 >> 16) & 0xffff) as i16 as i32;
                show_menu(window, x, y);
            } else if event == NIN_BALLOONUSERCLICK {
                open_release_page();
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TIMER_UPDATES => {
            unsafe {
                let _ = SetTimer(Some(window), TIMER_UPDATES, 60 * 60 * 1000, None);
            }
            let last = Registry.number("lastUpdateCheck").map(u64::from);
            if releases::due(last, unix_now()) {
                check_for_updates(window, false);
            }
            LRESULT(0)
        }
        WM_UPDATES => {
            let lookup = unsafe { Box::from_raw(lparam.0 as *mut Lookup) };
            finish_update_check(window, *lookup, wparam.0 != 0);
            LRESULT(0)
        }
        WM_WTSSESSION_CHANGE => {
            update(|app| match wparam.0 as u32 {
                WTS_SESSION_LOCK => app.locked = true,
                WTS_SESSION_UNLOCK => app.locked = false,
                WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => app.away = true,
                WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT => app.away = false,
                _ => {}
            });
            LRESULT(0)
        }
        // Plugged in or unplugged.
        WM_POWERBROADCAST if wparam.0 as u32 == PBT_APMPOWERSTATUSCHANGE => {
            power_changed();
            LRESULT(1)
        }
        WM_POWERBROADCAST if wparam.0 as u32 == PBT_POWERSETTINGCHANGE => {
            let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
            if setting.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                let off = setting.Data[0] == 0;
                update(|app| app.display_off = off);
            } else if setting.PowerSetting == GUID_POWER_SAVING_STATUS {
                power_changed();
            }
            LRESULT(1)
        }
        WM_CLOSE => {
            quit(window);
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            refresh_icon(window);
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        _ if message != 0 && message == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            APP.with_borrow(|app| {
                if let Some(app) = app {
                    add_tray_icon(window, app.icon);
                    send(app, Command::ShellRestarted);
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Changes what pauses the snow and tells the snow thread whether it may run.
fn update(change: impl FnOnce(&mut App)) {
    APP.with_borrow_mut(|app| {
        let Some(app) = app else { return };
        change(app);
        let active = !app.locked && !app.display_off && !app.away;
        send(app, Command::Active(active));
    });
}

/// Lets the snow thread know at once, so Pause on battery stops and starts the snow without waiting.
fn power_changed() {
    APP.with_borrow(|app| {
        if let Some(app) = app {
            send(app, Command::PowerChanged);
        }
    });
}

fn send(app: &App, command: Command) {
    let _ = app.commands.send(command);
    unsafe {
        let _ = SetEvent(app.wake);
    }
}

fn show_menu(window: HWND, x: i32, y: i32) {
    let Some(settings) = APP.with_borrow(|app| app.as_ref().map(|app| app.settings.clone())) else { return };
    let language = Language::from_langid(unsafe { GetUserDefaultUILanguage() });
    let screens = menu_screens();
    let items = menu::build(&settings, &Context { language, version: env!("CARGO_PKG_VERSION"), screens: &screens });
    let mut actions = Vec::new();
    let chosen = unsafe {
        let Some(menu) = native_menu(&items, &mut actions) else { return };
        // A tray menu only closes on an outside click while its owner is in the foreground.
        let _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0, x, y, window, None).0 as usize;
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen
    };
    // Item identifiers are one past the action's index, as 0 means nothing was chosen.
    if let Some(action) = chosen.checked_sub(1).and_then(|i| actions.into_iter().nth(i)) {
        choose(window, action);
    }
}

/// A Win32 popup menu for `items`, with each command's action pushed onto `actions`.
fn native_menu(items: &[Item], actions: &mut Vec<Action>) -> Option<HMENU> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for item in items {
            match item {
                Item::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
                Item::Submenu { label, items, enabled } => {
                    let Some(submenu) = native_menu(items, actions) else { continue };
                    let flags = MF_POPUP | if *enabled { MF_ENABLED } else { MF_GRAYED };
                    let _ = AppendMenuW(menu, flags, submenu.0 as usize, &HSTRING::from(label.as_str()));
                }
                Item::Command { label, action, checked, radio, enabled } => {
                    actions.push(action.clone());
                    let id = actions.len() as u32;
                    let flags =
                        MF_STRING | if *enabled { MF_ENABLED } else { MF_GRAYED } | if *checked && !radio { MF_CHECKED } else { MF_UNCHECKED };
                    let _ = AppendMenuW(menu, flags, id as usize, &HSTRING::from(label.as_str()));
                    if *checked && *radio {
                        let _ = CheckMenuRadioItem(menu, id, id, id, MF_BYCOMMAND.0);
                    }
                }
            }
        }
        Some(menu)
    }
}

fn choose(window: HWND, action: Action) {
    match action {
        Action::Quit => quit(window),
        Action::CheckForUpdates => check_for_updates(window, true),
        Action::ClearSnow => APP.with_borrow(|app| {
            if let Some(app) = app {
                send(app, Command::ClearSnow(!animations_enabled()));
            }
        }),
        Action::Change(change) => APP.with_borrow_mut(|app| {
            let Some(app) = app else { return };
            log!("{change:?}");
            // The menu shows the setting, so it only changes once the autostart entry did.
            if let Change::OpenAtLogin(on) = change
                && !registry::set_open_at_login(on)
            {
                return;
            }
            app.settings.apply(&change);
            app.settings.save(&mut Registry);
            send(app, Command::Settings(app.settings.clone()));
        }),
    }
}

/// The "Animation effects" setting: off means the person prefers less motion.
fn animations_enabled() -> bool {
    let mut enabled = windows::core::BOOL(1);
    let result = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&raw mut enabled).cast::<c_void>()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
    };
    result.is_err() || enabled.as_bool()
}

fn quit(window: HWND) {
    remove_tray_icon(window);
    if let Some(mut app) = APP.with_borrow_mut(Option::take) {
        send(&app, Command::Quit);
        if let Some(snow) = app.snow.take() {
            let _ = snow.join();
        }
        unsafe {
            let _ = DestroyIcon(app.icon);
        }
    }
    unsafe { PostQuitMessage(0) };
}

/// Recolours the tray's snowflake whenever the taskbar turns light or dark.
fn refresh_icon(window: HWND) {
    APP.with_borrow_mut(|app| {
        let Some(app) = app else { return };
        let light = light_taskbar();
        if light == app.light {
            return;
        }
        app.light = light;
        let icon = tray_icon(light);
        let data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            uFlags: NIF_ICON,
            hIcon: icon,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
            let _ = DestroyIcon(app.icon);
        }
        app.icon = icon;
    });
}

/// The tray's icon at its size for the display's scale, or when its picture can't be had, the app icon from the
/// executable's resources.
fn tray_icon(light: bool) -> HICON {
    unsafe {
        let size = GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem());
        if let Some(icon) = picture_icon(glyph(light), size) {
            return icon;
        }
        let Ok(instance) = GetModuleHandleW(None) else { return HICON::default() };
        LoadImageW(Some(instance.into()), PCWSTR(1 as *const u16), IMAGE_ICON, size, size, LR_DEFAULTCOLOR)
            .map_or(HICON::default(), |icon| HICON(icon.0))
    }
}

/// The picture of `pictures` closest above `size` pixels across, or the largest, as an icon that size.
fn picture_icon(pictures: &[(i32, &[u8])], size: i32) -> Option<HICON> {
    let (_, picture) = pictures.iter().find(|(side, _)| *side >= size).or(pictures.last())?;
    // Icon resources have kept the version Windows 3 gave them; a PNG is taken as one.
    unsafe { CreateIconFromResourceEx(picture, true, 0x0003_0000, size, size, LR_DEFAULTCOLOR) }.ok()
}

fn add_tray_icon(window: HWND, icon: HICON) {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    for (slot, unit) in data.szTip.iter_mut().zip("Snowfall".encode_utf16()) {
        *slot = unit;
    }
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

fn remove_tray_icon(window: HWND) {
    let data = NOTIFYICONDATAW { cbSize: size_of::<NOTIFYICONDATAW>() as u32, hWnd: window, uID: 1, ..Default::default() };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// The connected displays as the Screens submenu lists them, from left to right.
fn menu_screens() -> Vec<menu::Screen> {
    let mut monitors = screen::monitors(&screen::display_names());
    monitors.sort_by_key(|monitor| (monitor.bounds.left, monitor.bounds.top));
    monitors
        .into_iter()
        .map(|monitor| menu::Screen {
            // Windows numbers its displays as `\.\DISPLAY2` does, and so does its Settings app.
            number: monitor.name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().ok(),
            id: monitor.id,
            name: monitor.friendly,
            primary: monitor.primary,
        })
        .collect()
}

/// A notification from the tray icon, as Windows shows it: a toast on Windows 10 and 11.
fn notify(window: HWND, text: &str) {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };
    // The text fields are fixed arrays; whatever doesn't fit is cut, leaving room for the closing NUL.
    for (slot, unit) in data.szInfoTitle.iter_mut().take(63).zip("Snowfall".encode_utf16()) {
        *slot = unit;
    }
    for (slot, unit) in data.szInfo.iter_mut().take(255).zip(text.encode_utf16()) {
        *slot = unit;
    }
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

/// What a look for updates found.
enum Lookup {
    /// Offline, or GitHub didn't answer: nothing is said, and the next look is soon.
    Failed,
    UpToDate,
    Newer(releases::Release),
}

/// Looks on GitHub for a newer release on a thread of its own; the answer comes back as `WM_UPDATES`.
fn check_for_updates(window: HWND, manual: bool) {
    let started = APP.with_borrow_mut(|app| match app {
        Some(app) if !app.checking => {
            app.checking = true;
            true
        }
        _ => false,
    });
    if !started {
        return;
    }
    let target = window.0 as isize;
    let _ = thread::Builder::new().name("updates".into()).spawn(move || {
        let lookup = match releases::fetch() {
            None => Lookup::Failed,
            Some(answer) => releases::newer(&answer, env!("CARGO_PKG_VERSION")).map_or(Lookup::UpToDate, Lookup::Newer),
        };
        let lookup = Box::into_raw(Box::new(lookup));
        unsafe {
            if PostMessageW(Some(HWND(target as _)), WM_UPDATES, WPARAM(manual as usize), LPARAM(lookup as isize)).is_err() {
                drop(Box::from_raw(lookup));
            }
        }
    });
}

fn finish_update_check(window: HWND, lookup: Lookup, manual: bool) {
    let strings = Language::from_langid(unsafe { GetUserDefaultUILanguage() }).strings();
    if !matches!(lookup, Lookup::Failed) {
        Registry.set_number("lastUpdateCheck", unix_now() as u32);
    }
    match lookup {
        Lookup::Failed => log!("update check failed"),
        Lookup::UpToDate => {
            log!("up to date");
            if manual {
                notify(window, strings.up_to_date);
            }
        }
        Lookup::Newer(release) => {
            log!("update available: {}", release.version);
            notify(window, &strings.update_available.replace("{version}", &release.version));
            APP.with_borrow_mut(|app| {
                if let Some(app) = app {
                    app.release_page = Some(release.url);
                }
            });
        }
    }
    APP.with_borrow_mut(|app| {
        if let Some(app) = app {
            app.checking = false;
        }
    });
}

/// Opens the page of the release the notification told of, in the browser.
fn open_release_page() {
    let Some(url) = APP.with_borrow(|app| app.as_ref().and_then(|app| app.release_page.clone())) else { return };
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}
