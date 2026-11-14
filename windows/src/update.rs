//! The daily look for a newer Snowfall on GitHub Releases. Nothing is downloaded or installed, and nothing is sent
//! beyond the request itself.

use std::cmp::Ordering;
use std::ffi::c_void;

use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WinHttpAddRequestHeaders, WinHttpCloseHandle,
    WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};
use windows::core::{PCWSTR, w};

/// GitHub's API for the latest release that isn't a pre-release.
const HOST: PCWSTR = w!("api.github.com");
const PATH: PCWSTR = w!("/repos/dsahakyan15/snowfall/releases/latest");
/// How often to look on its own, in seconds.
pub const INTERVAL: u64 = 24 * 60 * 60;

/// A release newer than this one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// As tagged, without a leading `v`.
    pub version: String,
    /// Its page on GitHub.
    pub url: String,
}

/// Whether it is time to look again, `now` and `last` in seconds since the Unix epoch. A clock set back counts as due.
pub fn due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now < last || now - last >= INTERVAL)
}

/// Orders versions as semantic versioning does: `1.2.10` after `1.2.9`, and a pre-release such as `1.3.0-beta.2`
/// before `1.3.0`. A leading `v` and build metadata after `+` are ignored. Returns `None` for what isn't a version.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let (a, b) = (Version::parse(a)?, Version::parse(b)?);
    Some(a.core.cmp(&b.core).then_with(|| match (&a.pre, &b.pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_pre(a, b),
    }))
}

struct Version<'a> {
    core: [u64; 3],
    pre: Option<Vec<&'a str>>,
}

impl<'a> Version<'a> {
    fn parse(text: &'a str) -> Option<Version<'a>> {
        let text = text.trim();
        let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre.split('.').collect())),
            None => (text, None),
        };
        let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
        let version = [parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?];
        if parts.next().is_some() {
            return None;
        }
        Some(Version { core: version, pre })
    }
}

/// Pre-release identifiers: numbers by value and before words, words alphabetically, and more of them after fewer.
fn compare_pre(a: &[&str], b: &[&str]) -> Ordering {
    for (a, b) in a.iter().zip(b) {
        let order = match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

/// The release in GitHub's answer, if it is newer than `current`.
pub fn newer(answer: &str, current: &str) -> Option<Release> {
    let tag = string_field(answer, "tag_name")?;
    let url = string_field(answer, "html_url")?;
    if compare(&tag, current)? != Ordering::Greater || !url.starts_with("https://github.com/") {
        return None;
    }
    Some(Release { version: tag.trim_start_matches(['v', 'V']).to_owned(), url })
}

/// A string field at the top level of a JSON object. Nested objects, such as a release's `author` with its own
/// `html_url`, and text inside other strings are stepped over, never read as fields.
fn string_field(json: &str, name: &str) -> Option<String> {
    let mut chars = json.chars().peekable();
    skip_space(&mut chars);
    if chars.next()? != '{' {
        return None;
    }
    loop {
        skip_space(&mut chars);
        if chars.next_if_eq(&'}').is_some() {
            return None;
        }
        if chars.next()? != '"' {
            return None;
        }
        let key = read_string(&mut chars)?;
        skip_space(&mut chars);
        if chars.next()? != ':' {
            return None;
        }
        skip_space(&mut chars);
        if chars.peek() == Some(&'"') {
            chars.next();
            let value = read_string(&mut chars)?;
            if key == name {
                return Some(value);
            }
        } else {
            skip_value(&mut chars)?;
        }
        skip_space(&mut chars);
        if chars.next_if_eq(&',').is_none() {
            return None;
        }
    }
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

fn skip_space(chars: &mut Chars) {
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
}

/// The rest of a string whose opening quote has been read, unescaped.
fn read_string(chars: &mut Chars) -> Option<String> {
    let mut value = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'u' => {
                    let mut code = hex4(chars)?;
                    // A character beyond the first plane comes as two escapes, a surrogate pair.
                    if (0xD800..0xDC00).contains(&code) {
                        if chars.next()? != '\\' || chars.next()? != 'u' {
                            return None;
                        }
                        let low = hex4(chars)?;
                        code = 0x10000 + ((code - 0xD800) << 10) + low.checked_sub(0xDC00).filter(|low| *low < 0x400)?;
                    }
                    value.push(char::from_u32(code)?);
                }
                'b' => value.push('\u{8}'),
                'f' => value.push('\u{c}'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                c @ ('"' | '\\' | '/') => value.push(c),
                _ => return None,
            },
            c => value.push(c),
        }
    }
}

fn hex4(chars: &mut Chars) -> Option<u32> {
    let code: String = chars.by_ref().take(4).collect();
    if code.len() != 4 {
        return None;
    }
    u32::from_str_radix(&code, 16).ok()
}

/// Steps over a value other than a string: a number, `true`, `false`, `null`, or a whole object or array.
fn skip_value(chars: &mut Chars) -> Option<()> {
    let mut depth = 0usize;
    loop {
        match *chars.peek()? {
            '"' => {
                chars.next();
                read_string(chars)?;
            }
            '{' | '[' => {
                chars.next();
                depth += 1;
            }
            '}' | ']' if depth > 0 => {
                chars.next();
                depth -= 1;
            }
            ',' | '}' | ']' if depth == 0 => return Some(()),
            _ => {
                chars.next();
            }
        }
        if depth == 0 && matches!(chars.peek(), Some(',' | '}')) {
            return Some(());
        }
    }
}

/// Asks GitHub for the latest release. Any failure, offline or otherwise, is `None`.
pub fn fetch() -> Option<String> {
    unsafe {
        // A plain client name: nothing about the person goes along.
        let session = Handle(WinHttpOpen(w!("Snowfall-update-check"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if session.0.is_null() {
            return None;
        }
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 10_000, 10_000).ok()?;
        let connection = Handle(WinHttpConnect(session.0, HOST, 443, 0));
        if connection.0.is_null() {
            return None;
        }
        let request =
            Handle(WinHttpOpenRequest(connection.0, w!("GET"), PATH, PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
        if request.0.is_null() {
            return None;
        }
        let headers: Vec<u16> = "Accept: application/vnd.github+json\r\n".encode_utf16().collect();
        WinHttpAddRequestHeaders(request.0, &headers, 0x2000_0000).ok()?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).ok()?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).ok()?;
        const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
        let (mut status, mut size) = (0u32, 4u32);
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&raw mut status).cast::<c_void>()),
            &mut size,
            std::ptr::null_mut(),
        )
        .ok()?;
        if status != 200 {
            return None;
        }
        let mut body = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let mut read = 0;
            WinHttpReadData(request.0, chunk.as_mut_ptr().cast(), chunk.len() as u32, &mut read).ok()?;
            if read == 0 || body.len() > 1 << 20 {
                break;
            }
            body.extend_from_slice(&chunk[..read as usize]);
        }
        String::from_utf8(body).ok()
    }
}

