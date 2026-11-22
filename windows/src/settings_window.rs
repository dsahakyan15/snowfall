//! Settings as data: menu parity, tab layout and input rules stay independent of Win32.

use crate::menu::{self, Action, Context, Item};
use crate::settings::{Change, Settings, WindowTheme};

pub const WIDTH: f32 = 560.0;
pub const HEIGHT: f32 = 480.0;
pub const TOOLBAR: f32 = 110.0;
pub const CONTENT: Rect = Rect::new(0.0, TOOLBAR, WIDTH, HEIGHT - TOOLBAR);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum Tab {
    #[default]
    Snow,
    Windows,
    Play,
    Appearance,
    System,
}
impl Tab {
    pub const ALL: [Self; 5] = [Self::Snow, Self::Windows, Self::Play, Self::Appearance, Self::System];
    pub fn next(self, backwards: bool) -> Self {
        Self::ALL[(self as usize + if backwards { 4 } else { 1 }) % 5]
    }
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub label: String,
    pub action: Action,
    pub checked: bool,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Toggle,
    Segments,
    Checkbox,
    Popup,
    Button,
    Themes,
}
pub struct Row {
    pub label: String,
    pub caption: String,
    pub kind: Kind,
    pub choices: Vec<Choice>,
    pub trailing: &'static str,
}
pub struct Group {
    pub heading: &'static str,
    pub rows: Vec<Row>,
    pub caption: &'static str,
}
pub struct Page {
    pub title: &'static str,
    pub groups: Vec<Group>,
    pub footer: &'static str,
}
pub struct View {
    pub pages: Vec<Page>,
    pub theme: WindowTheme,
}

