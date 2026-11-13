//! Every setting the person can change, with its default, and how it is kept between runs.
//!
//! Free of Windows: the registry is one [`Store`], and tests use another.

use std::collections::BTreeSet;
#[cfg(test)]
use std::collections::HashMap;

/// How much snow falls: the engine's own levels.
pub use snowfall_core::simulation::Snowfall as Level;

/// Which way, and how hard, the wind blows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wind {
    None,
    LightLeft,
    LightRight,
    StrongLeft,
    StrongRight,
}

impl Wind {
    pub const ALL: [Wind; 5] = [Wind::None, Wind::LightLeft, Wind::LightRight, Wind::StrongLeft, Wind::StrongRight];
}

/// How opaque falling snow is, in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opacity(u8);

impl Opacity {
    pub const ALL: [Opacity; 5] = [Opacity(100), Opacity(80), Opacity(60), Opacity(40), Opacity(20)];

    pub fn percent(self) -> u8 {
        self.0
    }
}

/// The settings, as the tray menu shows them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub level: Level,
    pub wind: Wind,
    /// The screens the person turned snow off on, by their stable identity. Every other screen gets snow.
    pub screens_off: BTreeSet<String>,
    pub opacity: Opacity,
    pub melt: bool,
    pub keep_windows_clear: bool,
    pub behind_windows: bool,
    pub pause_on_battery: bool,
    pub show_over_full_screen: bool,
    pub show_in_screenshots: bool,
    pub open_at_login: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            level: Level::Snow,
            wind: Wind::None,
            screens_off: BTreeSet::new(),
            opacity: Opacity(100),
            melt: true,
            keep_windows_clear: false,
            behind_windows: false,
            pause_on_battery: false,
            show_over_full_screen: false,
            show_in_screenshots: false,
            open_at_login: true,
        }
    }
}

/// One change the person asks for from the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Level(Level),
    Wind(Wind),
    /// Turns snow on one screen, by its stable identity, on or off.
    Screen(String, bool),
    Opacity(Opacity),
    Melt(bool),
    KeepWindowsClear(bool),
    BehindWindows(bool),
    PauseOnBattery(bool),
    ShowOverFullScreen(bool),
    ShowInScreenshots(bool),
    OpenAtLogin(bool),
}

impl Settings {
    pub fn apply(&mut self, change: &Change) {
        match *change {
            Change::Level(level) => self.level = level,
            Change::Wind(wind) => self.wind = wind,
            Change::Screen(ref screen, true) => {
                self.screens_off.remove(screen);
            }
            Change::Screen(ref screen, false) => {
                self.screens_off.insert(screen.clone());
            }
            Change::Opacity(opacity) => self.opacity = opacity,
            Change::Melt(on) => self.melt = on,
            Change::KeepWindowsClear(on) => self.keep_windows_clear = on,
            Change::BehindWindows(on) => self.behind_windows = on,
            Change::PauseOnBattery(on) => self.pause_on_battery = on,
            Change::ShowOverFullScreen(on) => self.show_over_full_screen = on,
            Change::ShowInScreenshots(on) => self.show_in_screenshots = on,
            Change::OpenAtLogin(on) => self.open_at_login = on,
        }
    }

    pub fn screen_on(&self, screen: &str) -> bool {
        !self.screens_off.contains(screen)
    }

    /// The settings kept in `store`, with defaults for anything missing or unreadable.
    pub fn load(store: &impl Store) -> Settings {
        let defaults = Settings::default();
        let flag = |name: &str, default: bool| store.number(name).map_or(default, |value| value != 0);
        Settings {
            level: store.number("level").and_then(|i| Level::ALL.get(i as usize).copied()).unwrap_or(defaults.level),
            wind: store.number("wind").and_then(|i| Wind::ALL.get(i as usize).copied()).unwrap_or(defaults.wind),
            screens_off: store
                .text("screensOff")
                .map(|text| text.split('\n').filter(|screen| !screen.is_empty()).map(str::to_owned).collect())
                .unwrap_or(defaults.screens_off),
            opacity: store.number("opacity").and_then(|percent| Opacity::ALL.into_iter().find(|o| o.0 as u32 == percent)).unwrap_or(defaults.opacity),
            melt: flag("melt", defaults.melt),
            keep_windows_clear: flag("keepWindowsClear", defaults.keep_windows_clear),
            behind_windows: flag("behindWindows", defaults.behind_windows),
            pause_on_battery: flag("pauseOnBattery", defaults.pause_on_battery),
            show_over_full_screen: flag("showOverFullScreen", defaults.show_over_full_screen),
            show_in_screenshots: flag("showInScreenshots", defaults.show_in_screenshots),
            open_at_login: flag("openAtLogin", defaults.open_at_login),
        }
    }