/// A WinHTTP handle, closed when dropped.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert_eq!(compare("1.2.10", "1.2.9"), Some(Ordering::Greater));
        assert_eq!(compare("v0.2.0", "0.1.9"), Some(Ordering::Greater));
        assert_eq!(compare("1.0", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare("v1.0.0+build.7", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare("0.9.0", "1.0.0"), Some(Ordering::Less));
    }

    #[test]
    fn a_pre_release_comes_before_its_release_and_after_the_one_before() {
        assert_eq!(compare("1.3.0-beta.1", "1.3.0"), Some(Ordering::Less));
        assert_eq!(compare("1.3.0-beta.1", "1.2.9"), Some(Ordering::Greater));
        assert_eq!(compare("1.3.0-beta.2", "1.3.0-beta.10"), Some(Ordering::Less));
        assert_eq!(compare("1.3.0-alpha", "1.3.0-beta"), Some(Ordering::Less));
        assert_eq!(compare("1.3.0-beta", "1.3.0-beta.1"), Some(Ordering::Less));
        assert_eq!(compare("1.3.0-1", "1.3.0-alpha"), Some(Ordering::Less));
    }

    #[test]
    fn what_isnt_a_version_isnt_compared() {
        assert_eq!(compare("latest", "1.0.0"), None);
        assert_eq!(compare("1.2.3.4", "1.0.0"), None);
        assert_eq!(compare("", "1.0.0"), None);
    }

    const ANSWER: &str = r#"{"url":"https://api.github.com/repos/x/y/releases/1","html_url":"https://github.com/dsahakyan15/snowfall/releases/tag/v0.2.0","id":1,"tag_name":"v0.2.0","name":"Snowfall \"0.2\"","author":{"html_url":"https://github.com/someone"}}"#;

    #[test]
    fn a_newer_release_is_offered_with_its_page() {
        let release = newer(ANSWER, "0.1.0").expect("a newer release");
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.url, "https://github.com/dsahakyan15/snowfall/releases/tag/v0.2.0");
    }

    #[test]
    fn the_same_or_an_older_release_is_not_offered() {
        assert_eq!(newer(ANSWER, "0.2.0"), None);
        assert_eq!(newer(ANSWER, "0.3.0"), None);
        assert_eq!(newer("not json", "0.1.0"), None);
        assert_eq!(newer(r#"{"tag_name":"v9.0.0","html_url":"https://example.com/"}"#, "0.1.0"), None, "only GitHub pages are opened");
    }

    #[test]
    fn json_strings_are_unescaped() {
        // Escapes are spelled out at run time: `b` is a backslash.
        let b = char::from(92u8);
        let json = format!(r#"{{"name": "a {b}"b{b}" {b}u00e9{b}{b} {b}ud83d{b}ude80 {b}/{b}r"}}"#);
        assert_eq!(
            string_field(&json, "name"),
            Some(format!("a \"b\" {}{b} {} /\r", char::from_u32(0xe9).unwrap(), char::from_u32(0x1f680).unwrap()))
        );
        assert_eq!(string_field(&format!(r#"{{"name": "{b}ud83d"}}"#), "name"), None, "half a surrogate pair");
    }

    #[test]
    fn a_look_is_due_once_a_day() {
        assert!(due(None, 1_000));
        assert!(!due(Some(1_000), 1_000 + INTERVAL - 1));
        assert!(due(Some(1_000), 1_000 + INTERVAL));
        assert!(due(Some(5_000), 1_000), "a clock set back");
    }

    #[test]
    fn only_top_level_fields_are_read() {
        let answer = r#"{"author": {"html_url": "https://github.com/someone", "tag_name": "v99.0.0"}, "body": "\"tag_name\": \"v98\"", "assets": [{"x": [1, {"y": null}]}], "html_url": "https://github.com/o/r/releases/tag/v1.0.0", "tag_name": "v1.0.0", "prerelease": false}"#;
        assert_eq!(string_field(answer, "html_url").as_deref(), Some("https://github.com/o/r/releases/tag/v1.0.0"));
        assert_eq!(string_field(answer, "tag_name").as_deref(), Some("v1.0.0"));
        assert_eq!(string_field(answer, "missing"), None);
    }
}
