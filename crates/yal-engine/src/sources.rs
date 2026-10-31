//! The command sources.
//!
//! Each source knows how to turn the host state it owns into candidate items.
//! Sources never rank; they only produce. The engine ranks across all of
//! them so that a snippet and an application compete on equal terms.

use chrono::{DateTime, Utc};
use yal_calc::{Value, evaluate, format as format_value};
use yal_core::action::Action;
use yal_core::command::CommandKind;
use yal_core::icon::Icon;
use yal_core::item::Item;
use yal_data::emoji::Emoji;
use yal_index::Discovered;
use yal_search::Candidate;
use yal_store::Store;

/// The kind of thing a result came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An installed application.
    Application,
    /// A saved text snippet.
    Snippet,
    /// A saved link.
    Quicklink,
    /// A saved bookmark.
    Bookmark,
    /// A clipboard history entry.
    Clipboard,
    /// The result of evaluating the query as an expression.
    Calculator,
    /// An emoji.
    Emoji,
}

impl Source {
    /// The command this source belongs to.
    pub fn command_kind(self) -> CommandKind {
        match self {
            Source::Application => CommandKind::Applications,
            Source::Snippet => CommandKind::Snippets,
            Source::Quicklink => CommandKind::Quicklinks,
            Source::Bookmark => CommandKind::Bookmarks,
            Source::Clipboard => CommandKind::Clipboard,
            Source::Calculator => CommandKind::Calculator,
            Source::Emoji => CommandKind::Emoji,
        }
    }
}

/// Build candidates for installed applications.
pub fn applications(discovered: &[Discovered]) -> Vec<Candidate> {
    discovered
        .iter()
        .map(|application| {
            let mut item = Item::new(application.name.clone())
                .with_id(application.id.clone())
                .with_subtitle(application.path.display().to_string())
                .with_action(Action::open_path(application.path.clone()))
                .with_secondary_action(Action::reveal(application.path.clone()))
                .with_secondary_action(Action::copy(
                    application.path.display().to_string(),
                ));
            if let Some(icon) = icon_for(&application.path) {
                item = item.with_icon(icon);
            }
            Candidate::new(item, CommandKind::Applications)
        })
        .collect()
}

/// Build candidates for the aliases that rename applications.
///
/// A user's alias is a short word for an application they use constantly:
/// `gh` for GitHub Desktop, for example. When the query *is* the term, the
/// renamed application is offered, with the alias credited in the subtitle
/// so it is clear why it appeared. The target is resolved by name against
/// the same application list the plain search uses, so an alias for an
/// uninstalled application offers nothing.
pub fn aliases(
    store: &Store,
    discovered: &[Discovered],
    query: &str,
) -> yal_store::StoreResult<Vec<Candidate>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }

    let mut found = Vec::new();
    for alias in store.aliases()? {
        if alias.term.to_lowercase() != needle {
            continue;
        }
        for application in discovered {
            if application.name.to_lowercase() == alias.target.to_lowercase() {
                let item = Item::new(application.name.clone())
                    .with_id(application.id.clone())
                    .with_subtitle(format!(
                        "{} — an alias of yours ({})",
                        application.path.display(),
                        alias.term
                    ))
                    .with_keyword(alias.term.clone())
                    .with_action(Action::open_path(application.path.clone()))
                    .with_secondary_action(Action::reveal(application.path.clone()))
                    .with_secondary_action(Action::copy(
                        application.path.display().to_string(),
                    ));
                found.push(Candidate::new(item, CommandKind::Applications));
                break;
            }
        }
    }
    Ok(found)
}

/// Build candidates for the user's script commands.
///
/// Every script in the folder is offered every search: the ranking decides
/// what the query matches against titles and keywords, the same way it does
/// for the applications. The folder is the user's; an unreadable or absent
/// folder offers nothing rather than erroring, since a missing folder is a
/// way a user can choose not to have script commands at all.
pub fn scripts(scripts_folder: Option<std::path::PathBuf>) -> Vec<Candidate> {
    let Some(folder) = yal_system::scripts::resolve_folder(
        scripts_folder.filter(|path| !path.as_os_str().is_empty()),
    ) else {
        return Vec::new();
    };

    yal_system::scripts::list(&folder)
        .into_iter()
        .map(|script| {
            use yal_system::scripts::Mode;

            let slug = script.mode.slug();
            let action = match script.mode {
                Mode::Silent => {
                    // Detached: the palette hands the runner off and
                    // forgets about it.
                    let (program, args) = yal_system::scripts::runner_command(&script);
                    Action::run(program, args)
                }
                _ => {
                    // The palette task runs it in the background and
                    // announces the outcome, so the window does not wait.
                    Action::task(
                        "script",
                        vec![
                            script.path.to_string_lossy().into_owned(),
                            slug.to_string(),
                        ],
                    )
                }
            };

            let runner = match script.runner {
                yal_system::scripts::Runner::PowerShell => "PowerShell",
                yal_system::scripts::Runner::Batch => "batch",
                yal_system::scripts::Runner::Python => "Python",
            };
            let subtitle = match (&script.description, script.mode) {
                (Some(description), _) => format!("{runner} script — {description} ({slug})"),
                (None, _) => format!("{runner} script ({slug})"),
            };

            let mut item = Item::new(script.title.clone())
                .with_id(format!("script:{}", script.path.display()))
                .with_subtitle(subtitle)
                .with_action(action)
                .with_secondary_action(Action::reveal(script.path.clone()));
            for keyword in &script.keywords {
                item = item.with_keyword(keyword.clone());
            }
            Candidate::new(item, CommandKind::Scripts)
        })
        .collect()
}
/// Build candidates for snippets.
pub fn snippets(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .snippets()?
        .into_iter()
        .map(|snippet| {
            let item = Item::new(snippet.name.clone())
                .with_id(format!("snippet:{}", snippet.id))
                .with_subtitle(snippet.expansion.clone())
                .with_keyword(snippet.keyword.clone())
                .with_action(Action::paste(snippet.expansion.clone()))
                .with_secondary_action(Action::copy(snippet.expansion.clone()));
            Candidate::new(item, CommandKind::Snippets)
        })
        .collect())
}

