//! The words the person reads, in English and Russian.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    /// The language for a Windows UI language identifier (LANGID): Russian for any Russian locale, English otherwise.
    pub fn from_langid(langid: u16) -> Language {
        const LANG_RUSSIAN: u16 = 0x19;
        if langid & 0x3ff == LANG_RUSSIAN { Language::Russian } else { Language::English }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Language::English => &ENGLISH,
            Language::Russian => &RUSSIAN,
        }
    }
}

pub struct Strings {
    pub snowfall: &'static str,
    pub off: &'static str,
    pub snow: &'static str,
    pub heavy_snow: &'static str,
    pub wind: &'static str,
    pub no_wind: &'static str,
    pub light_left: &'static str,
    pub light_right: &'static str,
    pub strong_left: &'static str,
    pub strong_right: &'static str,
    pub screens: &'static str,
    /// "Screen {n}", for a screen without a friendly name.
    pub screen: &'static str,
    pub primary: &'static str,
    pub opacity: &'static str,
    pub clear_snow: &'static str,
    pub behind_windows: &'static str,
    pub keep_windows_clear: &'static str,
    pub melt: &'static str,
    pub pause_on_battery: &'static str,
    pub show_over_full_screen: &'static str,
    pub show_in_screenshots: &'static str,
    /// Shown instead of "Show in screenshots" while the snow is behind windows, where Windows can't hide it from capture.
    pub always_in_screenshots: &'static str,
    pub open_at_login: &'static str,
    /// "Snowfall {version}: check for updates".
    pub check_for_updates: &'static str,
    pub quit: &'static str,
    /// Shown once, on the very first run.
    pub first_run: &'static str,
    /// "Snowfall {version} is available", with a click to its page.
    pub update_available: &'static str,
    /// After a check from the menu found nothing newer.
    pub up_to_date: &'static str,
    /// After a check from the menu couldn't reach GitHub.
    pub update_failed: &'static str,
}

pub const ENGLISH: Strings = Strings {
    snowfall: "Snowfall",
    off: "Off",
    snow: "Snow",
    heavy_snow: "Heavy snow",
    wind: "Wind",
    no_wind: "None",
    light_left: "Light ←",
    light_right: "Light →",
    strong_left: "Strong ←",
    strong_right: "Strong →",
    screens: "Screens",
    screen: "Screen {n}",
    primary: "(primary)",
    opacity: "Opacity",
    clear_snow: "Clear snow",
    behind_windows: "Behind windows",
    keep_windows_clear: "Keep windows clear",
    melt: "Melt",
    pause_on_battery: "Pause on battery",
    show_over_full_screen: "Show over full screen",
    show_in_screenshots: "Show in screenshots",
    always_in_screenshots: "Show in screenshots (always, behind windows)",
    open_at_login: "Open at login",
    check_for_updates: "Snowfall {version}: check for updates",
    quit: "Quit",
    first_run: "Snowfall is running — settings are in the snowflake icon.",
    update_available: "Snowfall {version} is available. Click to download it.",
    up_to_date: "You have the latest Snowfall.",
    update_failed: "Couldn't reach GitHub to check for updates.",
};

pub const RUSSIAN: Strings = Strings {
    snowfall: "Снегопад",
    off: "Выключен",
    snow: "Снег",
    heavy_snow: "Сильный снег",
    wind: "Ветер",
    no_wind: "Нет",
    light_left: "Слабый ←",
    light_right: "Слабый →",
    strong_left: "Сильный ←",
    strong_right: "Сильный →",
    screens: "Экраны",
    screen: "Экран {n}",
    primary: "(основной)",
    opacity: "Непрозрачность",
    clear_snow: "Убрать снег",
    behind_windows: "За окнами",
    keep_windows_clear: "Не засыпать окна",
    melt: "Таяние",
    pause_on_battery: "Пауза от батареи",
    show_over_full_screen: "Поверх полноэкранных окон",
    show_in_screenshots: "Показывать на скриншотах",
    always_in_screenshots: "Показывать на скриншотах (всегда, за окнами)",
    open_at_login: "Запускать при входе",
    check_for_updates: "Snowfall {version}: проверить обновления",
    quit: "Выход",
    first_run: "Snowfall запущен — настройки в значке снежинки.",
    update_available: "Доступен Snowfall {version}. Нажмите, чтобы скачать.",
    up_to_date: "У вас последняя версия Snowfall.",
    update_failed: "Не удалось связаться с GitHub, чтобы проверить обновления.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_locales_get_russian_and_everything_else_english() {
        assert_eq!(Language::from_langid(0x0419), Language::Russian, "ru-RU");
        assert_eq!(Language::from_langid(0x0819), Language::Russian, "ru-MD");
        assert_eq!(Language::from_langid(0x0409), Language::English, "en-US");
        assert_eq!(Language::from_langid(0x0422), Language::English, "uk-UA falls back to English");
        assert_eq!(Language::from_langid(0), Language::English);
    }
}