/// Copy values, enabled states and actions from the menu, including its forced capture state.
pub fn build(settings: &Settings, context: &Context) -> View {
    let items = menu::build(settings, context);
    let text = context.language.strings();
    let copy = context.language.window_strings();
    let submenu = |label: &str, explanation: usize, kind| {
        let items = items
            .iter()
            .find_map(|item| match item {
                Item::Submenu { label: name, items, .. } if name == label => Some(items),
                _ => None,
            })
            .expect("setting submenu");
        Row {
            label: label.into(),
            caption: copy.explanations[explanation].into(),
            kind,
            choices: items.iter().filter_map(choice).collect(),
            trailing: "",
        }
    };
    let toggle = |change: Change, explanation: usize| {
        let c = items
            .iter()
            .filter_map(choice)
            .find(|c| {
                matches!(&c.action, Action::Change(value)
            if std::mem::discriminant(value) == std::mem::discriminant(&change))
            })
            .expect("setting toggle");
        Row { label: c.label.clone(), caption: copy.explanations[explanation].into(), kind: Kind::Toggle, choices: vec![c], trailing: "" }
    };
    // Keeping actions as rows makes adding another visitor a single entry.
    let action = |action, label: &str, caption: String, button: usize| {
        let mut c = items.iter().filter_map(choice).find(|c| c.action == action).expect("menu action");
        c.label = copy.buttons[button].into();
        Row { label: label.into(), caption, kind: Kind::Button, choices: vec![c], trailing: "" }
    };
    let group = |rows| Group { heading: "", rows, caption: "" };
    let mut snowfall = submenu(text.snowfall, 0, Kind::Segments);
    for (choice, label) in snowfall.choices.iter_mut().zip(copy.levels) {
        choice.label = label.into();
    }
    let screens = submenu(text.screens, 3, Kind::Checkbox)
        .choices
        .into_iter()
        .zip(context.screens)
        .map(|(mut c, screen)| {
            // Primary status is a separate, muted column rather than part of the screen name.
            if screen.primary {
                c.label = c.label.trim_end_matches(text.primary).trim_end().into();
            }
            Row {
                label: c.label.clone(),
                caption: String::new(),
                kind: Kind::Checkbox,
                choices: vec![c],
                trailing: if screen.primary { copy.primary } else { "" },
            }
        })
        .collect();
    let mut full_screen = toggle(Change::ShowOverFullScreen(false), 7);
    full_screen.label = copy.full_screen.into();
    let mut capture = toggle(Change::ShowInScreenshots(false), 8);
    capture.label = text.show_in_screenshots.into();
    let mut opacity = submenu(text.opacity, 2, Kind::Popup);
    opacity.label = copy.snow_opacity.into();
    opacity.caption.clear();
    let theme = Row {
        label: copy.theme_heading.into(),
        caption: String::new(),
        kind: Kind::Themes,
        trailing: "",
        choices: WindowTheme::ALL
            .into_iter()
            .zip(copy.themes)
            .map(|(theme, label)| Choice {
                label: label.into(),
                action: Action::Change(Change::WindowTheme(theme)),
                checked: settings.window_theme == theme,
                enabled: true,
            })
            .collect(),
    };
    View {
        theme: settings.window_theme,
        pages: vec![
            Page {
                title: copy.tabs[0],
                footer: "",
                groups: vec![
                    group(vec![snowfall, submenu(text.wind, 1, Kind::Popup), toggle(Change::Melt(false), 4)]),
                    Group { heading: text.screens, rows: screens, caption: "" },
                    group(vec![action(Action::ClearSnow, text.clear_snow, copy.clear_hint.into(), 0)]),
                ],
            },
            Page {
                title: copy.tabs[1],
                footer: "",
                groups: vec![group(vec![toggle(Change::BehindWindows(false), 5), toggle(Change::KeepWindowsClear(false), 6), full_screen, capture])],
            },
            Page {
                title: copy.tabs[2],
                footer: "",
                groups: vec![
                    group(vec![toggle(Change::Snowballs(false), 9), toggle(Change::PloughTaskbar(false), 10)]),
                    group(vec![
                        action(Action::SendSnowplough, copy.snowplough, copy.plough_hint.into(), 1),
                        action(Action::SendYeti, copy.yeti, copy.yeti_hint.into(), 3),
                    ]),
                ],
            },
            Page {
                title: copy.tabs[3],
                footer: "",
                groups: vec![Group { heading: copy.theme_heading, rows: vec![theme], caption: copy.theme_hint }, group(vec![opacity])],
            },
            Page {
                title: copy.tabs[4],
                footer: copy.quit_hint,
                groups: vec![
                    group(vec![toggle(Change::OpenAtLogin(false), 12), toggle(Change::PauseOnBattery(false), 11)]),
                    group(vec![action(Action::CheckForUpdates, copy.updates, copy.installed.replace("{version}", context.version), 2)]),
                ],
            },
        ],
    }
}
fn choice(item: &Item) -> Option<Choice> {
    match item {
        Item::Command { label, action, checked, enabled, .. } => {
            Some(Choice { label: label.clone(), action: action.clone(), checked: *checked, enabled: *enabled })
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    pub fn inset(self, amount: f32) -> Self {
        Self::new(self.x + amount, self.y + amount, self.width - 2.0 * amount, self.height - 2.0 * amount)
    }
}
pub fn light(index: usize) -> Rect {
    Rect::new(14.0 + index as f32 * 20.0, 13.0, 12.0, 12.0)
}
/// Close and minimize only: a zoom button would do nothing in a fixed-size window.
pub const LIGHT_COUNT: usize = 2;
pub const LIGHTS: Rect = Rect::new(8.0, 6.0, 56.0, 26.0);
pub fn tab_bounds(tab: Tab) -> Rect {
    Rect::new(96.0 + tab as usize as f32 * 76.0, 37.0, 64.0, 62.0)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Light(usize),
    Tab(Tab),
    Control(usize, usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Tabs,
    Control(usize),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation {
    Action(Action),
    Popup(usize),
}
pub struct PlacedGroup {
    pub group: usize,
    pub bounds: Rect,
}
pub struct PlacedRow {
    pub group: usize,
    pub row: usize,
    pub bounds: Rect,
}
pub struct Control {
    pub group: usize,
    pub row: usize,
    pub bounds: Rect,
    pub choices: Vec<Rect>,
}
impl Control {
    pub fn row<'a>(&self, view: &'a View, tab: Tab) -> &'a Row {
        &view.pages[tab as usize].groups[self.group].rows[self.row]
    }
    pub fn enabled(&self, view: &View, tab: Tab) -> bool {
        self.row(view, tab).choices.iter().any(|c| c.enabled)
    }
}
pub struct Layout {
    pub tab: Tab,
    pub groups: Vec<PlacedGroup>,
    pub rows: Vec<PlacedRow>,
    pub controls: Vec<Control>,
    pub bottom: f32,
    pub scroll: f32,
}
impl Layout {
    pub fn new(view: &View, tab: Tab, scroll: f32) -> Self {
        let mut layout = Self { tab, groups: vec![], rows: vec![], controls: vec![], bottom: 0.0, scroll: 0.0 };
        let mut y = TOOLBAR + 16.0;
        for (g, group) in view.pages[tab as usize].groups.iter().enumerate() {
            if !group.heading.is_empty() {
                y += 24.0;
            }
            let top = y;
            for (r, row) in group.rows.iter().enumerate() {
                let height = match row.kind {
                    Kind::Themes => 112.0,
                    Kind::Checkbox => 30.0,
                    _ if row.caption.is_empty() => 42.0,
                    _ => 52.0,
                };
                let bounds = Rect::new(22.0, y, WIDTH - 44.0, height);
                layout.rows.push(PlacedRow { group: g, row: r, bounds });
                let control = match row.kind {
                    Kind::Toggle => Rect::new(WIDTH - 68.0, y + (height - 19.0) / 2.0, 32.0, 19.0),
                    Kind::Segments => Rect::new(WIDTH - 228.0, y + (height - 25.0) / 2.0, 192.0, 25.0),
                    Kind::Popup => Rect::new(WIDTH - 162.0, y + (height - 25.0) / 2.0, 126.0, 25.0),
                    Kind::Button => Rect::new(WIDTH - 128.0, y + (height - 25.0) / 2.0, 92.0, 25.0),
                    Kind::Checkbox => bounds.inset(4.0),
                    Kind::Themes => Rect::new((WIDTH - 270.0) / 2.0, y + 14.0, 270.0, 84.0),
                };
                let choices = match row.kind {
                    Kind::Segments => {
                        (0..row.choices.len()).map(|i| Rect::new(control.x + i as f32 * 64.0, control.y, 64.0, control.height)).collect()
                    }
                    Kind::Themes => (0..3).map(|i| Rect::new(control.x + i as f32 * 96.0, control.y, 78.0, control.height)).collect(),
                    _ => vec![control],
                };
                layout.controls.push(Control { group: g, row: r, bounds: control, choices });
                y += height;
            }
            if y > top {
                layout.groups.push(PlacedGroup { group: g, bounds: Rect::new(22.0, top, WIDTH - 44.0, y - top) });
            }
            if !group.caption.is_empty() {
                y += 27.0;
            }
            y += 14.0;
        }
        layout.bottom = y + if view.pages[tab as usize].footer.is_empty() { 0.0 } else { 24.0 };
        layout.scroll = scroll.clamp(0.0, (layout.bottom - HEIGHT).max(0.0));
        for group in &mut layout.groups {
            group.bounds.y -= layout.scroll;
        }
        for row in &mut layout.rows {
            row.bounds.y -= layout.scroll;
        }
        for control in &mut layout.controls {
            control.bounds.y -= layout.scroll;
            for r in &mut control.choices {
                r.y -= layout.scroll;
            }
        }
        layout
    }
    pub fn hit(&self, view: &View, x: f32, y: f32) -> Option<Hit> {
        for i in 0..LIGHT_COUNT {
            if light(i).contains(x, y) {
                return Some(Hit::Light(i));
            }
        }
        for tab in Tab::ALL {
            if tab_bounds(tab).contains(x, y) {
                return Some(Hit::Tab(tab));
            }
        }
        if !CONTENT.contains(x, y) {
            return None;
        }
        self.controls.iter().enumerate().find_map(|(i, c)| {
            if !c.enabled(view, self.tab) {
                return None;
            }
            c.choices.iter().position(|r| r.contains(x, y)).map(|j| Hit::Control(i, j))
        })
    }
    pub fn caption(&self, x: f32, y: f32) -> bool {
        (0.0..WIDTH).contains(&x) && (0.0..TOOLBAR).contains(&y) && !LIGHTS.contains(x, y) && !Tab::ALL.iter().any(|&t| tab_bounds(t).contains(x, y))
    }
    pub fn focus_next(&self, view: &View, current: Option<Focus>, backwards: bool) -> Focus {
        let stops: Vec<_> = std::iter::once(Focus::Tabs)
            .chain(self.controls.iter().enumerate().filter(|(_, c)| c.enabled(view, self.tab)).map(|(i, _)| Focus::Control(i)))
            .collect();
        let index = current.and_then(|c| stops.iter().position(|&s| s == c));
        stops[match (index, backwards) {
            (Some(i), true) => (i + stops.len() - 1) % stops.len(),
            (Some(i), false) => (i + 1) % stops.len(),
            (None, true) => stops.len() - 1,
            (None, false) => 0,
        }]
    }
    pub fn activate(&self, view: &View, index: usize, choice: Option<usize>, arrow: i32) -> Option<Activation> {
        let c = self.controls.get(index)?;
        let row = c.row(view, self.tab);
        if !c.enabled(view, self.tab) {
            return None;
        }
        if row.kind == Kind::Popup {
            return (arrow == 0).then_some(Activation::Popup(index));
        }
        let index = if matches!(row.kind, Kind::Segments | Kind::Themes) {
            choice.unwrap_or_else(|| {
                (row.choices.iter().position(|c| c.checked).unwrap_or(0) as i32 + arrow).clamp(0, row.choices.len() as i32 - 1) as usize
            })
        } else {
            if arrow != 0 {
                return None;
            }
            0
        };
        let choice = row.choices.get(index)?;
        choice.enabled.then(|| Activation::Action(choice.action.clone()))
    }
    /// Keyboard traversal also reaches screens below the viewport on large multi-monitor setups.
    pub fn scroll_to(&self, focus: Focus) -> f32 {
        let Focus::Control(i) = focus else { return self.scroll };
        let r = self.controls[i].bounds;
        if r.y < TOOLBAR + 8.0 {
            self.scroll + r.y - TOOLBAR - 8.0
        } else if r.y + r.height > HEIGHT - 8.0 {
            self.scroll + r.y + r.height - HEIGHT + 8.0
        } else {
            self.scroll
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::Screen;
    use crate::settings::{Level, Opacity, Wind};
    use crate::strings::Language;
    fn commands(items: &[Item]) -> Vec<Choice> {
        items
            .iter()
            .flat_map(|i| match i {
                Item::Submenu { items, .. } => commands(items),
                _ => choice(i).into_iter().collect(),
            })
            .collect()
    }
    #[test]
    fn every_setting_and_action_agrees_with_the_menu_in_both_languages() {
        let screens = [
            Screen { id: "a".into(), name: Some("DELL".into()), primary: true, number: Some(1) },
            Screen { id: "b".into(), name: None, primary: false, number: Some(3) },
        ];
        for language in [Language::English, Language::Russian] {
            for behind_windows in [false, true] {
                for alternate in [false, true] {
                    let mut settings = Settings { behind_windows, ..Settings::default() };
                    if alternate {
                        for change in [
                            Change::Level(Level::Off),
                            Change::Wind(Wind::StrongRight),
                            Change::Opacity(Opacity::ALL[3]),
                            Change::Screen("b".into(), false),
                            Change::Melt(false),
                            Change::Snowballs(false),
                            Change::PloughTaskbar(false),
                            Change::KeepWindowsClear(true),
                            Change::ShowOverFullScreen(true),
                            Change::ShowInScreenshots(true),
                            Change::OpenAtLogin(false),
                            Change::PauseOnBattery(true),
                        ] {
                            settings.apply(&change);
                        }
                    }
                    let context = Context { language, version: "1.2.3", screens: &screens };
                    let menu = commands(&menu::build(&settings, &context));
                    let view = build(&settings, &context);
                    let choices: Vec<_> = view.pages.iter().flat_map(|p| &p.groups).flat_map(|g| &g.rows).flat_map(|r| &r.choices).collect();
                    for item in menu.iter().filter(|c| !matches!(c.action, Action::Quit | Action::OpenSnowfall)) {
                        let projected = choices.iter().find(|c| c.action == item.action).expect("every menu choice appears");
                        assert_eq!((projected.checked, projected.enabled), (item.checked, item.enabled), "{:?}", item.action);
                    }
                    for c in choices {
                        assert!(menu.iter().any(|m| m.action == c.action) || matches!(c.action, Action::Change(Change::WindowTheme(_))));
                    }
                    for tab in Tab::ALL {
                        let layout = Layout::new(&view, tab, 0.0);
                        for (i, control) in layout.controls.iter().enumerate() {
                            let row = control.row(&view, tab);
                            for (j, choice) in row.choices.iter().enumerate() {
                                let expected = choice
                                    .enabled
                                    .then(|| if row.kind == Kind::Popup { Activation::Popup(i) } else { Activation::Action(choice.action.clone()) });
                                assert_eq!(layout.activate(&view, i, Some(j), 0), expected);
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn tabs_focus_arrows_hit_testing_and_drag_regions() {
        let settings = Settings { behind_windows: true, ..Settings::default() };
        let view = build(&settings, &Context { language: Language::English, version: "1", screens: &[] });
        assert_eq!(Tab::Snow.next(true), Tab::System);
        assert_eq!(Tab::System.next(false), Tab::Snow);
        for tab in Tab::ALL {
            let layout = Layout::new(&view, tab, 0.0);
            for i in 0..LIGHT_COUNT {
                let r = light(i);
                assert_eq!(layout.hit(&view, r.x + 6.0, r.y + 6.0), Some(Hit::Light(i)));
            }
            for t in Tab::ALL {
                let r = tab_bounds(t);
                assert_eq!(layout.hit(&view, r.x + 2.0, r.y + 2.0), Some(Hit::Tab(t)));
                assert!(!layout.caption(r.x + 2.0, r.y + 2.0));
            }
            assert!(layout.caption(90.0, 18.0));
            assert!(layout.caption(540.0, 80.0));
            assert!(!layout.caption(14.0, 13.0));
            assert!(!layout.caption(100.0, 120.0));
            assert_eq!(layout.focus_next(&view, None, false), Focus::Tabs);
            let mut focus = Focus::Tabs;
            let count = layout.controls.iter().filter(|c| c.enabled(&view, tab)).count();
            for _ in 0..count {
                focus = layout.focus_next(&view, Some(focus), false);
                assert!(matches!(focus, Focus::Control(_)));
            }
            assert_eq!(layout.focus_next(&view, Some(focus), false), Focus::Tabs);
            assert_eq!(layout.focus_next(&view, Some(Focus::Tabs), true), focus);
            for (i, c) in layout.controls.iter().enumerate() {
                for (j, r) in c.choices.iter().enumerate() {
                    assert_eq!(layout.hit(&view, r.x + r.width / 2.0, r.y + r.height / 2.0), c.enabled(&view, tab).then_some(Hit::Control(i, j)));
                    if !c.enabled(&view, tab) {
                        assert_eq!(layout.activate(&view, i, None, 0), None);
                    }
                }
            }
        }
        let snow = Layout::new(&view, Tab::Snow, 0.0);
        assert_eq!(snow.activate(&view, 0, None, -1), Some(Activation::Action(Action::Change(Change::Level(Level::Off)))));
        assert_eq!(snow.activate(&view, 0, None, 1), Some(Activation::Action(Action::Change(Change::Level(Level::HeavySnow)))));
        assert_eq!(snow.activate(&view, 1, None, 0), Some(Activation::Popup(1)));
        let appearance = Layout::new(&view, Tab::Appearance, 0.0);
        assert_eq!(appearance.activate(&view, 0, None, -1), Some(Activation::Action(Action::Change(Change::WindowTheme(WindowTheme::Auto)))));
        assert_eq!(appearance.activate(&view, 0, None, 1), Some(Activation::Action(Action::Change(Change::WindowTheme(WindowTheme::Light)))));
        assert_eq!(appearance.activate(&view, 0, Some(2), 0), Some(Activation::Action(Action::Change(Change::WindowTheme(WindowTheme::Dark)))));
    }
    #[test]
    fn localized_copy_and_scrollable_screens_are_complete() {
        for language in [Language::English, Language::Russian] {
            for count in 0..=8 {
                let screens: Vec<_> = (0..count).map(|i| Screen { id: i.to_string(), name: None, primary: i == 0, number: Some(i + 1) }).collect();
                let view = build(&Settings::default(), &Context { language, version: "2.3.4", screens: &screens });
                for tab in Tab::ALL {
                    let page = &view.pages[tab as usize];
                    assert!(!page.title.is_empty());
                    let mut layout = Layout::new(&view, tab, 0.0);
                    for row in page.groups.iter().flat_map(|g| &g.rows) {
                        assert!(!row.label.is_empty());
                        assert!(row.choices.iter().all(|c| !c.label.is_empty()));
                        if !matches!(row.kind, Kind::Checkbox | Kind::Themes | Kind::Popup) {
                            assert!(!row.caption.is_empty());
                        }
                    }
                    for i in 0..layout.controls.len() {
                        layout = Layout::new(&view, tab, layout.scroll_to(Focus::Control(i)));
                        let r = layout.controls[i].bounds;
                        assert!(r.y >= TOOLBAR && r.y + r.height <= HEIGHT);
                        assert!(r.x >= 22.0 && r.x + r.width <= WIDTH - 22.0);
                    }
                }
                assert_eq!(view.pages[0].groups[1].rows.len(), count);
                assert!(view.pages[4].groups[1].rows[0].caption.contains("2.3.4"));
                let copy = language.window_strings();
                assert!(
                    copy.themes
                        .iter()
                        .chain(copy.buttons.iter())
                        .chain([copy.yeti, copy.yeti_hint].iter())
                        .chain(copy.levels.iter())
                        .all(|s| !s.is_empty())
                );
                assert!(!copy.theme_hint.is_empty() && !copy.primary.is_empty());
            }
        }
    }
}