/// Build candidates for quicklinks.
///
/// A quicklink may hold a `{}` placeholder for the query. When there is a
/// query, the placeholder is filled with the percent-encoded text, so
/// `google.com/search?q={}` plus `rust closures` opens the search results
/// page. With no query the link opens as written, which is how a quicklink
/// whose target has no placeholder behaves.
pub fn quicklinks(store: &Store, query: &str) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .quicklinks()?
        .into_iter()
        .map(|quicklink| {
            let url = if query.is_empty() {
                quicklink.url.clone()
            } else {
                quicklink.url.replace("{}", &url_encode(query))
            };
            let item = Item::new(quicklink.name.clone())
                .with_id(format!("quicklink:{}", quicklink.id))
                .with_subtitle(url.clone())
                .with_action(Action::open_url(url))
                .with_secondary_action(Action::copy(
                    if query.is_empty() {
                        quicklink.url.clone()
                    } else {
                        quicklink.url.replace("{}", &url_encode(query))
                    },
                ));
            Candidate::new(item, CommandKind::Quicklinks)
        })
        .collect())
}

/// Build candidates for bookmarks.
pub fn bookmarks(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .bookmarks()?
        .into_iter()
        .map(|bookmark| {
            let item = Item::new(bookmark.name.clone())
                .with_id(format!("bookmark:{}", bookmark.id))
                .with_subtitle(bookmark.url.clone())
                .with_action(Action::open_url(bookmark.url.clone()));
            Candidate::new(item, CommandKind::Bookmarks)
        })
        .collect())
}

/// Build candidates for the clipboard history.
pub fn clipboard(store: &Store, limit: usize) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(store
        .clipboard(limit)?
        .into_iter()
        .map(|entry| {
            let item = Item::new(first_line(&entry.text))
                .with_id(format!("clipboard:{}", entry.id))
                .with_subtitle(entry.text.clone())
                .with_action(Action::copy(entry.text.clone()))
                .with_secondary_action(Action::task(
                    "clipboard-remove",
                    vec![entry.id.to_string()],
                ));
            Candidate::new(item, CommandKind::Clipboard)
        })
        .collect())
}

/// Build the calculator candidate, if the query evaluates to a value.
pub fn calculator(query: &str) -> Option<Candidate> {
    let value = evaluate(query).ok()?;
    if !is_interesting(&value) {
        return None;
    }
    let text = format_value(&value);
    let item = Item::new(text.clone())
        .with_id("calculator:result".to_string())
        .with_subtitle(format!("{query} = {text}"))
        .with_action(Action::copy(text));
    Some(Candidate::new(item, CommandKind::Calculator))
}

/// Build candidates for emoji.
pub fn emoji(query: &str, limit: usize) -> Vec<Candidate> {
    yal_data::emoji::search(query, limit)
        .into_iter()
        .map(|Emoji { character, name, keywords }| {
            let mut item = Item::new(character.clone())
                .with_id(format!("emoji:{}", name))
                .with_subtitle(name.clone())
                .with_action(Action::copy(character.clone()));
            for keyword in keywords {
                item = item.with_keyword(keyword);
            }
            Candidate::new(item, CommandKind::Emoji)
        })
        .collect()
}

/// A note read out of the stored settings.
pub struct SavedNote {
    /// The identifier, as stored under `note:<id>`.
    pub id: String,
    /// What the note says, possibly over several lines.
    pub text: String,
    /// When the note was taken.
    pub created_at: DateTime<Utc>,
}

/// Decode the notes stored in settings.
///
/// The one interpretation of a stored note record, shared by the engine's
/// listing, the CLI and the palette's task that takes one.
pub fn saved_notes(store: &Store) -> yal_store::StoreResult<Vec<SavedNote>> {
    let mut found = Vec::new();
    for (key, value) in store.settings()? {
        let Some(id) = key.strip_prefix("note:") else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) else {
            continue;
        };
        let Some(text) = parsed.get("text").and_then(|text| text.as_str()) else {
            continue;
        };
        let Some(created_at) = parsed
            .get("created_at")
            .and_then(|at| at.as_i64())
            .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        else {
            continue;
        };
        found.push(SavedNote {
            id: id.to_string(),
            text: text.to_string(),
            created_at,
        });
    }
    Ok(found)
}

/// Build candidates for the notes, newest first.
///
/// The note's first line is the title; when the note carries more lines
/// the rest rides in the subtitle. The primary act copies the whole text
/// — a note is written to be taken somewhere — and the panel removes it.
pub fn notes(store: &Store) -> yal_store::StoreResult<Vec<Candidate>> {
    let mut found = saved_notes(store)?;
    found.sort_by_key(|note| note.created_at);
    found.reverse();
    Ok(found
        .into_iter()
        .map(|note| {
            let first = first_line(&note.text);
            let rest: String = note
                .text
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            let subtitle = if rest.is_empty() {
                note.created_at
                    .with_timezone(&chrono::Local)
                    .format("taken %d %b %H:%M")
                    .to_string()
            } else {
                format!(
                    "{} — {}",
                    rest,
                    note.created_at
                        .with_timezone(&chrono::Local)
                        .format("taken %d %b %H:%M")
                )
            };
            let text = note.text.clone();
            Candidate::new(
                Item::new(first)
                    .with_id(format!("note:{}", note.id))
                    .with_subtitle(subtitle)
                    .with_keyword("notes")
                    .with_action(Action::copy(text))
                    .with_secondary_action(Action::task(
                        "note-remove",
                        vec![note.id.clone()],
                    )),
                CommandKind::Notes,
            )
        })
        .collect())
}

