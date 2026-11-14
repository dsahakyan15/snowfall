//! The tray menu as plain data: a projection of the settings, drawn by the tray and tested without Windows.

use crate::settings::{Change, Level, Opacity, Settings, Wind};
use crate::strings::{Language, Strings};

/// Features whose engine side hasn't landed yet show in the menu greyed out.

/// What choosing a menu item does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Change(Change),
    ClearSnow,
    CheckForUpdates,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Command {
        label: String,
        action: Action,
        checked: bool,
        /// Checked with a dot, as one of a set of choices, rather than a tick.
        radio: bool,
        enabled: bool,
    },
    Submenu {
        label: String,
        items: Vec<Item>,
        enabled: bool,
    },
    Separator,
}

/// A screen as the Screens submenu lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// The stable identity the setting remembers it by.
    pub id: String,
    /// Its friendly name, when it has one.
    pub name: Option<String>,
    pub primary: bool,
    /// The number Windows gives it, for a screen without a name.
    pub number: Option<usize>,
}

pub struct Context<'a> {
    pub language: Language,
    pub version: &'a str,
    pub screens: &'a [Screen],
}

pub fn build(settings: &Settings, context: &Context) -> Vec<Item> {
    let text = context.language.strings();
    let toggle = |label: &str, on: bool, change: fn(bool) -> Change, enabled: bool| Item::Command {
        label: label.to_owned(),
        action: Action::Change(change(!on)),
        checked: on,
        radio: false,
        enabled,
    };
    let behind = settings.behind_windows;
    vec![
        submenu(text.snowfall, true, choices(&Level::ALL, settings.level, |level| level_label(text, level), Change::Level)),
        submenu(text.wind, true, choices(&Wind::ALL, settings.wind, |wind| wind_label(text, wind), Change::Wind)),
        submenu(text.screens, true, screens(settings, context, text)),
        submenu(text.opacity, true, choices(&Opacity::ALL, settings.opacity, |o| format!("{}%", o.percent()), Change::Opacity)),
        command(text.clear_snow, Action::ClearSnow, true),
        Item::Separator,
        toggle(text.behind_windows, behind, Change::BehindWindows, true),
        toggle(text.keep_windows_clear, settings.keep_windows_clear, Change::KeepWindowsClear, true),
        toggle(text.melt, settings.melt, Change::Melt, true),
        toggle(text.pause_on_battery, settings.pause_on_battery, Change::PauseOnBattery, true),
        toggle(text.show_over_full_screen, settings.show_over_full_screen, Change::ShowOverFullScreen, !behind),
        // Behind windows, the snow is in the desktop's own layer, which Windows can't keep out of screenshots.
        if behind {
            Item::Command {
                label: text.always_in_screenshots.to_owned(),
                action: Action::Change(Change::ShowInScreenshots(!settings.show_in_screenshots)),
                checked: true,
                radio: false,
                enabled: false,
            }
        } else {
            toggle(text.show_in_screenshots, settings.show_in_screenshots, Change::ShowInScreenshots, true)
        },
        Item::Separator,
        toggle(text.open_at_login, settings.open_at_login, Change::OpenAtLogin, true),
        command(&text.check_for_updates.replace("{version}", context.version), Action::CheckForUpdates, true),
        command(text.quit, Action::Quit, true),
    ]
}

fn submenu(label: &str, enabled: bool, items: Vec<Item>) -> Item {
    Item::Submenu { label: label.to_owned(), items, enabled }
}

fn command(label: &str, action: Action, enabled: bool) -> Item {
    Item::Command { label: label.to_owned(), action, checked: false, radio: false, enabled }
}

/// One radio item per value, with the current one checked.
fn choices<T: Copy + PartialEq>(all: &[T], current: T, label: impl Fn(T) -> String, change: fn(T) -> Change) -> Vec<Item> {
    all.iter()
        .map(|&value| Item::Command {
            label: label(value),
            action: Action::Change(change(value)),
            checked: value == current,
            radio: true,
            enabled: true,
        })
        .collect()
}

fn screens(settings: &Settings, context: &Context, text: &Strings) -> Vec<Item> {
    context
        .screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let number = screen.number.unwrap_or(i + 1);
            let name = screen.name.clone().unwrap_or_else(|| text.screen.replace("{n}", &number.to_string()));
            let label = if screen.primary { format!("{name} {}", text.primary) } else { name };
            let on = settings.screen_on(&screen.id);
            Item::Command { label, action: Action::Change(Change::Screen(screen.id.clone(), !on)), checked: on, radio: false, enabled: true }
        })
        .collect()
}

fn level_label(text: &Strings, level: Level) -> String {
    match level {
        Level::Off => text.off,
        Level::Snow => text.snow,
        Level::HeavySnow => text.heavy_snow,
    }
    .to_owned()
}

