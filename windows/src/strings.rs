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
    pub open_snowfall: &'static str,
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
    pub send_snowplough: &'static str,
    pub send_yeti: &'static str,
    pub behind_windows: &'static str,
    pub keep_windows_clear: &'static str,
    pub snowballs: &'static str,
    pub plough_taskbar: &'static str,
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
    open_snowfall: "Open Snowfall…",
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
    send_snowplough: "Send snowplough",
    send_yeti: "Send yeti",
    behind_windows: "Behind windows",
    keep_windows_clear: "Keep windows clear",
    snowballs: "Snowballs",
    plough_taskbar: "Plough the taskbar",
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
    open_snowfall: "Открыть Snowfall…",
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
    send_snowplough: "Вызвать снегоочиститель",
    send_yeti: "Позвать йети",
    behind_windows: "За окнами",
    keep_windows_clear: "Не засыпать окна",
    snowballs: "Снежки",
    plough_taskbar: "Чистить панель задач",
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

/// Copy for the settings window; setting names and choices still come from the menu.
pub struct WindowStrings {
    pub tabs: [&'static str; 5],
    pub explanations: [&'static str; 13],
    pub full_screen: &'static str,
    pub updates: &'static str,
    pub buttons: [&'static str; 4],
    pub levels: [&'static str; 3],
    pub theme_heading: &'static str,
    pub themes: [&'static str; 3],
    pub theme_hint: &'static str,
    pub snow_opacity: &'static str,
    pub snowplough: &'static str,
    pub primary: &'static str,
    pub clear_hint: &'static str,
    pub plough_hint: &'static str,
    pub yeti: &'static str,
    pub yeti_hint: &'static str,
    pub installed: &'static str,
    pub quit_hint: &'static str,
}

impl Language {
    pub fn window_strings(self) -> &'static WindowStrings {
        match self {
            Language::English => &WINDOW_ENGLISH,
            Language::Russian => &WINDOW_RUSSIAN,
        }
    }
}

const WINDOW_ENGLISH: WindowStrings = WindowStrings {
    tabs: ["Snow", "Windows", "Snow play", "Appearance", "System"],
    explanations: [
        "How much snow falls",
        "Blows the flakes and sweeps snow off edges",
        "How solid the snow looks.",
        "Which monitors get snow.",
        "Snow slowly melts when it hasn't snowed for a while.",
        "Snow only on the wallpaper, windows stay on top.",
        "Snow doesn't build up on window tops.",
        "Snow keeps falling over full-screen games and video.",
        "Otherwise snow stays out of screenshots and screen recordings.",
        "Roll a snowball out of a drift with the mouse, throw it, build snowmen.",
        "Now and then a snowplough clears deep snow.",
        "Snow freezes while the laptop runs on battery.",
        "Snowfall starts with Windows.",
    ],
    full_screen: "Show over full-screen windows",
    updates: "Updates",
    buttons: ["Clear", "Send", "Check", "Call"],
    levels: ["Off", "Snow", "Heavy"],
    theme_heading: "Window theme",
    themes: ["Auto", "Light", "Dark"],
    theme_hint: "Auto follows the Windows theme.",
    snow_opacity: "Snow opacity",
    snowplough: "Snowplough",
    primary: "main",
    clear_hint: "Clear all settled snow now",
    plough_hint: "Drives along the taskbar",
    yeti: "Yeti",
    yeti_hint: "Comes for a Snowman on the taskbar",
    installed: "Installed: {version}",
    quit_hint: "Quit: right-click the tray icon",
};

const WINDOW_RUSSIAN: WindowStrings = WindowStrings {
    tabs: ["Снег", "Окна", "Игра", "Вид", "Система"],
    explanations: [
        "Сколько снега падает",
        "Сносит снежинки и сдувает снег с краёв",
        "Насколько плотно выглядит снег.",
        "На каких мониторах идёт снег.",
        "Снег медленно тает, если давно не шёл.",
        "Снег только на обоях, окна поверх него.",
        "Снег не копится на верхних краях окон.",
        "Снег идёт и над играми и видео во весь экран.",
        "Иначе снег не попадает в скриншоты и запись экрана.",
        "Скатайте снежок из сугроба мышью, бросайте, лепите снеговиков.",
        "Иногда приезжает снегоочиститель и убирает глубокий снег.",
        "Снег замирает, когда ноутбук работает от батареи.",
        "Snowfall стартует вместе с Windows.",
    ],
    full_screen: "Поверх полноэкранных окон",
    updates: "Обновления",
    buttons: ["Убрать", "Вызвать", "Проверить", "Позвать"],
    levels: ["Нет", "Снег", "Сильный"],
    theme_heading: "Тема окна",
    themes: ["Авто", "Светлая", "Тёмная"],
    theme_hint: "«Авто» следует теме Windows.",
    snow_opacity: "Непрозрачность снега",
    snowplough: "Снегоочиститель",
    primary: "основной",
    clear_hint: "Сразу очистить весь лежащий снег",
    plough_hint: "Проедет по панели задач",
    yeti: "Йети",
    yeti_hint: "Придёт за снеговиком на панели задач",
    installed: "Установлена версия {version}",
    quit_hint: "Выход — правый клик по значку",
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