/// The note candidate for a query like `note pick up the print out`.
///
/// There is no parser: a reminder needs its due time, a note needs
/// nothing but its text. `None` when the query is not a note at all.
pub fn note_specification(query: &str) -> Option<Candidate> {
    let rest = query.strip_prefix("note ")?.trim();
    if rest.is_empty() {
        return None;
    }
    let text = rest.to_string();
    Some(Candidate::new(
        Item::new(first_line(&format!("Take a note — {text}")))
            .with_id("note:new".to_string())
            .with_subtitle("kept in the local store; listed among the notes")
            .with_keyword("note")
            .with_action(Action::task("note-add", vec![text])),
        CommandKind::Notes,
    ))
}

/// Build the QR code candidate for a query.
///
/// The rendered PNG travels in the item payload so the palette can draw it
/// without touching the filesystem.
pub fn qr_code(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let code = yal_data::qr::generate(trimmed, 6).ok()?;
    let encoded = base64_encode(&code.png);
    let item = Item::new(format!("QR code — {trimmed}"))
        .with_id("qr:result".to_string())
        .with_subtitle(format!("{}×{} pixels", code.width, code.height))
        .with_action(Action::copy(trimmed.to_string()))
        .with_payload(serde_json::json!({
            "kind": "qr",
            "png_base64": encoded,
            "width": code.width,
            "height": code.height,
        }));
    Some(Candidate::new(item, CommandKind::QrCode))
}

/// Build candidates for the installed font families.
///
/// The family name is what a font-choosing program asks for, so copying it
/// is the one action that cannot disappoint. The subtitle carries a preview
/// phrase: the palette draws it in its own font (a limitation stated in the
/// README), so the phrase is here as what the family will look like when
/// the user later applies it, not as a rendering.
pub fn fonts(
    families: &[yal_system::fonts::FontFamily],
    query: &str,
    limit: usize,
) -> Vec<Candidate> {
    families
        .iter()
        .filter(|font| {
            if query.is_empty() {
                return true;
            }
            let needle = query.to_lowercase();
            font.family.to_lowercase().contains(&needle)
        })
        .take(limit)
        .map(|font| {
            Candidate::new(
                Item::new(font.family.clone())
                    .with_id(format!("font:{}", font.family))
                    .with_subtitle("the quick brown fox jumps over the lazy dog · Enter copies the name")
                    .with_keyword(format!("font {}", font.family.to_lowercase()))
                    .with_action(Action::copy(font.family.clone())),
                CommandKind::Fonts,
            )
        })
        .collect()
}

/// Build the one-time password candidate for a query.
///
/// The query is a base32 secret. The subtitle carries the seconds remaining
/// before the code changes.
pub fn totp(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let secret = yal_data::totp::TotpSecret::parse(trimmed)?;
    let now = chrono::Utc::now().timestamp() as u64;
    let (code, remaining) = secret.code_and_remaining(
        now,
        yal_data::totp::DEFAULT_DIGITS,
        yal_data::totp::DEFAULT_PERIOD,
    );
    let item = Item::new(code.clone())
        .with_id("totp:result".to_string())
        .with_subtitle(format!("{remaining}s remaining"))
        .with_action(Action::copy(code))
        .with_payload(serde_json::json!({
            "kind": "totp",
            "seconds_remaining": remaining,
        }));
    Some(Candidate::new(item, CommandKind::Passwords))
}

/// Build the regular expression candidate for a query.
///
/// The pattern is the query; the subject is the text to match against.
pub fn regex(query: &str, subject: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let result = yal_data::regex::match_pattern(trimmed, subject);
    let summary = yal_data::regex::summarise(&result);
    let mut item = Item::new(summary.clone())
        .with_id("regex:result".to_string())
        .with_subtitle(trimmed.to_string());
    if let Some(first) = result.matches.first() {
        item = item.with_action(Action::copy(first.text.clone()));
    }
    Some(Candidate::new(item, CommandKind::Regex))
}

/// Build candidates for the open windows.
///
/// The window itself, plus the window-management actions for it. Each action
/// is its own candidate so the palette stays a flat list: a query for one
/// window ranks its own actions with it.
pub fn windows(list: &[yal_system::Window]) -> Vec<Candidate> {
    list.iter()
        .flat_map(|window| {
            let title = window.title.clone();
            let prefix = window.process_name.clone().unwrap_or_else(|| {
                format!("pid {}", window.process_id)
            });

            let mut focus = Item::new(title.clone())
                .with_id(format!("window:{}", window.handle))
                .with_subtitle(prefix.clone())
                .with_action(Action::task(
                    "window",
                    vec![window.handle.to_string(), "focus".into()],
                ))
                .with_secondary_action(Action::task(
                    "screenshot-window",
                    vec![window.handle.to_string()],
                ));
            if let Some(name) = &window.process_name {
                focus = focus.with_keyword(name.clone());
            }

            let mut out = vec![Candidate::new(focus, CommandKind::Windows)];
            out.extend(
                yal_system::window_controls::WINDOW_MENU
                    .iter()
                    .filter(|entry| {
                        entry.action != yal_system::window_controls::WindowAction::Focus
                    })
                    .map(|entry| {
                        Candidate::new(
                            Item::new(format!("{} — {}", entry.title, title))
                                .with_id(format!("window:{}:{}", window.handle, entry.title))
                                .with_subtitle(prefix.clone())
                                .with_action(Action::task(
                                    "window",
                                    vec![
                                        window.handle.to_string(),
                                        action_slug(entry.action).to_string(),
                                    ],
                                )),
                            CommandKind::Windows,
                        )
                    }),
            );
            out
        })
        .collect()
}

