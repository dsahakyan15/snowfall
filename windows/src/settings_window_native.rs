//! One activating, custom-framed settings window; overlays keep their no-activate styles.
use crate::menu::Context;
use crate::settings::Settings;
use crate::settings_window::{self as model, Activation, CONTENT, Focus, HEIGHT, Hit, Kind, Layout, Rect, TOOLBAR, Tab, View, WIDTH};
use crate::{app, log};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::size_of;
use std::time::Instant;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, PAINTSTRUCT,
    ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, Interface, PCWSTR, Result, w};
use windows_numerics::Vector2;
const CLASS: PCWSTR = w!("SnowfallSettings");
// Keep taskbar/system-menu behavior; WM_NCCALCSIZE removes the stock frame.
const STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_POPUP.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);
const ANIMATION_TIMER: usize = 1;
struct Animation {
    control: usize,
    from: f32,
    to: f32,
    started: Instant,
}
impl Animation {
    fn value(&self) -> f32 {
        let t = (self.started.elapsed().as_secs_f32() / 0.15).min(1.0);
        self.from + (self.to - self.from) * (t * t * (3.0 - 2.0 * t))
    }
}
struct Window {
    handle: HWND,
    controller: HWND,
    view: View,
    layout: Layout,
    dpi: u32,
    dark: bool,
    canvas: Option<Canvas>,
    focus: Option<Focus>,
    keyboard: bool,
    focused: bool,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    mouse: (f32, f32),
    animations: Vec<Animation>,
}
thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
    static LAST_TAB: Cell<Tab> = const { Cell::new(Tab::Snow) };
}
pub fn is_open() -> bool {
    WINDOW.with_borrow(|w| w.is_some())
}
pub fn open(controller: HWND, settings: &Settings, context: &Context) -> Result<()> {
    if let Some(handle) = WINDOW.with_borrow(|w| w.as_ref().map(|w| w.handle)) {
        unsafe {
            let _ = ShowWindow(handle, SW_RESTORE);
            let _ = SetForegroundWindow(handle);
            let _ = SetFocus(Some(handle));
        }
        return Ok(());
    }
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let icon = LoadIconW(Some(instance.into()), PCWSTR(std::ptr::without_provenance(1))).unwrap_or_default();
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            hIcon: icon,
            hIconSm: icon,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS,
            ..Default::default()
        });
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let (mut dpi, mut dpi_y) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        let (width, height) = frame_size(dpi);
        let x = info.rcWork.left + ((info.rcWork.right - info.rcWork.left - width) / 2).max(0);
        let y = info.rcWork.top + ((info.rcWork.bottom - info.rcWork.top - height) / 2).max(0);
        // An unowned app window gets its icon and its own taskbar button.
        let handle = CreateWindowExW(WS_EX_APPWINDOW, CLASS, w!("Snowfall"), STYLE, x, y, width, height, None, None, Some(instance.into()), None)?;
        let view = model::build(settings, context);
        let layout = Layout::new(&view, LAST_TAB.get(), 0.0);
        let dark = view.theme.dark(dark_theme());
        let dpi = GetDpiForWindow(handle).max(96);
        let canvas = match Canvas::new(handle, dpi) {
            Ok(canvas) => canvas,
            Err(error) => {
                let _ = DestroyWindow(handle);
                return Err(error);
            }
        };
        WINDOW.set(Some(Window {
            handle,
            controller,
            view,
            layout,
            dpi,
            dark,
            canvas: Some(canvas),
            focus: None,
            keyboard: false,
            focused: false,
            hover: None,
            pressed: None,
            mouse: (-1.0, -1.0),
            animations: vec![],
        }));
        decorate(handle, dark);
        let _ = ShowWindow(handle, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(handle);
        let _ = SetFocus(Some(handle));
    }
    Ok(())
}
fn frame_size(dpi: u32) -> (i32, i32) {
    let scale = dpi as f32 / 96.0;
    ((WIDTH * scale).round() as i32, (HEIGHT * scale).round() as i32)
}
pub fn refresh(settings: &Settings, context: &Context) {
    let caption = WINDOW.with_borrow_mut(|w| {
        let window = w.as_mut()?;
        let view = model::build(settings, context);
        let layout = Layout::new(&view, window.layout.tab, window.layout.scroll);
        for (i, c) in layout.controls.iter().enumerate() {
            let row = c.row(&view, layout.tab);
            if row.kind != Kind::Toggle {
                continue;
            }
            if let Some(old) = window.layout.controls.get(i) {
                let old = old.row(&window.view, layout.tab);
                if old.kind == Kind::Toggle && old.choices[0].checked != row.choices[0].checked {
                    let from = window.animations.iter().find(|a| a.control == i).map_or(f32::from(old.choices[0].checked), Animation::value);
                    window.animations.retain(|a| a.control != i);
                    window.animations.push(Animation { control: i, from, to: f32::from(row.choices[0].checked), started: Instant::now() });
                }
            }
        }
        window.view = view;
        window.layout = layout;
        window.dark = window.view.theme.dark(dark_theme());
        if let Some(Focus::Control(i)) = window.focus
            && window.layout.controls.get(i).is_none_or(|c| !c.enabled(&window.view, window.layout.tab))
        {
            window.focus = Some(Focus::Tabs);
        }
        window.hover = None;
        window.pressed = None;
        if !window.animations.is_empty() {
            unsafe {
                SetTimer(Some(window.handle), ANIMATION_TIMER, 16, None);
            }
        }
        window.invalidate();
        Some((window.handle, window.dark))
    });
    if let Some((handle, dark)) = caption {
        decorate(handle, dark);
    }
}
pub fn refresh_theme() {
    let caption = WINDOW.with_borrow_mut(|w| {
        let w = w.as_mut()?;
        w.dark = w.view.theme.dark(dark_theme());
        w.invalidate();
        Some((w.handle, w.dark))
    });
    if let Some((handle, dark)) = caption {
        decorate(handle, dark);
    }
}
pub fn close() {
    if let Some(handle) = WINDOW.with_borrow(|w| w.as_ref().map(|w| w.handle)) {
        unsafe {
            let _ = DestroyWindow(handle);
        }
    }
}
fn dark_theme() -> bool {
    let mut light = 1u32;
    let mut bytes = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut light).cast::<c_void>()),
            Some(&mut bytes),
        )
    };
    status.is_ok() && light == 0
}
fn decorate(handle: HWND, dark: bool) {
    let enabled = BOOL::from(dark);
    unsafe {
        let _ = DwmSetWindowAttribute(handle, DWMWA_USE_IMMERSIVE_DARK_MODE, (&raw const enabled).cast(), size_of::<BOOL>() as u32);
        let corners = DWMWCP_ROUND;
        // Windows 10 ignores corner preferences; the extended frame still gives a shadow.
        let _ = DwmSetWindowAttribute(
            handle,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const corners).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        let _ = DwmExtendFrameIntoClientArea(handle, &MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: 1, cyBottomHeight: 1 });
    }
}
impl Window {
    fn invalidate(&self) {
        // Offscreen input tests have no HWND; a null HWND would invalidate the whole desktop.
        if self.handle.is_invalid() {
            return;
        }
        unsafe {
            let _ = InvalidateRect(Some(self.handle), None, false);
        }
    }
    fn point(&self, lparam: LPARAM) -> (f32, f32) {
        let s = 96.0 / self.dpi as f32;
        ((lparam.0 as u16 as i16) as f32 * s, ((lparam.0 >> 16) as u16 as i16) as f32 * s)
    }
    fn select_tab(&mut self, tab: Tab) {
        self.layout = Layout::new(&self.view, tab, 0.0);
        LAST_TAB.set(tab);
        self.focus = Some(Focus::Tabs);
        self.animations.clear();
        self.hover = None;
        self.pressed = None;
        self.invalidate();
    }
    fn scroll(&mut self, position: f32) {
        self.layout = Layout::new(&self.view, self.layout.tab, position);
        self.hover = None;
        self.invalidate();
    }
    fn key(&mut self, key: u16, shift: bool, control: bool, alt: bool, repeated: bool) -> Option<Activation> {
        self.keyboard = true;
        if key == VK_TAB.0 {
            if control {
                self.select_tab(self.layout.tab.next(shift));
            } else {
                let focus = self.layout.focus_next(&self.view, self.focus, shift);
                self.focus = Some(focus);
                self.scroll(self.layout.scroll_to(focus));
            }
        } else {
            let arrow = if key == VK_LEFT.0 {
                -1
            } else if key == VK_RIGHT.0 {
                1
            } else {
                0
            };
            let activate = key == VK_SPACE.0 || key == VK_RETURN.0 || (alt && key == VK_DOWN.0);
            if arrow != 0 || (activate && !repeated) {
                let focus = *self.focus.get_or_insert(Focus::Tabs);
                match focus {
                    Focus::Tabs if arrow != 0 => self.select_tab(self.layout.tab.next(arrow < 0)),
                    Focus::Control(i) => {
                        if alt && self.layout.controls[i].row(&self.view, self.layout.tab).kind != Kind::Popup {
                            return None;
                        }
                        return self.layout.activate(&self.view, i, None, arrow);
                    }
                    _ => {}
                }
            }
        }
        None
    }
    fn paint(&mut self) -> Result<()> {
        if self.canvas.is_none() {
            self.canvas = Some(Canvas::new(self.handle, self.dpi)?);
        }
        let result = self.canvas.as_ref().expect("canvas").paint(self);
        if result.as_ref().is_err_and(|e| e.code() == D2DERR_RECREATE_TARGET) {
            self.canvas = None;
            self.invalidate();
        }
        result
    }
}
unsafe extern "system" fn procedure(handle: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_NCCALCSIZE => LRESULT(0),
        WM_NCHITTEST => {
            let mut point = POINT { x: lparam.0 as u16 as i16 as i32, y: (lparam.0 >> 16) as u16 as i16 as i32 };
            unsafe {
                let _ = ScreenToClient(handle, &mut point);
            }
            let caption = WINDOW.with_borrow(|w| {
                w.as_ref().is_some_and(|w| w.layout.caption(point.x as f32 * 96.0 / w.dpi as f32, point.y as f32 * 96.0 / w.dpi as f32))
            });
            LRESULT(if caption { HTCAPTION } else { HTCLIENT } as isize)
        }
        WM_NCLBUTTONDBLCLK => LRESULT(0),
        WM_SYSCOMMAND if wparam.0 & 0xfff0 == SC_MAXIMIZE as usize || wparam.0 & 0xfff0 == SC_SIZE as usize => LRESULT(0),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(handle, &mut paint);
            }
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w
                    && let Err(e) = w.paint()
                {
                    log!("settings paint: {e}");
                }
            });
            unsafe {
                let _ = EndPaint(handle, &paint);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CLOSE => {
            close();
            LRESULT(0)
        }
        WM_NCDESTROY => {
            unsafe {
                let _ = KillTimer(Some(handle), ANIMATION_TIMER);
            }
            WINDOW.with_borrow_mut(|w| {
                w.take();
            });
            unsafe { DefWindowProcW(handle, message, wparam, lparam) }
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            refresh_theme();
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xffff) as u32;
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    w.dpi = dpi;
                    w.canvas = None;
                    w.invalidate();
                }
            });
            let rect = unsafe { &*(lparam.0 as *const RECT) };
            let (width, height) = frame_size(dpi);
            unsafe {
                let _ = SetWindowPos(handle, None, rect.left, rect.top, width, height, SWP_NOZORDER | SWP_NOACTIVATE);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    w.canvas = None;
                    w.invalidate();
                }
            });
            LRESULT(0)
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    w.focused = message == WM_SETFOCUS;
                    w.invalidate();
                }
            });
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ANIMATION_TIMER => {
            let finished = WINDOW.with_borrow_mut(|w| {
                let Some(w) = w else { return true };
                w.animations.retain(|a| a.started.elapsed().as_secs_f32() < 0.15);
                w.invalidate();
                w.animations.is_empty()
            });
            if finished {
                unsafe {
                    let _ = KillTimer(Some(handle), ANIMATION_TIMER);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let key = wparam.0 as u16;
            if key == VK_ESCAPE.0 {
                close();
                return LRESULT(0);
            }
            let (shift, control, alt) =
                unsafe { (GetKeyState(VK_SHIFT.0 as i32) < 0, GetKeyState(VK_CONTROL.0 as i32) < 0, GetKeyState(VK_MENU.0 as i32) < 0) };
            if message == WM_SYSKEYDOWN && !(alt && key == VK_DOWN.0) {
                return unsafe { DefWindowProcW(handle, message, wparam, lparam) };
            }
            let activation = WINDOW.with_borrow_mut(|w| {
                let w = w.as_mut()?;
                let a = w.key(key, shift, control, alt, lparam.0 & (1 << 30) != 0);
                w.invalidate();
                a
            });
            dispatch(activation);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    let p = w.point(lparam);
                    if w.mouse != p {
                        w.mouse = p;
                        w.hover = w.layout.hit(&w.view, p.0, p.1);
                        w.invalidate();
                    }
                }
            });
            let mut track =
                TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: handle, ..Default::default() };
            unsafe {
                let _ = TrackMouseEvent(&mut track);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE | WM_CAPTURECHANGED => {
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    w.hover = None;
                    w.pressed = None;
                    w.mouse = (-1.0, -1.0);
                    w.invalidate();
                }
            });
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    let delta = (wparam.0 >> 16) as u16 as i16 as f32 / 120.0;
                    w.scroll(w.layout.scroll - delta * 60.0);
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            unsafe {
                let _ = SetFocus(Some(handle));
                SetCapture(handle);
            }
            WINDOW.with_borrow_mut(|w| {
                if let Some(w) = w {
                    let (x, y) = w.point(lparam);
                    w.pressed = w.layout.hit(&w.view, x, y);
                    w.keyboard = false;
                    match w.pressed {
                        Some(Hit::Control(i, _)) => w.focus = Some(Focus::Control(i)),
                        Some(Hit::Tab(_)) => w.focus = Some(Focus::Tabs),
                        _ => {}
                    }
                    w.invalidate();
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let hit = WINDOW.with_borrow_mut(|w| {
                let w = w.as_mut()?;
                let (x, y) = w.point(lparam);
                let hit = w.layout.hit(&w.view, x, y);
                let pressed = w.pressed.take();
                w.invalidate();
                (hit == pressed).then_some(hit).flatten()
            });
            unsafe {
                let _ = ReleaseCapture();
            }
            match hit {
                Some(Hit::Light(0)) => close(),
                Some(Hit::Light(1)) => unsafe {
                    let _ = ShowWindow(handle, SW_MINIMIZE);
                },
                Some(Hit::Tab(tab)) => WINDOW.with_borrow_mut(|w| {
                    if let Some(w) = w {
                        w.select_tab(tab);
                    }
                }),
                Some(Hit::Control(i, j)) => {
                    let a = WINDOW.with_borrow(|w| {
                        let w = w.as_ref()?;
                        w.layout.activate(&w.view, i, Some(j), 0)
                    });
                    dispatch(a);
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(handle, message, wparam, lparam) },
    }
}
/// Neither choose() nor a native popup may run while the window state is borrowed.
fn dispatch(activation: Option<Activation>) {
    match activation {
        Some(Activation::Action(action)) => {
            if let Some(controller) = WINDOW.with_borrow(|w| w.as_ref().map(|w| w.controller)) {
                app::choose(controller, action);
            }
        }
        Some(Activation::Popup(i)) => {
            if let Err(e) = popup(i) {
                log!("settings popup: {e}");
            }
        }
        None => {}
    }
}
fn popup(index: usize) -> Result<()> {
    let data = WINDOW.with_borrow(|w| {
        let w = w.as_ref()?;
        let c = w.layout.controls.get(index)?;
        Some((w.handle, w.controller, w.dpi, c.bounds, c.row(&w.view, w.layout.tab).choices.clone()))
    });
    let Some((handle, controller, dpi, bounds, choices)) = data else { return Ok(()) };
    unsafe {
        let menu = CreatePopupMenu()?;
        let result = (|| -> Result<usize> {
            for (i, c) in choices.iter().enumerate() {
                let flags = MF_STRING | if c.checked { MF_CHECKED } else { MF_UNCHECKED } | if c.enabled { MF_ENABLED } else { MF_GRAYED };
                AppendMenuW(menu, flags, i + 1, &HSTRING::from(c.label.as_str()))?;
            }
            let mut position =
                POINT { x: (bounds.x * dpi as f32 / 96.0).round() as i32, y: ((bounds.y + bounds.height + 3.0) * dpi as f32 / 96.0).round() as i32 };
            let _ = ClientToScreen(handle, &mut position);
            Ok(TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTALIGN).0, position.x, position.y, handle, None).0 as usize)
        })();
        let _ = DestroyMenu(menu);
        let selected = result?;
        if let Some(choice) = selected.checked_sub(1).and_then(|i| choices.get(i)).filter(|c| c.enabled) {
            app::choose(controller, choice.action.clone());
        }
    }
    Ok(())
}

