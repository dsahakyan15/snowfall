//! Small input windows for Snowballs. They and the simulations live on the snow thread; callbacks only queue input.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::size_of;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CombineRgn, CreateEllipticRgn, CreateRectRgn, DeleteObject, GetStockObject, HBRUSH, RGN_AND, RGN_DIFF, SetWindowRgn, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, ReleaseCapture, SetCapture, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Error, PCWSTR, Result, w};

use snowfall_core::geometry::V2;
use snowfall_core::play::{Snowball, SnowballId};

use crate::log;
use crate::screen::Screen;
use crate::settings::Settings;

const CLASS: PCWSTR = w!("SnowfallSnowball");

enum Input {
    Press(SnowballId, V2),
    Move(V2),
    Release(V2),
    Cancel,
}

thread_local! {
    static INPUT: RefCell<Vec<Input>> = const { RefCell::new(Vec::new()) };
}

fn enqueue(input: Input) {
    INPUT.with_borrow_mut(|queue| queue.push(input));
}

/// Owns every hit window, including the capturing window while its ball crosses to another screen.
#[derive(Default)]
pub struct Grabs {
    windows: HashMap<SnowballId, HitWindow>,
    held: Option<SnowballId>,
    samples: VecDeque<(Instant, V2)>,
    registered: bool,
}

impl Grabs {
    pub fn has_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    /// Runs outside window callbacks, so moving a window can safely dispatch more messages.
    pub fn input(&mut self, screens: &mut [Screen], now: i64) {
        let input = INPUT.with_borrow_mut(std::mem::take);
        for event in input {
            match event {
                Input::Press(id, point) if self.held.is_none() => {
                    for screen in screens.iter_mut() {
                        let local = screen.local(point.x, point.y);
                        if screen.simulation.grab(id, local) {
                            self.held = Some(id);
                            self.samples.clear();
                            self.sample(point);
                            screen.wake(now);
                            break;
                        }
                    }
                    if self.held.is_none() {
                        unsafe {
                            let _ = ReleaseCapture();
                        }
                    }
                }
                Input::Move(point) => self.move_to(screens, point, now),
                Input::Release(point) => {
                    self.move_to(screens, point, now);
                    self.release(screens, now);
                }
                Input::Cancel => self.release(screens, now),
                _ => {}
            }
        }
        // A non-activating window cannot rely on foreground capture outside its visible region.
        // While dragging only, also sample the pointer so a fast move or a release outside it cannot strand the ball.
        if self.held.is_some() {
            if let Some(point) = pointer() {
                self.move_to(screens, point, now);
            }
            if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } >= 0 {
                self.release(screens, now);
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
        }
    }

    fn sample(&mut self, point: V2) {
        let now = Instant::now();
        self.samples.push_back((now, point));
        while self.samples.len() > 1 && now.duration_since(self.samples[0].0) > Duration::from_millis(80) {
            self.samples.pop_front();
        }
    }

    fn move_to(&mut self, screens: &mut [Screen], point: V2, now: i64) {
        let Some(id) = self.held else { return };
        let Some(from) = screens.iter().position(|screen| screen.simulation.snowballs().iter().any(|ball| ball.id == id)) else {
            self.held = None;
            return;
        };
        self.sample(point);
        let to = screens
            .iter()
            .position(|screen| {
                let r = screen.monitor.bounds;
                screen.can_grab() && point.x >= r.left as f32 && point.x < r.right as f32 && point.y >= r.top as f32 && point.y < r.bottom as f32
            })
            .unwrap_or(from);
        if from != to {
            let (scale, offset) = screens[from].mapping_to(&screens[to]);
            if let Some(mut ball) = screens[from].simulation.take_snowball(id) {
                ball.map_to(scale, offset);
                screens[to].simulation.bring_snowball(ball);
                screens[from].wake(now);
            }
        }
        let local = screens[to].local(point.x, point.y);
        screens[to].simulation.move_held(local);
        screens[to].wake(now);
    }