/// The slug a window action is carried out under.
fn action_slug(action: yal_system::window_controls::WindowAction) -> &'static str {
    use yal_system::window_controls::WindowAction;
    match action {
        WindowAction::Close => "close",
        WindowAction::Minimize => "minimize",
        WindowAction::Maximize => "maximize",
        WindowAction::Restore => "restore",
        WindowAction::Focus => "focus",
        WindowAction::LeftHalf => "left-half",
        WindowAction::RightHalf => "right-half",
        WindowAction::TopHalf => "top-half",
        WindowAction::BottomHalf => "bottom-half",
        WindowAction::TopLeftQuarter => "top-left-quarter",
        WindowAction::TopRightQuarter => "top-right-quarter",
        WindowAction::BottomLeftQuarter => "bottom-left-quarter",
        WindowAction::BottomRightQuarter => "bottom-right-quarter",
        WindowAction::LeftThird => "left-third",
        WindowAction::CenterThird => "middle-third",
        WindowAction::RightThird => "right-third",
        WindowAction::Center => "center",
        WindowAction::Fullscreen => "fullscreen",
    }
}

/// Build candidates for the system commands.
///
/// The destructive actions are here because the observed launcher offers them; carrying
/// them out is guarded by the user choosing them deliberately. The volume
/// and theme actions read the current state for their subtitle, so a muted
/// machine offers "Restore the volume" rather than a guess.
pub fn system_commands() -> Vec<Candidate> {
    use yal_system::system::SystemCommand;

    let destructive = [
        SystemCommand::Lock,
        SystemCommand::Sleep,
        SystemCommand::Shutdown,
        SystemCommand::Restart,
        SystemCommand::LogOut,
        SystemCommand::EmptyBin,
    ];

    let mut found: Vec<Candidate> = destructive
        .into_iter()
        .map(|command| {
            let (slug, subject) = match command {
                SystemCommand::Lock => ("lock", "affects your session"),
                SystemCommand::Sleep => ("sleep", "affects the whole machine"),
                SystemCommand::Shutdown => ("shutdown", "affects the whole machine"),
                SystemCommand::Restart => ("restart", "affects the whole machine"),
                SystemCommand::LogOut => ("logout", "affects your session"),
                SystemCommand::EmptyBin => ("empty-bin", "empties the recycle bin"),
            };
            let mut item = Item::new(command.title())
                .with_id(format!("system:{slug}"))
                .with_subtitle(subject)
                .with_action(Action::task("system", vec![slug.to_string()]));
            for keyword in command.keywords() {
                item = item.with_keyword(*keyword);
            }
            Candidate::new(item, CommandKind::System)
        })
        .collect();

    // The symmetric volume actions. What is on offer depends on the state
    // right now, which is as honest as a subtitle can be.
    if let Ok(state) = yal_system::audio::volume() {
        let builder = |title: &str, slug: &str, id: String| {
            Candidate::new(
                Item::new(title.to_string())
                    .with_id(id)
                    .with_subtitle(format!("currently {:.0}", state.level))
                    .with_action(Action::task("system", vec![slug.to_string()])),
                CommandKind::System,
            )
        };
        if state.muted {
            found.push(builder("Restore the volume", "unmute", "system:unmute".into()));
        } else {
            found.push(builder("Mute the volume", "mute", "system:mute".into()));
        }
        found.push(builder("Volume up", "volume-up", "system:volume-up".into()));
        found.push(builder("Volume down", "volume-down", "system:volume-down".into()));
    }

    // The theme.
    if let Ok(dark) = yal_system::display::is_dark_mode() {
        let (title, slug) = if dark {
            ("Switch to light mode", "light-mode")
        } else {
            ("Switch to dark mode", "dark-mode")
        };
        found.push(Candidate::new(
            Item::new(title)
                .with_id(format!("system:{slug}"))
                .with_subtitle("applications re-read it as they restart")
                .with_action(Action::task("system", vec![slug.to_string()])),
            CommandKind::System,
        ));
    }

    // Generators that need no input.
    found.push(Candidate::new(
        Item::new(yal_data::text::uuid())
            .with_id("system:uuid")
            .with_subtitle("a fresh identifier, ready to copy")
            .with_action(Action::copy(quick_uuid())),
        CommandKind::System,
    ));

    // The screenshot acts: the screen read is a plain movement of pixels
    // to a file and to the clipboard. The whole-screen act hides the
    // palette first, so the launcher is not inside its own picture; the
    // area act takes the screen over for a drag.
    found.push(Candidate::new(
        Item::new("Capture the screen")
            .with_id("system:screenshot-screen")
            .with_subtitle("the whole screen, saved and on the clipboard")
            .with_action(Action::task("screenshot-screen", Vec::new())),
        CommandKind::System,
    ));
    found.push(Candidate::new(
        Item::new("Capture an area")
            .with_id("system:capture-area")
            .with_subtitle("drag a rectangle; it lands as a picture")
            .with_action(Action::task("capture-area", Vec::new())),
        CommandKind::System,
    ));
    // The eyedropper: the screen freezes under the palette and a loupe
    // follows the pointer; a click or Enter takes the pixel under it, and
    // the colour is said as hex, rgb and hsl.
    found.push(Candidate::new(
        Item::new("Pick a colour")
            .with_id("system:pick-colour")
            .with_subtitle("a loupe over what is on screen; the colour lands on the clipboard")
            .with_action(Action::task("pick-colour", Vec::new())),
        CommandKind::System,
    ));

    found
}

/// A fresh identifier, generated at build time so the title and the copied
/// text are the same string.
fn quick_uuid() -> String {
    yal_data::text::uuid()
}