    pub fn save(&self, store: &mut impl Store) {
        let index = |position: Option<usize>| position.unwrap_or(0) as u32;
        store.set_number("level", index(Level::ALL.iter().position(|&level| level == self.level)));
        store.set_number("wind", index(Wind::ALL.iter().position(|&wind| wind == self.wind)));
        store.set_text("screensOff", &self.screens_off.iter().cloned().collect::<Vec<_>>().join("\n"));
        store.set_number("opacity", self.opacity.0 as u32);
        store.set_number("melt", self.melt as u32);
        store.set_number("keepWindowsClear", self.keep_windows_clear as u32);
        store.set_number("behindWindows", self.behind_windows as u32);
        store.set_number("pauseOnBattery", self.pause_on_battery as u32);
        store.set_number("showOverFullScreen", self.show_over_full_screen as u32);
        store.set_number("showInScreenshots", self.show_in_screenshots as u32);
        store.set_number("openAtLogin", self.open_at_login as u32);
    }
}

/// Where settings are kept between runs: named numbers and texts.
pub trait Store {
    fn number(&self, name: &str) -> Option<u32>;
    fn set_number(&mut self, name: &str, value: u32);
    fn text(&self, name: &str) -> Option<String>;
    fn set_text(&mut self, name: &str, value: &str);
}

/// A store that forgets everything when dropped, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    numbers: HashMap<String, u32>,
    texts: HashMap<String, String>,
}

#[cfg(test)]
impl Store for MemoryStore {
    fn number(&self, name: &str) -> Option<u32> {
        self.numbers.get(name).copied()
    }

    fn set_number(&mut self, name: &str, value: u32) {
        self.numbers.insert(name.to_owned(), value);
    }

    fn text(&self, name: &str) -> Option<String> {
        self.texts.get(name).cloned()
    }

    fn set_text(&mut self, name: &str, value: &str) {
        self.texts.insert(name.to_owned(), value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let settings = Settings::default();
        assert_eq!(settings.level, Level::Snow);
        assert_eq!(settings.wind, Wind::None);
        assert!(settings.screens_off.is_empty(), "every screen gets snow");
        assert_eq!(settings.opacity.percent(), 100);
        assert!(settings.melt);
        assert!(!settings.keep_windows_clear);
        assert!(!settings.behind_windows);
        assert!(!settings.pause_on_battery);
        assert!(!settings.show_over_full_screen);
        assert!(!settings.show_in_screenshots);
        assert!(settings.open_at_login);
    }

    #[test]
    fn an_empty_store_gives_the_defaults() {
        assert_eq!(Settings::load(&MemoryStore::default()), Settings::default());
    }

    #[test]
    fn every_setting_survives_a_save_and_load() {
        let changed = Settings {
            level: Level::HeavySnow,
            wind: Wind::StrongLeft,
            screens_off: ["\\\\?\\DISPLAY#A".to_owned(), "\\\\?\\DISPLAY#B".to_owned()].into(),
            opacity: Opacity(40),
            melt: false,
            keep_windows_clear: true,
            behind_windows: true,
            pause_on_battery: true,
            show_over_full_screen: true,
            show_in_screenshots: true,
            open_at_login: false,
        };
        let mut store = MemoryStore::default();
        changed.save(&mut store);
        assert_eq!(Settings::load(&store), changed);
    }

    #[test]
    fn unreadable_values_fall_back_to_defaults() {
        let mut store = MemoryStore::default();
        store.set_number("level", 9);
        store.set_number("wind", 77);
        store.set_number("opacity", 55);
        let settings = Settings::load(&store);
        assert_eq!(settings.level, Level::Snow);
        assert_eq!(settings.wind, Wind::None);
        assert_eq!(settings.opacity.percent(), 100);
    }

    #[test]
    fn turning_a_screen_off_and_on_again() {
        let mut settings = Settings::default();
        settings.apply(&Change::Screen("A".into(), false));
        assert!(!settings.screen_on("A"));
        assert!(settings.screen_on("B"));
        settings.apply(&Change::Screen("A".into(), true));
        assert!(settings.screen_on("A"));
    }
}