    fn release(&mut self, screens: &mut [Screen], now: i64) {
        let Some(id) = self.held.take() else { return };
        let velocity = match (self.samples.front(), self.samples.back()) {
            (Some((start, from)), Some((end, to))) => (*to - *from) / end.duration_since(*start).as_secs_f32().max(0.01),
            _ => V2::ZERO,
        };
        for screen in screens {
            if screen.simulation.snowballs().iter().any(|ball| ball.id == id) {
                let scale = screen.scale();
                screen.simulation.release(V2::new((velocity.x * scale.0).clamp(-1500.0, 1500.0), (-velocity.y * scale.1).clamp(-1500.0, 1500.0)));
                screen.wake(now);
                break;
            }
        }
        self.samples.clear();
    }

    /// Geometry and capture affinity are cached: resting balls cause no window updates.
    pub fn sync(&mut self, screens: &mut [Screen], settings: &Settings, running: bool, now: i64) {
        let enabled = running && settings.snowballs && !settings.behind_windows;
        if self.held.is_some_and(|id| {
            !enabled || !screens.iter().any(|screen| screen.can_grab() && screen.simulation.snowballs().iter().any(|ball| ball.id == id))
        }) {
            self.release(screens, now);
            unsafe {
                let _ = ReleaseCapture();
            }
        }
        if !enabled {
            self.windows.clear();
            INPUT.with_borrow_mut(Vec::clear);
            return;
        }
        let mut present = HashSet::new();
        for screen in screens.iter().filter(|screen| screen.can_grab()) {
            for ball in screen.simulation.snowballs() {
                let shape = Shape::new(screen, ball);
                present.insert(ball.id);
                if !self.registered {
                    match register() {
                        Ok(()) => self.registered = true,
                        Err(error) => {
                            log!("Snowball input unavailable: {error}");
                            return;
                        }
                    }
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = self.windows.entry(ball.id) {
                    match HitWindow::new(ball.id, &shape, settings.show_in_screenshots, screen.overlay_window()) {
                        Ok(window) => {
                            entry.insert(window);
                        }
                        Err(error) => {
                            log!("Snowball input failed: {error}");
                            continue;
                        }
                    }
                }
                if let Some(window) = self.windows.get_mut(&ball.id) {
                    if let Err(error) = window.update(shape, settings.show_in_screenshots, screen.overlay_window()) {
                        log!("Snowball input update failed: {error}");
                        // A stale hit target must never remain over another app.
                        present.remove(&ball.id);
                    }
                }
            }
        }
        self.windows.retain(|id, _| present.contains(id));
    }
}

#[derive(Clone, PartialEq)]
struct Shape {
    rect: RECT,
    clip: RECT,
    blockers: Vec<RECT>,
}

impl Shape {
    fn new(screen: &Screen, ball: &Snowball) -> Self {
        let (sx, sy) = screen.scale();
        let area = screen.monitor.rect;
        let pixels = |left: f32, bottom: f32, right: f32, top: f32| RECT {
            left: area.left + (left / sx).floor() as i32,
            top: area.bottom - (top / sy).ceil() as i32,
            right: area.left + (right / sx).ceil() as i32,
            bottom: area.bottom - (bottom / sy).floor() as i32,
        };
        let r = ball.radius;
        let rect = pixels(ball.center.x - r, ball.center.y - r, ball.center.x + r, ball.center.y + r);
        let relative = |r: RECT| RECT { left: r.left - rect.left, top: r.top - rect.top, right: r.right - rect.left, bottom: r.bottom - rect.top };
        Self {
            rect,
            clip: relative(area),
            blockers: screen.simulation.snowball_occluders(ball).iter().map(|b| relative(pixels(b[0], b[1], b[2], b[3]))).collect(),
        }
    }
}

struct HitWindow {
    window: HWND,
    shape: Shape,
    shown_in_screenshots: bool,
}

impl HitWindow {
    fn new(id: SnowballId, shape: &Shape, shown: bool, overlay: HWND) -> Result<Self> {
        let r = shape.rect;
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                CLASS,
                w!("Snowfall Snowball"),
                WS_POPUP,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        let result = Self { window, shape: shape.clone(), shown_in_screenshots: shown };
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, id as isize);
            SetLayeredWindowAttributes(window, COLORREF(0), 1, LWA_ALPHA)?;
        }
        result.affinity(shown)?;
        result.region(shape)?;
        result.stack(overlay)?;
        unsafe {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        Ok(result)
    }