/// Build candidates for the text transforms.
///
/// The query is split at its first space: the head chooses the transform and
/// the remainder is the text to transform. `upper hello` offers the
/// upper-cased text; a head with no text yet offers the transform without a
/// preview.
pub fn text_transforms(query: &str) -> Vec<Candidate> {
    let Some((head, input)) = query.split_once(' ') else {
        return Vec::new();
    };
    let head = head.trim().to_lowercase();
    let input = input.trim();
    if head.is_empty() {
        return Vec::new();
    }

    yal_data::text::TRANSFORMS
        .iter()
        .filter(|transform| {
            transform.slug == head
                || transform.keywords.iter().any(|keyword| *keyword == head)
        })
        .filter_map(|transform| {
            if input.is_empty() {
                // Without input there is nothing to preview; the palette
                // shows the transform and asks for the text.
                return Some(Candidate::new(
                    Item::new(transform.title)
                        .with_id(format!("text:{}", transform.slug))
                        .with_subtitle("type the text after the transform"),
                    CommandKind::Text,
                ));
            }

            let result = yal_data::text::apply(transform.slug, input).ok()?;
            // The user typed the head and the input as one query; that is
            // the text this result is ranked against.
            let invocation = format!("{head} {input}");
            let mut item = Item::new(display_text(&result))
                .with_id(format!("text:{}", transform.slug))
                .with_subtitle(format!("{} — copied when chosen", transform.title))
                .with_keyword(invocation)
                .with_keyword(transform.slug.to_string());
            for keyword in transform.keywords {
                item = item.with_keyword((*keyword).to_string());
            }
            Some(Candidate::new(
                item.with_action(Action::copy(result.clone()))
                    .with_payload(serde_json::json!({ "kind": "text", "result": result })),
                CommandKind::Text,
            ))
        })
        .collect()
}

/// A result shortened for the title line.
fn display_text(text: &str) -> String {
    const WIDTH: usize = 80;
    let line = text.lines().next().unwrap_or(text);
    if line.chars().count() <= WIDTH {
        return line.to_string();
    }
    line.chars().take(WIDTH).collect()
}

/// Build candidates for the running processes.
///
/// `query` is the text after the `kill` prefix; an empty query offers the
/// most demanding processes first.
pub fn processes(
    running: &[yal_system::processes::Running],
    query: &str,
    limit: usize,
) -> Vec<Candidate> {
    running
        .iter()
        .filter(|process| {
            if query.is_empty() {
                return true;
            }
            let needle = query.to_lowercase();
            process.name.to_lowercase().contains(&needle)
                || process.process_id.to_string().contains(&needle)
        })
        .take(limit)
        .map(|process| {
            Candidate::new(
                Item::new(format!("Kill — {}", process.name))
                    .with_id(format!("kill:{}", process.process_id))
                    .with_subtitle(format!(
                        "pid {}, {:.0} MB in use",
                        process.process_id,
                        process.memory as f64 / 1_048_576.0
                    ))
                    .with_keyword(format!("kill {}", process.process_id))
                    .with_keyword(format!("kill {}", process.name.to_lowercase()))
                    .with_action(Action::task(
                        "kill",
                        vec![process.process_id.to_string()],
                    )),
                CommandKind::Process,
            )
        })
        .collect()
}

/// Build candidates for the open browser tabs.
pub fn browser_tabs(
    tabs: &[yal_system::tabs::BrowserTab],
    query: &str,
    limit: usize,
) -> Vec<Candidate> {
    tabs.iter()
        .filter(|tab| {
            if query.is_empty() {
                return true;
            }
            let needle = query.to_lowercase();
            tab.title.to_lowercase().contains(&needle)
                || tab.browser.to_lowercase().contains(&needle)
        })
        .take(limit)
        .map(|tab| {
            Candidate::new(
                Item::new(tab.title.clone())
                    .with_id(format!("tab:{}:{}", tab.window, tab.title))
                    .with_subtitle(tab.browser.clone())
                    .with_keyword(format!("tabs {}", tab.title.to_lowercase()))
                    .with_keyword(format!("tabs {}", tab.browser.to_lowercase()))
                    .with_keyword(tab.title.to_lowercase())
                    .with_action(Action::task(
                        "window",
                        vec![tab.window.to_string(), "focus".into()],
                    )),
                CommandKind::BrowserTabs,
            )
        })
        .collect()
}

/// A saved timer, decoded from the store.
pub struct SavedTimer {
    /// The identifier, as stored under `timer:<id>`.
    pub id: String,
    /// What the timer counts down to.
    pub label: String,
    /// When the timer runs out.
    pub ends_at: chrono::DateTime<chrono::Utc>,
}

/// Decode the timers stored in settings.
///
/// This is the one interpretation of a stored timer record; the engine's
/// listing, the CLI and the palette's due watcher all read through it.
pub fn saved_timers(store: &Store) -> yal_store::StoreResult<Vec<SavedTimer>> {
    let mut found = Vec::new();
    for (key, value) in store.settings()? {
        let Some(id) = key.strip_prefix("timer:") else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) else {
            continue;
        };
        let Some(label) = parsed.get("label").and_then(|label| label.as_str()) else {
            continue;
        };
        let Some(ends_at) = parsed.get("ends_at").and_then(|ends| ends.as_i64()) else {
            continue;
        };
        let Some(ends_at) = chrono::DateTime::<chrono::Utc>::from_timestamp(ends_at, 0) else {
            continue;
        };
        found.push(SavedTimer {
            id: id.to_string(),
            label: label.to_string(),
            ends_at,
        });
    }
    Ok(found)
}

/// Build candidates for the running timers.
///
/// An expired timer is not listed: it is raised as a banner by the palette
/// and removed once the user has acknowledged it.
pub fn timers(store: &Store, now: chrono::DateTime<chrono::Utc>) -> yal_store::StoreResult<Vec<Candidate>> {
    Ok(saved_timers(store)?
        .into_iter()
        .filter(|timer| timer.ends_at > now)
        .map(|timer| {
            let remaining = timer.ends_at - now;
            let seconds = remaining.num_seconds().max(0);
            let minutes = seconds / 60;
            let shown = if minutes >= 1 {
                format!("{}m {:02}s", minutes, seconds % 60)
            } else {
                format!("{seconds}s")
            };
            Candidate::new(
                Item::new(format!("Timer — {}", timer.label))
                    .with_id(format!("timer:{}", timer.id))
                    .with_subtitle(format!("{shown} remaining"))
                    .with_action(Action::task("timer-cancel", vec![timer.id.clone()])),
                CommandKind::Timer,
            )
        })
        .collect())
}