fn wind_label(text: &Strings, wind: Wind) -> String {
    match wind {
        Wind::None => text.no_wind,
        Wind::LightLeft => text.light_left,
        Wind::LightRight => text.light_right,
        Wind::StrongLeft => text.strong_left,
        Wind::StrongRight => text.strong_right,
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(language: Language) -> Context<'static> {
        Context { language, version: "1.2.3", screens: &[] }
    }

    fn labels(items: &[Item]) -> Vec<&str> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Command { label, .. } | Item::Submenu { label, .. } => Some(label.as_str()),
                Item::Separator => None,
            })
            .collect()
    }

    /// The item labelled `label`, anywhere in the menu.
    fn find<'a>(items: &'a [Item], label: &str) -> &'a Item {
        fn search<'a>(items: &'a [Item], label: &str) -> Option<&'a Item> {
            items.iter().find_map(|item| match item {
                Item::Command { label: l, .. } if l == label => Some(item),
                Item::Submenu { label: l, .. } if l == label => Some(item),
                Item::Submenu { items, .. } => search(items, label),
                _ => None,
            })
        }
        search(items, label).unwrap_or_else(|| panic!("no item {label}"))
    }

    fn state(item: &Item) -> (bool, bool) {
        match item {
            Item::Command { checked, enabled, .. } => (*checked, *enabled),
            Item::Submenu { enabled, .. } => (false, *enabled),
            Item::Separator => panic!("a separator has no state"),
        }
    }

    #[test]
    fn the_menu_has_every_m1_item_in_order() {
        let menu = build(&Settings::default(), &context(Language::English));
        assert_eq!(
            labels(&menu),
            [
                "Snowfall",
                "Wind",
                "Screens",
                "Opacity",
                "Clear snow",
                "Behind windows",
                "Keep windows clear",
                "Melt",
                "Pause on battery",
                "Show over full screen",
                "Show in screenshots",
                "Open at login",
                "Snowfall 1.2.3: check for updates",
                "Quit"
            ]
        );
    }

    #[test]
    fn the_menu_is_in_russian_for_a_russian_ui() {
        let menu = build(&Settings::default(), &context(Language::Russian));
        let labels = labels(&menu);
        assert_eq!(labels[0], "Снегопад");
        assert_eq!(labels.last(), Some(&"Выход"));
        assert!(labels.iter().all(|label| !label.contains("Quit")));
    }

    #[test]
    fn defaults_check_snow_no_wind_full_opacity_melt_and_open_at_login() {
        let menu = build(&Settings::default(), &context(Language::English));
        assert!(state(find(&menu, "Snow")).0);
        assert!(!state(find(&menu, "Heavy snow")).0);
        assert!(state(find(&menu, "None")).0);
        assert!(state(find(&menu, "100%")).0);
        assert_eq!(state(find(&menu, "Melt")), (true, true));
        assert_eq!(state(find(&menu, "Open at login")), (true, true));
        assert_eq!(state(find(&menu, "Behind windows")), (false, true));
        assert_eq!(state(find(&menu, "Show in screenshots")), (false, true));
    }

    #[test]
    fn exactly_one_choice_is_checked_in_each_set() {
        let settings = Settings { level: Level::HeavySnow, wind: Wind::StrongRight, ..Settings::default() };
        let menu = build(&settings, &context(Language::English));
        for name in ["Snowfall", "Wind", "Opacity"] {
            let Item::Submenu { items, .. } = find(&menu, name) else { panic!() };
            let checked: Vec<_> = items.iter().filter(|item| state(item).0).collect();
            assert_eq!(checked.len(), 1, "{name}");
        }
        assert!(state(find(&menu, "Heavy snow")).0);
        assert!(state(find(&menu, "Strong →")).0);
    }

    #[test]
    fn choosing_an_item_asks_for_the_opposite_of_its_state() {
        let menu = build(&Settings::default(), &context(Language::English));
        let Item::Command { action, .. } = find(&menu, "Melt") else { panic!() };
        assert_eq!(action, &Action::Change(Change::Melt(false)));
        let Item::Command { action, .. } = find(&menu, "Behind windows") else { panic!() };
        assert_eq!(action, &Action::Change(Change::BehindWindows(true)));
    }

    #[test]
    fn behind_windows_disables_show_over_full_screen_and_always_shows_in_screenshots() {
        let settings = Settings { behind_windows: true, ..Settings::default() };
        let menu = build(&settings, &context(Language::English));
        assert!(!state(find(&menu, "Show over full screen")).1);
        assert_eq!(state(find(&menu, "Show in screenshots (always, behind windows)")), (true, false));
    }

    #[test]
    fn screens_are_listed_by_name_or_number_and_checked_unless_turned_off() {
        let screens = [
            Screen { id: "a".into(), name: Some("DELL U2720Q".into()), primary: true, number: Some(1) },
            // Windows calls it Display 3, though it is second from the left.
            Screen { id: "b".into(), name: None, primary: false, number: Some(3) },
        ];
        let mut settings = Settings::default();
        settings.apply(&Change::Screen("b".into(), false));
        let menu = build(&settings, &Context { language: Language::English, version: "1", screens: &screens });
        assert!(state(find(&menu, "DELL U2720Q (primary)")).0);
        assert!(!state(find(&menu, "Screen 3")).0);
        let Item::Command { action, .. } = find(&menu, "Screen 3") else { panic!() };
        assert_eq!(action, &Action::Change(Change::Screen("b".into(), true)));
    }
}