struct Palette {
    background: u32,
    toolbar: u32,
    panel: u32,
    border: u32,
    text: u32,
    muted: u32,
    track: u32,
    segment: u32,
    raised: u32,
    selected: u32,
    accent: u32,
}
impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: 0x1e1e1e,
                toolbar: 0x2a2a2a,
                panel: 0x2c2c2e,
                border: 0x414143,
                text: 0xf5f5f7,
                muted: 0x98989d,
                track: 0x48484a,
                segment: 0x3a3a3c,
                raised: 0x636366,
                selected: 0x3d3d3d,
                accent: 0x0a84ff,
            }
        } else {
            Self {
                background: 0xececec,
                toolbar: 0xe3e3e3,
                panel: 0xffffff,
                border: 0xd7d7d9,
                text: 0x1d1d1f,
                muted: 0x86868b,
                track: 0xd9d9de,
                segment: 0xe3e3e8,
                raised: 0xffffff,
                selected: 0xd3d3d3,
                accent: 0x0a84ff,
            }
        }
    }
}
struct Canvas {
    target: ID2D1RenderTarget,
    write: IDWriteFactory,
    formats: Vec<IDWriteTextFormat>,
    brush: ID2D1SolidColorBrush,
    factory: ID2D1Factory,
    stroke: ID2D1StrokeStyle,
}
impl Canvas {
    fn new(handle: HWND, dpi: u32) -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let mut rect = RECT::default();
            GetClientRect(handle, &mut rect)?;
            let target = factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES { dpiX: dpi as f32, dpiY: dpi as f32, ..Default::default() },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: handle,
                    pixelSize: D2D_SIZE_U { width: (rect.right - rect.left).max(1) as u32, height: (rect.bottom - rect.top).max(1) as u32 },
                    ..Default::default()
                },
            )?;
            Self::for_target(target.cast()?)
        }
    }
    /// Bitmap tests use the same resources and drawing path as the live window.
    fn for_target(target: ID2D1RenderTarget) -> Result<Self> {
        unsafe {
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut collection = None;
            write.GetSystemFontCollection(&mut collection, false)?;
            let (mut index, mut exists) = (0, BOOL(0));
            if let Some(c) = collection {
                c.FindFamilyName(w!("Segoe UI Variable Text"), &mut index, &mut exists)?;
            }
            let family = if exists.as_bool() { w!("Segoe UI Variable Text") } else { w!("Segoe UI") };
            let mut formats = Vec::new();
            for (size, weight) in [
                (13.0, DWRITE_FONT_WEIGHT_BOLD),
                (13.0, DWRITE_FONT_WEIGHT_MEDIUM),
                (11.5, DWRITE_FONT_WEIGHT_NORMAL),
                (11.5, DWRITE_FONT_WEIGHT_SEMI_BOLD),
                (12.0, DWRITE_FONT_WEIGHT_NORMAL),
                (11.0, DWRITE_FONT_WEIGHT_NORMAL),
                (13.0, DWRITE_FONT_WEIGHT_SEMI_BOLD),
            ] {
                let f = write.CreateTextFormat(family, None, weight, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, w!(""))?;
                f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                formats.push(f);
            }
            let brush = target.CreateSolidColorBrush(&color(0, 1.0), None)?;
            let factory: ID2D1Factory = target.GetFactory()?;
            let stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    ..Default::default()
                },
                None,
            )?;
            Ok(Self { target, write, formats, brush, factory, stroke })
        }
    }
    fn brush(&self, rgb: u32, alpha: f32) -> &ID2D1SolidColorBrush {
        unsafe {
            self.brush.SetColor(&color(rgb, alpha));
        }
        &self.brush
    }
    fn fill(&self, b: Rect, r: f32, rgb: u32, a: f32) {
        unsafe {
            self.target.FillRoundedRectangle(&rounded(b, r), self.brush(rgb, a));
        }
    }
    fn outline(&self, b: Rect, r: f32, rgb: u32, width: f32, alpha: f32) {
        unsafe {
            self.target.DrawRoundedRectangle(&rounded(b, r), self.brush(rgb, alpha), width, None);
        }
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), rgb: u32, width: f32) {
        unsafe {
            self.target.DrawLine(point(a.0, a.1), point(b.0, b.1), self.brush(rgb, 1.0), width, &self.stroke);
        }
    }
    fn circle(&self, x: f32, y: f32, r: f32, rgb: u32, fill: bool) {
        let e = D2D1_ELLIPSE { point: point(x, y), radiusX: r, radiusY: r };
        unsafe {
            if fill {
                self.target.FillEllipse(&e, self.brush(rgb, 1.0));
            } else {
                self.target.DrawEllipse(&e, self.brush(rgb, 1.0), 1.6, &self.stroke);
            }
        }
    }
    fn polygon(&self, points: &[(f32, f32)], rgb: u32) -> Result<()> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.BeginFigure(point(points[0].0, points[0].1), D2D1_FIGURE_BEGIN_FILLED);
            for &(x, y) in &points[1..] {
                sink.AddLine(point(x, y));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&path, self.brush(rgb, 1.0), None);
        }
        Ok(())
    }
    fn text(&self, text: &str, b: Rect, font: usize, rgb: u32, center: bool) -> Result<()> {
        let units: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let layout = self.write.CreateTextLayout(&units, &self.formats[font], b.width.max(1.0), b.height.max(1.0))?;
            layout.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            if center {
                layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            }
            let ellipsis = self.write.CreateEllipsisTrimmingSign(&self.formats[font])?;
            layout.SetTrimming(&DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, ..Default::default() }, &ellipsis)?;
            self.target.DrawTextLayout(point(b.x, b.y), &layout, self.brush(rgb, 1.0), D2D1_DRAW_TEXT_OPTIONS_CLIP);
        }
        Ok(())
    }
    fn paint(&self, w: &Window) -> Result<()> {
        unsafe {
            self.target.BeginDraw();
        }
        let result = self.draw(w);
        let end = unsafe { self.target.EndDraw(None, None) };
        result.and(end)
    }
    fn draw(&self, w: &Window) -> Result<()> {
        let p = Palette::new(w.dark);
        unsafe {
            self.target.Clear(Some(&color(p.background, 1.0)));
        }
        self.fill(Rect::new(0.0, 0.0, WIDTH, TOOLBAR), 0.0, p.toolbar, 1.0);
        self.line((0.0, TOOLBAR - 0.5), (WIDTH, TOOLBAR - 0.5), p.border, 1.0);
        self.text(w.view.pages[w.layout.tab as usize].title, Rect::new(100.0, 8.0, WIDTH - 200.0, 20.0), 0, p.text, true)?;
        for i in 0..model::LIGHT_COUNT {
            let r = model::light(i);
            self.circle(r.x + 6.0, r.y + 6.0, 6.0, [0xff5f57, 0xfebc2e][i], true);
        }
        if model::LIGHTS.contains(w.mouse.0, w.mouse.1) {
            self.line((18.0, 17.0), (22.0, 21.0), 0x6b1915, 1.1);
            self.line((22.0, 17.0), (18.0, 21.0), 0x6b1915, 1.1);
            self.line((38.0, 19.0), (42.0, 19.0), 0x805400, 1.2);
        }
        for tab in Tab::ALL {
            let b = model::tab_bounds(tab);
            let selected = tab == w.layout.tab;
            if selected {
                self.fill(b, 7.0, p.selected, 1.0);
            } else if w.hover == Some(Hit::Tab(tab)) {
                self.fill(b, 7.0, p.text, 0.035);
            }
            let rgb = if selected { p.accent } else { p.muted };
            self.icon(tab, b.x + b.width / 2.0, b.y + 21.0, rgb)?;
            self.text(w.view.pages[tab as usize].title, Rect::new(b.x - 3.0, b.y + 39.0, b.width + 6.0, 17.0), 5, rgb, true)?;
            if selected && w.focus == Some(Focus::Tabs) && w.keyboard && w.focused {
                self.outline(b.inset(-2.0), 9.0, p.accent, 3.0, 0.5);
            }
        }
        unsafe {
            self.target.PushAxisAlignedClip(&native_rect(CONTENT), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        let result = self.content(w, &p);
        unsafe {
            self.target.PopAxisAlignedClip();
        }
        result?;
        self.outline(Rect::new(0.5, 0.5, WIDTH - 1.0, HEIGHT - 1.0), 12.0, p.border, 1.0, 1.0);
        Ok(())
    }
    fn content(&self, w: &Window, p: &Palette) -> Result<()> {
        let page = &w.view.pages[w.layout.tab as usize];
        for placed in &w.layout.groups {
            let group = &page.groups[placed.group];
            let b = placed.bounds;
            if !group.heading.is_empty() {
                self.text(group.heading, Rect::new(b.x + 2.0, b.y - 24.0, b.width, 19.0), 3, p.muted, false)?;
            }
            self.fill(b, 10.0, p.panel, 1.0);
            self.outline(b.inset(0.5), 10.0, p.border, 1.0, 0.7);
            if !group.caption.is_empty() {
                self.text(group.caption, Rect::new(b.x + 2.0, b.y + b.height + 5.0, b.width - 4.0, 18.0), 2, p.muted, false)?;
            }
        }
        for (i, placed) in w.layout.rows.iter().enumerate() {
            let row = &page.groups[placed.group].rows[placed.row];
            let b = placed.bounds;
            let c = &w.layout.controls[i];
            let enabled = c.enabled(&w.view, w.layout.tab);
            let foreground = if enabled { p.text } else { p.muted };
            if placed.row > 0 {
                self.line((b.x + 14.0, b.y), (b.x + b.width - 1.0, b.y), p.border, 0.65);
            }
            let hovered = matches!(w.hover,Some(Hit::Control(index,_)) if index==i);
            if row.kind == Kind::Checkbox {
                if hovered {
                    self.fill(b.inset(2.0), 5.0, p.text, 0.035);
                }
                let check = Rect::new(b.x + 14.0, b.y + 8.0, 14.0, 14.0);
                self.fill(check, 4.0, if row.choices[0].checked { p.accent } else { p.panel }, 1.0);
                self.outline(check.inset(0.5), 4.0, if row.choices[0].checked { p.accent } else { p.border }, 1.0, 1.0);
                if row.choices[0].checked {
                    self.line((check.x + 3.0, check.y + 7.0), (check.x + 6.0, check.y + 10.0), 0xffffff, 1.5);
                    self.line((check.x + 6.0, check.y + 10.0), (check.x + 11.0, check.y + 4.0), 0xffffff, 1.5);
                }
                self.text(&row.label, Rect::new(b.x + 36.0, b.y, b.width - 150.0, b.height), 1, p.text, false)?;
                self.text(row.trailing, Rect::new(b.x + b.width - 108.0, b.y, 94.0, b.height), 2, p.muted, true)?;
            } else if row.kind != Kind::Themes {
                let width = c.bounds.x - b.x - 26.0;
                self.text(
                    &row.label,
                    Rect::new(
                        b.x + 14.0,
                        b.y + if row.caption.is_empty() { 0.0 } else { 7.0 },
                        width,
                        if row.caption.is_empty() { b.height } else { 19.0 },
                    ),
                    1,
                    foreground,
                    false,
                )?;
                if !row.caption.is_empty() {
                    self.text(&row.caption, Rect::new(b.x + 14.0, b.y + 26.0, width, 18.0), 2, p.muted, false)?;
                }
            }
            let r = c.bounds;
            match row.kind {
                Kind::Toggle => {
                    let checked = row.choices[0].checked;
                    let position = w.animations.iter().find(|a| a.control == i).map_or(f32::from(checked), Animation::value);
                    self.fill(r, 9.5, if checked { p.accent } else { p.track }, if enabled { 1.0 } else { 0.5 });
                    if hovered {
                        self.fill(r, 9.5, p.text, 0.05);
                    }
                    let knob = Rect::new(r.x + 2.0 + 13.0 * position, r.y + 2.0, 15.0, 15.0);
                    self.fill(Rect::new(knob.x - 0.5, knob.y + 1.0, 16.0, 16.0), 8.0, 0x000000, 0.12);
                    self.fill(knob, 7.5, 0xffffff, if enabled { 1.0 } else { 0.65 });
                }
                Kind::Segments => {
                    self.fill(r, 6.0, p.segment, 1.0);
                    for (j, choice) in row.choices.iter().enumerate() {
                        let b = c.choices[j].inset(2.0);
                        if choice.checked {
                            self.fill(Rect::new(b.x, b.y + 1.0, b.width, b.height), 4.0, 0x000000, 0.12);
                            self.fill(b, 4.0, p.raised, 1.0);
                        } else if w.hover == Some(Hit::Control(i, j)) {
                            self.fill(b, 4.0, p.text, 0.04);
                        }
                        self.text(&choice.label, b, 4, p.text, true)?;
                    }
                }
                Kind::Popup | Kind::Button => {
                    self.fill(Rect::new(r.x, r.y + 1.0, r.width, r.height), 5.0, 0x000000, 0.10);
                    self.fill(r, 5.0, if w.dark { 0x454547 } else { 0xffffff }, 1.0);
                    self.outline(r.inset(0.5), 5.0, p.border, 0.7, 1.0);
                    if hovered {
                        self.fill(r, 5.0, p.text, 0.045);
                    }
                    if matches!(w.pressed,Some(Hit::Control(index,_)) if index==i) {
                        self.fill(r, 5.0, p.accent, 0.12);
                    }
                    let label =
                        if row.kind == Kind::Popup { row.choices.iter().find(|c| c.checked).unwrap_or(&row.choices[0]) } else { &row.choices[0] };
                    self.text(
                        &label.label,
                        Rect::new(r.x + 6.0, r.y, r.width - if row.kind == Kind::Popup { 24.0 } else { 12.0 }, r.height),
                        4,
                        p.text,
                        true,
                    )?;
                    if row.kind == Kind::Popup {
                        self.line((r.x + r.width - 14.0, r.y + 11.0), (r.x + r.width - 11.0, r.y + 14.0), p.muted, 1.3);
                        self.line((r.x + r.width - 11.0, r.y + 14.0), (r.x + r.width - 8.0, r.y + 11.0), p.muted, 1.3);
                    }
                }
                Kind::Themes => {
                    for (j, choice) in row.choices.iter().enumerate() {
                        let b = c.choices[j];
                        let preview = Rect::new(b.x, b.y, 78.0, 52.0);
                        self.preview(preview, j)?;
                        if choice.checked {
                            self.outline(preview.inset(-2.0), 9.0, p.accent, 3.0, 1.0);
                        } else if w.hover == Some(Hit::Control(i, j)) {
                            self.outline(preview.inset(-1.5), 8.5, p.muted, 1.5, 0.6);
                        }
                        self.text(
                            &choice.label,
                            Rect::new(b.x - 7.0, b.y + 61.0, b.width + 14.0, 20.0),
                            if choice.checked { 6 } else { 1 },
                            p.text,
                            true,
                        )?;
                    }
                }
                Kind::Checkbox => {}
            }
            if w.focus == Some(Focus::Control(i)) && w.focused && w.keyboard && enabled {
                let focus = if row.kind == Kind::Themes {
                    let j = row.choices.iter().position(|c| c.checked).unwrap_or(0);
                    let b = c.choices[j];
                    Rect::new(b.x, b.y, 78.0, 52.0)
                } else {
                    r
                };
                self.outline(focus.inset(-3.0), if row.kind == Kind::Toggle { 12.5 } else { 8.0 }, p.accent, 3.0, 0.5);
            }
        }
        if !page.footer.is_empty() {
            self.text(page.footer, Rect::new(22.0, w.layout.bottom - w.layout.scroll - 28.0, WIDTH - 44.0, 18.0), 2, p.muted, true)?;
        }
        if w.layout.bottom > HEIGHT {
            let viewport = HEIGHT - TOOLBAR - 12.0;
            let total = w.layout.bottom - TOOLBAR;
            let height = viewport * viewport / total;
            let y = TOOLBAR + 6.0 + (viewport - height) * w.layout.scroll / (w.layout.bottom - HEIGHT);
            self.fill(Rect::new(WIDTH - 8.0, y, 3.0, height), 1.5, p.muted, 0.45);
        }
        Ok(())
    }
    fn icon(&self, tab: Tab, x: f32, y: f32, rgb: u32) -> Result<()> {
        match tab {
            Tab::Snow => {
                for i in 0..6 {
                    let a = i as f32 * std::f32::consts::PI / 3.0;
                    let (dx, dy) = (a.cos(), a.sin());
                    self.line((x, y), (x + 10.0 * dx, y + 10.0 * dy), rgb, 1.6);
                    for side in [-1.0, 1.0] {
                        self.line((x + 6.5 * dx, y + 6.5 * dy), (x + 4.0 * dx + side * 2.5 * dy, y + 4.0 * dy - side * 2.5 * dx), rgb, 1.6);
                    }
                }
            }
            Tab::Windows => {
                self.outline(Rect::new(x - 10.0, y - 8.0, 20.0, 16.0), 3.0, rgb, 1.6, 1.0);
                self.line((x - 9.0, y - 3.0), (x + 9.0, y - 3.0), rgb, 1.6);
            }
            Tab::Play => {
                self.circle(x, y - 5.0, 4.2, rgb, false);
                self.circle(x, y + 5.0, 6.5, rgb, false);
            }
            Tab::Appearance => {
                unsafe {
                    self.target.PushAxisAlignedClip(&native_rect(Rect::new(x - 11.0, y - 11.0, 11.0, 22.0)), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                }
                self.circle(x, y, 9.0, rgb, true);
                unsafe {
                    self.target.PopAxisAlignedClip();
                }
                self.circle(x, y, 9.0, rgb, false);
            }
            Tab::System => {
                self.circle(x, y, 4.2, rgb, false);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::PI / 4.0;
                    self.line((x + 7.5 * a.cos(), y + 7.5 * a.sin()), (x + 10.5 * a.cos(), y + 10.5 * a.sin()), rgb, 1.6);
                }
            }
        }
        Ok(())
    }
    fn preview(&self, b: Rect, theme: usize) -> Result<()> {
        let dark = theme == 2;
        self.fill(b, 7.0, if dark { 0x202022 } else { 0xf3f3f5 }, 1.0);
        unsafe {
            self.target.PushAxisAlignedClip(&native_rect(Rect::new(b.x, b.y, b.width, 13.0)), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        self.fill(b, 7.0, if dark { 0x454547 } else { 0xdddddf }, 1.0);
        unsafe {
            self.target.PopAxisAlignedClip();
        }
        if theme == 0 {
            // Clip the diagonal half to the preview's rounded window geometry.
            unsafe {
                let mask = self.factory.CreateRoundedRectangleGeometry(&rounded(b, 7.0))?;
                let layer = self.target.CreateLayer(None)?;
                let mut parameters = D2D1_LAYER_PARAMETERS {
                    contentBounds: native_rect(b),
                    geometricMask: std::mem::ManuallyDrop::new(Some(mask.cast()?)),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: windows_numerics::Matrix3x2::identity(),
                    opacity: 1.0,
                    ..Default::default()
                };
                self.target.PushLayer(&parameters, &layer);
                let result = self.polygon(&[(b.x + b.width, b.y), (b.x + b.width, b.y + b.height), (b.x, b.y + b.height)], 0x242426);
                self.target.PopLayer();
                std::mem::ManuallyDrop::drop(&mut parameters.geometricMask);
                result?;
            }
        }
        self.fill(Rect::new(b.x + 10.0, b.y + 23.0, 37.0, 3.0), 1.5, if dark { 0x85858a } else { 0xb0b0b5 }, 1.0);
        self.fill(Rect::new(b.x + 10.0, b.y + 31.0, 27.0, 3.0), 1.5, if dark { 0x65656a } else { 0xc8c8cc }, 1.0);
        self.fill(Rect::new(b.x + 53.0, b.y + 37.0, 16.0, 7.0), 3.5, 0x0a84ff, 1.0);
        self.outline(b.inset(0.5), 7.0, if dark { 0x55555a } else { 0xc5c5c9 }, 0.7, 1.0);
        Ok(())
    }
}
fn color(rgb: u32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: ((rgb >> 16) & 255) as f32 / 255.0, g: ((rgb >> 8) & 255) as f32 / 255.0, b: (rgb & 255) as f32 / 255.0, a }
}
fn point(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}
fn native_rect(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.width, bottom: r.y + r.height }
}
fn rounded(r: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT { rect: native_rect(r), radiusX: radius, radiusY: radius }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Action, Screen};
    use crate::settings::WindowTheme;
    use crate::strings::Language;
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
    #[test]
    fn renders_every_localized_tab_and_theme_at_each_dpi_and_saves_pngs() {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
        }
        {
            let imaging: IWICImagingFactory = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap() };
            let factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).unwrap() };
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/settings-shots");
            std::fs::create_dir_all(&directory).unwrap();
            let screens = [
                Screen { id: "a".into(), name: Some("DELL U2720Q".into()), primary: true, number: Some(1) },
                Screen { id: "b".into(), name: None, primary: false, number: Some(3) },
            ];
            for language in [Language::English, Language::Russian] {
                for dark in [false, true] {
                    for tab in Tab::ALL {
                        for dpi in [96, 144, 192] {
                            for behind_windows in [false, true] {
                                let (width, height) = frame_size(dpi);
                                let (width, height) = (width as u32, height as u32);
                                let bitmap =
                                    unsafe { imaging.CreateBitmap(width, height, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad).unwrap() };
                                let target = unsafe {
                                    factory
                                        .CreateWicBitmapRenderTarget(
                                            &bitmap,
                                            &D2D1_RENDER_TARGET_PROPERTIES {
                                                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                                                dpiX: dpi as f32,
                                                dpiY: dpi as f32,
                                                ..Default::default()
                                            },
                                        )
                                        .unwrap()
                                };
                                let canvas = Canvas::for_target(target).unwrap();
                                let view = model::build(
                                    &Settings { behind_windows, ..Settings::default() },
                                    &Context { language, version: "0.1.0", screens: &screens },
                                );
                                let layout = Layout::new(&view, tab, 0.0);
                                let mut w = Window {
                                    handle: HWND::default(),
                                    controller: HWND::default(),
                                    view,
                                    layout,
                                    dpi,
                                    dark,
                                    canvas: None,
                                    focus: None,
                                    keyboard: false,
                                    focused: true,
                                    hover: None,
                                    pressed: None,
                                    mouse: (-1.0, -1.0),
                                    animations: vec![],
                                };
                                canvas.paint(&w).unwrap();
                                if dpi == 96 && !behind_windows {
                                    let name = format!(
                                        "{}-{}-{}.png",
                                        ["snow", "windows", "snow-play", "appearance", "system"][tab as usize],
                                        if dark { "dark" } else { "light" },
                                        if language == Language::English { "en" } else { "ru" }
                                    );
                                    save_png(&imaging, &bitmap, &directory.join(name));
                                }
                                // Also exercise traffic-light hover glyphs and every keyboard focus halo.
                                w.mouse = (20.0, 19.0);
                                w.keyboard = true;
                                w.focus = Some(Focus::Tabs);
                                canvas.paint(&w).unwrap();
                                for i in 0..w.layout.controls.len() {
                                    w.focus = Some(Focus::Control(i));
                                    w.hover = Some(Hit::Control(i, 0));
                                    canvas.paint(&w).unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
        unsafe {
            CoUninitialize();
        }
    }
    fn save_png(imaging: &IWICImagingFactory, bitmap: &IWICBitmap, path: &std::path::Path) {
        unsafe {
            let stream = imaging.CreateStream().unwrap();
            stream.InitializeFromFilename(&HSTRING::from(path.to_string_lossy().as_ref()), GENERIC_WRITE.0).unwrap();
            let encoder = imaging.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()).unwrap();
            encoder.Initialize(&stream, WICBitmapEncoderNoCache).unwrap();
            let mut frame = None;
            encoder.CreateNewFrame(&mut frame, std::ptr::null_mut()).unwrap();
            let frame = frame.unwrap();
            frame.Initialize(None).unwrap();
            let (mut width, mut height) = (0, 0);
            bitmap.GetSize(&mut width, &mut height).unwrap();
            frame.SetSize(width, height).unwrap();
            let mut format = GUID_WICPixelFormat32bppBGRA;
            frame.SetPixelFormat(&mut format).unwrap();
            frame.WriteSource(bitmap, std::ptr::null()).unwrap();
            frame.Commit().unwrap();
            encoder.Commit().unwrap();
        }
    }
    #[test]
    fn keyboard_tabs_controls_popup_arrows_and_last_tab() {
        let view = model::build(&Settings::default(), &Context { language: Language::English, version: "1", screens: &[] });
        let layout = Layout::new(&view, Tab::Snow, 0.0);
        let mut w = Window {
            handle: HWND::default(),
            controller: HWND::default(),
            view,
            layout,
            dpi: 96,
            dark: false,
            canvas: None,
            focus: None,
            keyboard: false,
            focused: true,
            hover: None,
            pressed: None,
            mouse: (0.0, 0.0),
            animations: vec![],
        };
        w.key(VK_TAB.0, false, false, false, false);
        assert_eq!(w.focus, Some(Focus::Tabs));
        w.key(VK_TAB.0, false, false, false, false);
        assert_eq!(w.focus, Some(Focus::Control(0)));
        w.key(VK_TAB.0, true, false, false, false);
        assert_eq!(w.focus, Some(Focus::Tabs));
        w.focus = Some(Focus::Control(2));
        assert_eq!(w.key(VK_DOWN.0, false, false, true, false), None);
        w.focus = Some(Focus::Tabs);
        w.key(VK_RIGHT.0, false, false, false, false);
        assert_eq!(w.layout.tab, Tab::Windows);
        w.key(VK_TAB.0, true, true, false, false);
        assert_eq!(w.layout.tab, Tab::Snow);
        w.key(VK_TAB.0, true, true, false, false);
        assert_eq!(w.layout.tab, Tab::System);
        assert_eq!(LAST_TAB.get(), Tab::System);
        w.select_tab(Tab::Appearance);
        w.focus = Some(Focus::Control(0));
        assert_eq!(
            w.key(VK_RIGHT.0, false, false, false, false),
            Some(Activation::Action(Action::Change(crate::settings::Change::WindowTheme(WindowTheme::Light))))
        );
        w.focus = Some(Focus::Control(1));
        for (key, alt) in [(VK_DOWN.0, true), (VK_SPACE.0, false), (VK_RETURN.0, false)] {
            assert_eq!(w.key(key, false, false, alt, false), Some(Activation::Popup(1)));
        }
        assert_eq!(w.key(VK_SPACE.0, false, false, false, true), None);
        LAST_TAB.set(Tab::Snow);
    }
    #[test]
    fn borderless_fixed_window_scales_without_caption_padding() {
        assert_eq!(frame_size(96), (560, 480));
        assert_eq!(frame_size(144), (840, 720));
        assert_eq!(frame_size(192), (1120, 960));
    }
}