/// The timer candidate for a query like `timer 10m pomodoro`.
///
/// The duration is a run of tokens such as `90`, `10m`, `1h 30m`; the label
/// is everything after the first token that is not a duration. `None` when
/// the query is not a timer at all.
pub fn timer_specification(query: &str) -> Option<Candidate> {
    let rest = query.strip_prefix("timer ")?.trim();
    if rest.is_empty() {
        return None;
    }

    let mut total: u64 = 0;
    let mut consumed = String::new();
    for token in rest.split_whitespace() {
        let split = token
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(token.len());
        let (digits, unit) = token.split_at(split);
        let multiplier = match unit.to_lowercase().as_str() {
            "" | "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => break,
        };
        let Ok(amount) = digits.parse::<u64>() else {
            break;
        };
        total += amount.saturating_mul(multiplier);
        consumed.push_str(token);
        consumed.push(' ');
    }

    if total == 0 || total > 24 * 3_600 {
        return None;
    }

    // The tokens that parsed as durations are exactly the prefix of the
    // query, so the rest is the label.
    let remainder = rest[consumed.len()..].trim();
    let label = if remainder.is_empty() {
        "Timer".to_string()
    } else {
        remainder.to_string()
    };

    Some(Candidate::new(
        Item::new(format!(
            "Timer — {} for {label}",
            shown_duration(total)
        ))
        .with_id("timer:new".to_string())
        .with_subtitle("starts when chosen")
        .with_action(Action::task(
            "timer",
            vec![total.to_string(), label.clone()],
        )),
        CommandKind::Timer,
    ))
}

/// A duration as it is written to people.
fn shown_duration(total: u64) -> String {
    let hours = total / 3_600;
    let minutes = (total % 3_600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

/// A reminder read out of the stored settings.
pub struct SavedReminder {
    /// The identifier, as stored under `reminder:<id>`.
    pub id: String,
    /// What the reminder says.
    pub text: String,
    /// When the reminder comes due.
    pub due_at: DateTime<Utc>,
}

/// Decode the reminders stored in settings.
///
/// The one interpretation of a stored reminder record, shared by the
/// engine's listing, the CLI and the palette's due watcher.
pub fn saved_reminders(store: &Store) -> yal_store::StoreResult<Vec<SavedReminder>> {
    let mut found = Vec::new();
    for (key, value) in store.settings()? {
        let Some(id) = key.strip_prefix("reminder:") else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) else {
            continue;
        };
        let Some(text) = parsed.get("text").and_then(|text| text.as_str()) else {
            continue;
        };
        let Some(due_at) = parsed
            .get("due_at")
            .and_then(|due| due.as_i64())
            .and_then(|timestamp| chrono::DateTime::<Utc>::from_timestamp(timestamp, 0))
        else {
            continue;
        };
        found.push(SavedReminder {
            id: id.to_string(),
            text: text.to_string(),
            due_at,
        });
    }
    Ok(found)
}

/// Build candidates for the reminders still open, soonest first.
///
/// Completing a reminder removes it — the same act the due path performs
/// when the time comes.
pub fn reminders(
    store: &Store,
    now: chrono::DateTime<Utc>,
) -> yal_store::StoreResult<Vec<Candidate>> {
    let mut found = saved_reminders(store)?;
    found.sort_by_key(|reminder| reminder.due_at);
    Ok(found
        .into_iter()
        .map(|reminder| {
            let due_in = (reminder.due_at - now).num_seconds();
            let shown = shown_duration(due_in.unsigned_abs());
            let subtitle = if due_in >= 0 {
                format!("due in {shown}")
            } else {
                format!("overdue by {shown}")
            };
            Candidate::new(
                Item::new(reminder.text.clone())
                    .with_id(format!("reminder:{}", reminder.id))
                    .with_subtitle(subtitle)
                    .with_action(Action::task(
                        "reminder-done",
                        vec![reminder.id.clone()],
                    )),
                CommandKind::Reminders,
            )
        })
        .collect())
}

/// The reminder candidate for a query like `remind check the oven in 40m`.
///
/// The words are read by one parser the CLI shares, so the palette and
/// the command line agree on what a reminder is. `None` when the words
/// are not one.
pub fn reminder_specification(query: &str, now: DateTime<Utc>) -> Option<Candidate> {
    let spec = yal_data::remind::parse(query, now)?;
    let raised = spec
        .due_at
        .with_timezone(&chrono::Local)
        .format("%a %H:%M");
    Some(Candidate::new(
        Item::new(format!("Reminder - {}", spec.text))
            .with_id("reminder:new")
            .with_subtitle(format!("raised {raised}, starts when chosen"))
            .with_action(Action::task(
                "reminder",
                vec![spec.due_at.timestamp().to_string(), spec.text.clone()],
            )),
        CommandKind::Reminders,
    ))
}

/// Build candidates for files found on disk.
pub fn files(found: &[yal_index::FoundFile]) -> Vec<Candidate> {
    found
        .iter()
        .map(|file| {
            let item = Item::new(file.name.clone())
                .with_id(format!("file:{}", file.path.display()))
                .with_subtitle(file.directory.display().to_string())
                .with_action(Action::open_path(file.path.clone()))
                .with_secondary_action(Action::reveal(file.path.clone()))
                .with_secondary_action(Action::copy(
                    file.path.display().to_string(),
                ));
            Candidate::new(item, CommandKind::Files)
        })
        .collect()
}

/// Build the fallback acts for a query that matched nothing.
///
/// The words themselves are all there is to offer: they can travel to a
/// search engine in the browser, or — when they have the shape of an
/// address — be opened as one. Both acts carry the words in the subtitle
/// so the composition is visible before anything opens, and both copy the
/// words as a secondary act, the thing a user most often wants next.
pub fn fallback(query: &str) -> Vec<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut found = Vec::new();

    let url = format!("https://duckduckgo.com/?q={}", url_encode(trimmed));
    found.push(Candidate::new(
        Item::new(format!("Search the web for “{trimmed}”"))
            .with_id("fallback:websearch".to_string())
            .with_subtitle(url.clone())
            .with_action(Action::open_url(url))
            .with_secondary_action(Action::copy(trimmed.to_string())),
        CommandKind::Fallback,
    ));

    if looks_like_an_address(trimmed) {
        let composed = format!("https://{trimmed}");
        found.push(Candidate::new(
            Item::new(format!("Open “{trimmed}” as an address"))
                .with_id("fallback:address".to_string())
                .with_subtitle(composed.clone())
                .with_action(Action::open_url(composed))
                .with_secondary_action(Action::copy(trimmed.to_string())),
            CommandKind::Fallback,
        ));
    }

    found
}