    fn update(&mut self, shape: Shape, shown: bool, overlay: HWND) -> Result<()> {
        if shape != self.shape {
            self.region(&shape)?;
            if shape.rect != self.shape.rect {
                let r = shape.rect;
                unsafe {
                    SetWindowPos(self.window, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE | SWP_NOZORDER)?;
                }
            }
            self.shape = shape;
        }
        if shown != self.shown_in_screenshots {
            self.affinity(shown)?;
            self.shown_in_screenshots = shown;
        }
        self.stack(overlay)?;
        Ok(())
    }

    /// Stay immediately above the overlay, which is itself kept below menus. A still stack needs no SetWindowPos.
    fn stack(&self, overlay: HWND) -> Result<()> {
        unsafe {
            let above = GetWindow(overlay, GW_HWNDPREV).unwrap_or(HWND_TOPMOST);
            if above != self.window {
                // There can be other ball windows between this one and the overlay; their order doesn't matter.
                let mut next = GetWindow(self.window, GW_HWNDNEXT).ok();
                while let Some(window) = next {
                    if window == overlay {
                        return Ok(());
                    }
                    if crate::tracker::class_name(window) != "SnowfallSnowball" {
                        break;
                    }
                    next = GetWindow(window, GW_HWNDNEXT).ok();
                }
                SetWindowPos(self.window, Some(above), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?;
            }
        }
        Ok(())
    }

    fn affinity(&self, shown: bool) -> Result<()> {
        unsafe { SetWindowDisplayAffinity(self.window, if shown { WDA_NONE } else { WDA_EXCLUDEFROMCAPTURE }) }
    }

    fn region(&self, shape: &Shape) -> Result<()> {
        unsafe {
            let r = shape.rect;
            let region = CreateEllipticRgn(0, 0, r.right - r.left, r.bottom - r.top);
            if region.is_invalid() {
                return Err(Error::from_thread());
            }
            for (r, mode) in std::iter::once((&shape.clip, RGN_AND)).chain(shape.blockers.iter().map(|r| (r, RGN_DIFF))) {
                let cut = CreateRectRgn(r.left, r.top, r.right, r.bottom);
                let result = CombineRgn(Some(region), Some(region), Some(cut), mode);
                let _ = DeleteObject(cut.into());
                if result.0 == 0 {
                    let _ = DeleteObject(region.into());
                    return Err(Error::from_thread());
                }
            }
            // Windows takes ownership only on success.
            if SetWindowRgn(self.window, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
                return Err(Error::from_thread());
            }
        }
        Ok(())
    }
}

impl Drop for HitWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

fn register() -> Result<()> {
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: GetModuleHandleW(None)?.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_HAND)?,
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(Error::from_thread());
        }
    }
    Ok(())
}

fn pointer() -> Option<V2> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some(V2::new(point.x as f32, point.y as f32))
}

fn message_point(window: HWND, lparam: LPARAM) -> Option<V2> {
    let mut point = POINT { x: (lparam.0 & 0xffff) as i16 as i32, y: ((lparam.0 >> 16) & 0xffff) as i16 as i32 };
    unsafe { ClientToScreen(window, &mut point) }.as_bool().then_some(V2::new(point.x as f32, point.y as f32))
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            unsafe {
                SetCapture(window);
            }
            if let Some(point) = message_point(window, lparam) {
                let id = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as SnowballId;
                enqueue(Input::Press(id, point));
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if unsafe { GetCapture() } == window => {
            if let Some(point) = message_point(window, lparam) {
                enqueue(Input::Move(point));
            }
            LRESULT(0)
        }
        WM_LBUTTONUP if unsafe { GetCapture() } == window => {
            if let Some(point) = message_point(window, lparam) {
                enqueue(Input::Release(point));
            }
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            enqueue(Input::Cancel);
            LRESULT(0)
        }
        WM_CANCELMODE => {
            enqueue(Input::Cancel);
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
