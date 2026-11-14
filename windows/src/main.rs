#![windows_subsystem = "windows"]

mod app;
mod carry;
mod desktop;
mod light;
mod log;
mod menu;
mod power;
mod registry;
mod render;
mod screen;
mod settings;
mod snow;
mod strings;
mod tracker;
mod update;

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::core::w;

fn main() {
    unsafe {
        let _instance = CreateMutexW(None, true, w!("Local\\Snowfall"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    log::start();
    log!("Snowfall {}", env!("CARGO_PKG_VERSION"));
    if let Err(error) = app::run() {
        log!("failed: {error}");
    }
}