/// Whether a query has the shape of an address a browser is willing to open.
///
/// Honest and narrow: no whitespace, simple host labels with a dot between
/// them — `example.com` yes, `rust closures` no. The subtitle always shows
/// exactly what will open, so a wrong guess is a wrong guess the user can
/// refuse with a glance.
fn looks_like_an_address(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 100
        || trimmed.chars().any(char::is_whitespace)
        || trimmed.chars().any(char::is_control)
    {
        return false;
    }
    let host = trimmed
        .split(['/', '?', '#', '@', ':'])
        .next()
        .unwrap_or("");
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
}

/// Build the snippet expansion candidate when the query is exactly a snippet
/// keyword.
///
/// The expansion is offered as its own result so it can be chosen without
/// leaving the palette.
pub fn snippet_expansion(
    store: &Store,
    query: &str,
) -> yal_store::StoreResult<Option<Candidate>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(None);
    }
    for snippet in store.snippets()? {
        if snippet.keyword.to_lowercase() == needle {
            let item = Item::new(snippet.expansion.clone())
                .with_id(format!("snippet-expansion:{}", snippet.id))
                .with_subtitle(format!("{} — snippet", snippet.name))
                .with_action(Action::paste(snippet.expansion.clone()));
            return Ok(Some(Candidate::new(item, CommandKind::Snippets)));
        }
    }
    Ok(None)
}

/// Percent-encode a string for use in a URL.
fn url_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Standard base64 encoding, used to carry the QR image in a payload.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        encoded.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

/// Whether a calculator value is worth showing.
///
/// A bare `0` is almost never what the user meant to compute, and `2` is more
/// likely to be an application search than an arithmetic result.
fn is_interesting(value: &Value) -> bool {
    match value {
        Value::Scalar(number) => number.abs() >= 1.0,
        _ => true,
    }
}

/// The first line of a text, truncated for display.
fn first_line(text: &str) -> String {
    const MAX: usize = 120;
    let line = text.lines().next().unwrap_or(text);
    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let mut truncated: String = line.chars().take(MAX).collect();
    truncated.push('…');
    truncated
}

/// The icon for an application, derived from its executable path.
fn icon_for(path: &std::path::Path) -> Option<Icon> {
    let extension = path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("exe") {
        Some(Icon::symbol("app"))
    } else {
        None
    }
}

