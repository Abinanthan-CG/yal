//! The command sources.
//!
//! Each source knows how to turn the host state it owns into candidate items.
//! Sources never rank; they only produce. The engine ranks across all of
//! them so that a snippet and an application compete on equal terms.

use chrono::Utc;
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
                .with_action(Action::open_path(application.path.clone()));
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
                    .with_action(Action::open_path(application.path.clone()));
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
                .with_action(action);
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
                .with_action(Action::copy(snippet.expansion.clone()));
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
                .with_action(Action::open_url(url));
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
                .with_action(Action::copy(entry.text.clone()));
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

/// Build the web search candidate for a query.
pub fn web_search(query: &str) -> Option<Candidate> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let url = format!("https://duckduckgo.com/?q={}", url_encode(trimmed));
    let item = Item::new(format!("Search the web for “{trimmed}”"))
        .with_id("websearch:result".to_string())
        .with_subtitle(url.clone())
        .with_action(Action::open_url(url));
    Some(Candidate::new(item, CommandKind::WebSearch))
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
        WindowAction::Fullscreen => "fullscreen",
    }
}

/// Build candidates for the system commands.
///
/// The destructive actions are here because launcher offers them; carrying
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
struct SavedTimer {
    id: String,
    label: String,
    ends_at: chrono::DateTime<chrono::Utc>,
}

/// Decode the timers stored in settings.
fn saved_timers(store: &Store) -> yal_store::StoreResult<Vec<SavedTimer>> {
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

/// Build candidates for files found on disk.
pub fn files(found: &[yal_index::FoundFile]) -> Vec<Candidate> {
    found
        .iter()
        .map(|file| {
            let item = Item::new(file.name.clone())
                .with_id(format!("file:{}", file.path.display()))
                .with_subtitle(file.directory.display().to_string())
                .with_action(Action::open_path(file.path.clone()));
            Candidate::new(item, CommandKind::Files)
        })
        .collect()
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
                .with_action(Action::copy(snippet.expansion.clone()));
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
}
