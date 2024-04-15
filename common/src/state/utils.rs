use std::{
    fs,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use regex::{Captures, Regex, Replacer};
use titlecase::titlecase;
use tracing::log;
use uuid::Uuid;
use walkdir::WalkDir;
use warp::crypto::DID;

use crate::{get_extras_dir, STATIC_ARGS};

use super::{ui::Font, Identity, State, Theme};

pub static USER_NAME_TAGS_REGEX: Lazy<Regex> =
    Lazy::new(|| mention_regex_epattern("[A-z0-9]+#[A-z0-9]{8}"));
pub static USER_DID_TAGS_REGEX: Lazy<Regex> =
    Lazy::new(|| mention_regex_epattern("did:key:[A-z0-9]{48}"));

pub fn get_available_themes() -> Vec<Theme> {
    let mut themes = vec![];

    let mut add_to_themes = |themes_path: &PathBuf| {
        log::debug!("add_to_themes: {}", themes_path.to_string_lossy());
        for file in WalkDir::new(themes_path)
            .into_iter()
            .filter_map(|file| file.ok())
        {
            if file.metadata().map(|x| x.is_file()).unwrap_or(false) {
                let theme_path = file.path().display().to_string();
                let pretty_theme_str = get_pretty_name(&theme_path);
                let pretty_theme_str = titlecase(&pretty_theme_str);

                let styles = fs::read_to_string(&theme_path).unwrap_or_default();
                let theme = Theme {
                    filename: theme_path.to_owned(),
                    name: pretty_theme_str.to_owned(),
                    styles,
                };
                if !themes.contains(&theme) {
                    themes.push(theme);
                }
            }
        }
    };
    add_to_themes(&STATIC_ARGS.themes_path);

    if let Ok(p) = get_extras_dir() {
        add_to_themes(&p.join("themes"));
    }

    themes.sort_by_key(|theme| theme.name.clone());

    let default = Theme {
        filename: "".into(),
        name: "Default".into(),
        styles: "".into(),
    };

    themes.push(default);

    themes.dedup(); // Why are we deduping here? We check above to only add if the theme doesn't already exist

    themes
}

fn get_pretty_name<S: AsRef<str>>(name: S) -> String {
    let path = Path::new(name.as_ref());
    let last = path
        .file_name()
        .and_then(|p| Path::new(p).file_stem())
        .unwrap_or_default();
    last.to_string_lossy().into()
}

pub fn get_available_fonts() -> Vec<Font> {
    let mut fonts = vec![];

    for file in WalkDir::new(&STATIC_ARGS.fonts_path)
        .into_iter()
        .filter_map(|file| file.ok())
    {
        if file.metadata().map(|x| x.is_file()).unwrap_or(false) {
            let file_osstr = file.file_name();
            let mut pretty_name: String = file_osstr.to_str().unwrap_or_default().into();
            pretty_name = pretty_name
                .replace(['_', '-'], " ")
                .split('.')
                .next()
                .unwrap()
                .into();

            let font = Font {
                name: pretty_name,
                path: file.path().to_str().unwrap_or_default().into(),
            };

            fonts.push(font);
        }
    }

    fonts
}

struct TagReplacer<'a, F: Fn(&Identity) -> String> {
    participants: &'a [Identity],
    own: &'a DID,
    is_mention: bool,
    is_username: bool,
    replacement: F,
}

impl<F: Fn(&Identity) -> String> Replacer for TagReplacer<'_, F> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        if !caps[0].starts_with('`') {
            let value = &caps[2];
            let key = &value[1..];
            if key.eq(&self.own.to_string()) {
                self.is_mention = true;
            }
            dst.push_str(&caps[1]);
            if let Some(id) = self.participants.iter().find(|id| {
                if self.is_username {
                    let name = format!("{}#{}", id.username(), id.short_id());
                    name.eq(key)
                } else {
                    id.did_key().to_string().eq(key)
                }
            }) {
                dst.push_str(&(self.replacement)(id))
            } else {
                dst.push_str(value);
            };
            dst.push_str(&caps[3]);
        } else {
            dst.push_str(&caps[0]);
        }
    }
}

pub fn mention_regex_epattern(value: &str) -> Regex {
    // This detects codeblocks
    // When replacing this needs to be explicitly checked
    let mut pattern = String::from(r#"(?:`{3}|`{1,2})+[^`]*(?:`{3}|`{1,2})"#);
    // Second capture group contains the mention
    // Since codeblocks are checked before they are basically "excluded"
    // First and third are any leading/trailing whitespaces
    pattern.push_str(&format!(r#"|(^|\s)(@{})($|\s)"#, value));
    Regex::new(&pattern).unwrap()
}

pub fn parse_mention_state(
    message: &str,
    state: &State,
    chat: Uuid,
    replacement: impl Fn(&Identity) -> String,
) -> (String, bool) {
    parse_mentions(
        message,
        &state
            .get_chat_by_id(chat)
            .map(|c| state.chat_participants(&c))
            .unwrap_or_default(),
        &state.did_key(),
        false,
        replacement,
    )
}

// Parse a message replacing mentions with a given function
pub fn parse_mentions(
    message: &str,
    participants: &[Identity],
    own: &DID,
    is_username: bool,
    replacement: impl Fn(&Identity) -> String,
) -> (String, bool) {
    let mut replacer = TagReplacer {
        participants,
        own,
        is_username,
        is_mention: false,
        replacement,
    };
    let result = if is_username {
        USER_NAME_TAGS_REGEX.replace_all(message, replacer.by_ref())
    } else {
        USER_DID_TAGS_REGEX.replace_all(message, replacer.by_ref())
    };
    (result.to_string(), replacer.is_mention)
}

pub fn mention_to_did_key(id: &Identity) -> String {
    format!("@{}", id.did_key())
}

// Replacement pattern converting a user tag to a highlight div
pub fn mention_replacement_pattern(id: &Identity, visual: bool) -> String {
    format!(
        r#"<a class="message-user-tag {}" value="{}">@{}</a>"#,
        if visual { "visual-only" } else { "" },
        id.did_key(),
        id.username()
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_pretty_name1() {
        if cfg!(windows) {
            let r = get_pretty_name("c:\\pretty\\name2.scss");
            assert_eq!(r, String::from("name2"));
        } else {
            let r = get_pretty_name("pretty/name1.scss");
            assert_eq!(r, String::from("name1"));
        }
    }
}