/// The current time, used when no other clock is available.
pub fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_truncates_long_text() {
        let long = "x".repeat(500);
        assert_eq!(first_line(&long).chars().count(), 121);
        assert_eq!(first_line("a\nb"), "a");
    }

    #[test]
    fn calculator_ignores_trivial_values() {
        assert!(calculator("0").is_none());
        assert!(calculator("1 + 1").is_some());
    }

    #[test]
    fn calculator_reports_the_expression() {
        let candidate = calculator("2 + 2").unwrap();
        assert_eq!(candidate.item.title, "4");
        assert_eq!(candidate.item.subtitle.as_deref(), Some("2 + 2 = 4"));
    }

    #[test]
    fn a_reminder_query_offers_a_creation_candidate() {
        let now = Utc::now();
        let candidate = reminder_specification("remind check the oven in 40m", now).unwrap();
        assert_eq!(candidate.item.title, "Reminder - check the oven");
        assert_eq!(candidate.kind, CommandKind::Reminders);
        let action = candidate.item.action.as_ref().unwrap();
        assert_eq!(action.kind, yal_core::action::ActionKind::LauncherTask);
        let yal_core::action::OpenTarget::Task { name, arguments } = &action.target else {
            panic!("the creation candidate should carry the reminder task");
        };
        assert_eq!(name, "reminder");
        let due_at = arguments[0].parse::<i64>().unwrap();
        assert_eq!(
            chrono::DateTime::<Utc>::from_timestamp(due_at, 0)
                .unwrap()
                .timestamp(),
            (now + chrono::Duration::seconds(40 * 60)).timestamp(),
            "the due time lands forty minutes from the words, to the second"
        );
        assert_eq!(arguments[1], "check the oven");
    }

    #[test]
    fn a_query_that_is_not_a_reminder_offers_no_creation() {
        assert!(reminder_specification("remind me later", Utc::now()).is_none());
        assert!(reminder_specification("reset the router", Utc::now()).is_none());
    }

    #[test]
    fn the_screen_is_there_to_be_captured() {
        let commands = system_commands();
        let action_of = |id: &str| {
            commands
                .iter()
                .find(|candidate| candidate.item.id.0 == id)
                .and_then(|candidate| candidate.item.action.clone())
        };

        let area = action_of("system:capture-area").expect("the area capture is offered");
        let yal_core::action::OpenTarget::Task { name, .. } = area.target else {
            panic!("the area capture should be a launcher task");
        };
        assert_eq!(name, "capture-area");

        let whole = action_of("system:screenshot-screen").expect("the whole-screen act is offered");
        let yal_core::action::OpenTarget::Task { name, .. } = whole.target else {
            panic!("the whole-screen act should be a launcher task");
        };
        assert_eq!(name, "screenshot-screen");

        let pick = action_of("system:pick-colour").expect("the colour pick is offered");
        let yal_core::action::OpenTarget::Task { name, .. } = pick.target else {
            panic!("the colour pick should be a launcher task");
        };
        assert_eq!(name, "pick-colour");
    }

    #[test]
    fn a_window_offers_to_be_pictured() {
        let window = yal_system::Window {
            handle: 0x1234,
            title: "Wonderbrew".into(),
            process_id: 42,
            process_name: Some("wonderbrew.exe".into()),
        };
        let candidates = windows(&[window]);
        let focus = &candidates[0].item;
        assert_eq!(focus.title, "Wonderbrew");
        let Some(shot) = focus.secondary_actions.first() else {
            panic!("the focus item should carry the window capture in its panel");
        };
        let yal_core::action::OpenTarget::Task { name, arguments } = &shot.target else {
            panic!("the window capture should be a launcher task");
        };
        assert_eq!(name, "screenshot-window");
        assert_eq!(
            arguments.first().map(String::as_str),
            Some("4660"),
            "the act names its window by handle"
        );
    }

    #[test]
    fn a_note_needs_only_its_text() {
        let candidate = note_specification("note pick up the print out").unwrap();
        assert_eq!(candidate.kind, CommandKind::Notes);
        assert!(candidate.item.title.contains("Take a note"));
        let action = candidate.item.action.as_ref().unwrap();
        assert_eq!(action.kind, yal_core::action::ActionKind::LauncherTask);
        let yal_core::action::OpenTarget::Task { name, arguments } = &action.target else {
            panic!("the note candidate should carry the note task");
        };
        assert_eq!(name, "note-add");
        assert_eq!(arguments[0], "pick up the print out");
    }

    #[test]
    fn a_query_that_is_not_a_note_offers_no_creation() {
        assert!(note_specification("not really a note").is_none());
        assert!(note_specification("note ").is_none());
    }

    #[test]
    fn stored_notes_surface_newest_first_with_a_remove_act() {
        let store = Store::open_in_memory().unwrap();
        let now = Utc::now();
        for (text, minutes) in [("older note", 60), ("newest note", 5)] {
            let record = serde_json::json!({
                "text": text,
                "created_at": (now - chrono::Duration::minutes(minutes)).timestamp(),
            });
            store
                .set_setting(&format!("note:test-{text}"), &record.to_string())
                .unwrap();
        }

        let notes = notes(&store).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].item.title, "newest note", "newest first");
        assert_eq!(
            notes[0].item.action.as_ref().unwrap().kind,
            yal_core::action::ActionKind::CopyToClipboard
        );
        assert_eq!(
            notes[0].item.secondary_actions[0].kind,
            yal_core::action::ActionKind::LauncherTask,
            "the panel offers the removal"
        );
    }

    #[test]
    fn a_multiline_note_tits_itself_by_its_first_line() {
        let store = Store::open_in_memory().unwrap();
        let record = serde_json::json!({
            "text": "shopping list\neggs\nmilk",
            "created_at": Utc::now().timestamp(),
        });
        store.set_setting("note:shopping", &record.to_string()).unwrap();

        let notes = notes(&store).unwrap();
        assert_eq!(notes[0].item.title, "shopping list");
        assert!(
            notes[0].item.subtitle.as_deref().unwrap().contains("eggs milk"),
            "the later lines ride in the subtitle"
        );
        // The copy act carries the WHOLE note, not its first line.
        let yal_core::action::OpenTarget::Text(text) =
            &notes[0].item.action.as_ref().unwrap().target
        else {
            panic!("the note's act should copy text");
        };
        assert_eq!(text, "shopping list\neggs\nmilk");
    }

    #[test]
    fn stored_reminders_surface_soonest_first() {
        let store = Store::open_in_memory().unwrap();
        let now = Utc::now();
        for (text, minutes) in [("later thing", 60), ("sooner thing", 10)] {
            let record = serde_json::json!({
                "text": text,
                "due_at": (now + chrono::Duration::minutes(minutes)).timestamp(),
            });
            store
                .set_setting(&format!("reminder:test-{text}"), &record.to_string())
                .unwrap();
        }

        let reminders = reminders(&store, now).unwrap();
        let texts: Vec<&str> = reminders
            .iter()
            .map(|candidate| candidate.item.title.as_str())
            .collect();
        assert_eq!(texts, vec!["sooner thing", "later thing"]);
        assert!(
            reminders[0].item.subtitle.as_deref().unwrap().contains("due in"),
            "the subtitle should say how far away the reminder sits"
        );
        assert_eq!(
            reminders[0].item.action.as_ref().unwrap().kind,
            yal_core::action::ActionKind::LauncherTask,
            "completing a reminder is a launcher task"
        );
    }

    #[test]
    fn an_overdue_reminder_says_so() {
        let store = Store::open_in_memory().unwrap();
        let now = Utc::now();
        let record = serde_json::json!({
            "text": "left behind",
            "due_at": (now - chrono::Duration::minutes(5)).timestamp(),
        });
        store
            .set_setting("reminder:overdue", &record.to_string())
            .unwrap();

        let reminders = reminders(&store, now).unwrap();
        assert!(
            reminders[0]
                .item
                .subtitle
                .as_deref()
                .unwrap()
                .contains("overdue"),
            "a reminder behind its time says so instead of hiding"
        );
    }
}
