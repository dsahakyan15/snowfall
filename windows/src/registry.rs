//! Settings kept in the current user's registry, and the autostart entry.

use std::ffi::c_void;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::log;
use crate::settings::Store;

const KEY: PCWSTR = w!("Software\\Snowfall");
const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("Snowfall");

/// `HKEY_CURRENT_USER\Software\Snowfall`.
pub struct Registry;

impl Store for Registry {
    fn number(&self, name: &str) -> Option<u32> {
        let mut value = 0u32;
        let mut size = 4u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                KEY,
                &HSTRING::from(name),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut value).cast::<c_void>()),
                Some(&mut size),
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    fn set_number(&mut self, name: &str, value: u32) {
        let status =
            unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, KEY, &HSTRING::from(name), REG_DWORD.0, Some((&raw const value).cast::<c_void>()), 4) };
        if status != ERROR_SUCCESS {
            log!("saving {name}: {status:?}");
        }
    }

    fn text(&self, name: &str) -> Option<String> {
        let name = HSTRING::from(name);
        read_text(KEY, PCWSTR(name.as_ptr()))
    }

    fn set_text(&mut self, name: &str, value: &str) {
        let wide = HSTRING::from(name);
        let status = write_text(KEY, PCWSTR(wide.as_ptr()), value);
        if status != ERROR_SUCCESS {
            log!("saving {name}: {status:?}");
        }
    }
}

/// Registers this executable to start at login, or removes it. An entry left by a copy elsewhere is pointed here.
pub fn set_open_at_login(on: bool) {
    let status = if on {
        match command() {
            Some(command) if read_text(RUN, RUN_VALUE).as_deref() == Some(command.as_str()) => ERROR_SUCCESS,
            Some(command) => write_text(RUN, RUN_VALUE, &command),
            None => WIN32_ERROR(1),
        }
    } else {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, RUN_VALUE) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            status => status,
        }
    };
    if status != ERROR_SUCCESS {
        log!("open at login {on}: {status:?}");
    }
}

/// The quoted path of this executable.
fn command() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    Some(format!("\"{}\"", path.display()))
}

fn read_text(key: PCWSTR, name: PCWSTR) -> Option<String> {
    let mut size = 0u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, key, name, RRF_RT_REG_SZ, None, None, Some(&mut size)) != ERROR_SUCCESS {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        if RegGetValueW(HKEY_CURRENT_USER, key, name, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast::<c_void>()), Some(&mut size))
            != ERROR_SUCCESS
        {
            return None;
        }
        let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..length]))
    }
}

fn write_text(key: PCWSTR, name: PCWSTR, value: &str) -> WIN32_ERROR {
    let value: Vec<u16> = value.encode_utf16().chain([0]).collect();
    unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key, name, REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32) }
}
