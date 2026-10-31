//! `yal` — yet another launcher.
//!
//! Run it plain and it is the launcher: the palette window over the local
//! store. Run any of its commands and it is headless, printing what the
//! palette would offer. That is not a convenience: it is how the behaviour
//! is tested, and how the launcher can be scripted.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use yal_calc::format as format_value;
use yal_data::colors;
use yal_data::password::{PasswordOptions, generate};
use yal_engine::Engine;
use yal_index::discover;
use yal_store::Store;

/// The launcher: the palette window, or its headless commands.
#[derive(Parser, Debug)]
#[command(
    name = "yal",
    version,
    about = "YAL — yet another launcher. Run without arguments for the palette window."
)]
struct Cli {
    /// The database to use. Defaults to the per-user store.
    #[arg(long, global = true, value_name = "PATH")]
    database: Option<PathBuf>,

    /// Print results as JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    /// Omitted when the launcher should open its palette window.
    #[command(subcommand)]
    command: Option<Command>,
}

/// The available commands.
#[derive(Subcommand, Debug)]
enum Command {
    /// Search everything and print the ranked results.
    Search {
        /// The query.
        query: String,
        /// How many results to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Evaluate an expression.
    Calc {
        /// The expression to evaluate.
        expression: String,
    },

    /// List the applications discovered on this host.
    Apps {
        /// Only show applications whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
    },

    /// Refresh the application index in the store.
    Index,

    /// Manage text snippets.
    Snippet {
        #[command(subcommand)]
        action: SnippetAction,
    },

    /// Manage quicklinks.
    Quicklink {
        #[command(subcommand)]
        action: QuicklinkAction,
    },

    /// Search the emoji dataset.
    Emoji {
        /// The query.
        query: String,
    },

    /// Show the details of a colour.
    ///
    /// With `--at x,y` the colour is sampled from a pixel of the screen
    /// instead of taken from a value.
    Color {
        /// A colour name or hexadecimal value.
        value: Option<String>,

        /// Sample the colour at `x,y` of the screen.
        #[arg(long = "at")]
        at: Option<String>,
    },

    /// Generate a password.
    Password {
        /// The length of the password.
        #[arg(long, default_value_t = 20)]
        length: usize,
        /// Leave out letters, digits or symbols.
        #[arg(long)]
        no_symbols: bool,
        /// Leave out characters that look alike.
        #[arg(long)]
        avoid_ambiguous: bool,
    },

    /// Capture the screen to a PNG file.
    ///
    /// `--region` and `--window` read a piece of the screen or one
    /// window's own surface instead of the whole primary screen; with
    /// `--clipboard` the pixels ride to the clipboard as a picture.
    Screenshot {
        /// A piece of the screen as `x,y,w,h`, clamped onto the screen.
        #[arg(long = "region")]
        region: Option<String>,

        /// Capture one window's own surface, by its handle from `yal windows`.
        #[arg(long = "window")]
        window: Option<u64>,

        /// Also put the pixels on the clipboard as a picture.
        #[arg(long)]
        clipboard: bool,

        /// Where to write the image.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Check which launcher hotkeys are free on this machine.
    Hotkeys,

    /// Generate a QR code.
    Qr {
        /// The text to encode.
        text: String,
        /// Where to write the image.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Show a time-based one-time password.
    Totp {
        /// The base32 secret.
        secret: String,
    },

    /// Match a regular expression.
    Regex {
        /// The pattern.
        pattern: String,
        /// The text to match against.
        #[arg(default_value = "")]
        subject: String,
    },

    /// List the open windows.
    Windows,

    /// Carry out an action on one window, by its handle from `yal windows`.
    Window {
        /// The window handle.
        handle: u64,
        /// `close`, `minimize`, `maximize`, `restore`, `focus`, `left-half`,
        /// `right-half`, `top-half`, `bottom-half`, a quarter (`top-left-quarter`
        /// and friends), a third (`left-third` and friends), `center` or
        /// `fullscreen`.
        action: String,
    },

    /// Search for files.
    Files {
        /// The text to look for in a file name.
        query: String,
    },

    /// Show or set the launcher hotkey.
    Hotkey {
        /// The hotkey to use, such as `ctrl+space`.
        spec: Option<String>,
    },

    /// Watch the clipboard and record what it sees.
    Watch {
        /// How long to watch, in seconds.
        #[arg(long, default_value_t = 30)]
        seconds: u64,
    },

    /// Register the launcher hotkey and report each press, for testing.
    HotkeyWait {
        /// How long to listen, in seconds.
        #[arg(long, default_value_t = 30)]
        seconds: u64,
        /// The virtual key code to listen for.
        #[arg(long, default_value_t = 0x20)]
        virtual_key: u32,
        /// Require the control key.
        #[arg(long)]
        control: bool,
        /// Require the alt key.
        #[arg(long)]
        alt: bool,
        /// Require the shift key.
        #[arg(long)]
        shift: bool,
    },

    /// Report on the state of the local environment.
    Doctor,

    /// Show or change the master volume.
    Audio {
        #[command(subcommand)]
        action: AudioAction,
    },

    /// Brightness and theme.
    Display {
        #[command(subcommand)]
        action: DisplayAction,
    },

    /// System power and shell commands.
    ///
    /// These act immediately when run.
    System {
        /// What to do: `lock`, `sleep`, `shutdown`, `restart`, `logout`,
        /// `empty-bin`.
        command: String,
    },

    /// List the running processes, heaviest first.
    Processes,

    /// End a running process.
    Kill {
        /// The process identifier.
        pid: u32,
    },

    /// Search the open browser tabs.
    Tabs {
        /// The tab title to look for.
        query: String,
    },

    /// List the installed font families.
    Fonts {
        /// A family name to look for.
        #[arg(default_value = "")]
        query: String,
    },

    /// Show a toast notification.
    Notice {
        /// The heading.
        title: String,
        /// The message.
        #[arg(default_value = "")]
        body: String,
        /// The user model identifier to send from, when the launcher's own
        /// is not trusted yet.
        #[arg(long)]
        app_id: Option<String>,
    },

    /// Start a countdown timer, such as `10m` or `1h 30s`.
    Timer {
        /// The duration tokens: `90`, `10m`, `1h 30m`.
        duration: Vec<String>,
        /// What the timer is for.
        #[arg(default_value = "Timer")]
        label: String,
    },

    /// List the script commands in your scripts folder.
    Scripts,

    /// Run one script command, by its file name or full path.
    ///
    /// The header's mode decides what happens with the output, exactly as
    /// the palette would: in `compact` and `full` the outcome is toasted
    /// as well as printed.
    ScriptRun {
        /// The script's file name, or its path.
        name: String,
        /// Run it detached, the way a `silent` script always runs.
        #[arg(long)]
        detached: bool,
    },

    /// Point the launcher at your scripts folder.
    ScriptsFolder {
        /// The folder path; without one, the default is restored.
        path: Option<String>,
    },

    /// Show the EXIF metadata a photograph carries.
    Exif {
        /// The photograph.
        path: std::path::PathBuf,
    },

    /// List the stored timers with the time left on them.
    Timers,

    /// Cancel a stored timer.
    ///
    /// The identifier is the key the timer is stored under.
    TimerCancel {
        /// The identifier from `yal timers`.
        id: String,
    },

    /// Create a reminder.
    ///
    /// The `when` takes the same words the palette does: `in 40m`,
    /// `at 17:30`, `tomorrow at 9:30`.
    Reminder {
        /// What the reminder is for.
        text: String,
        /// When it comes due. Quoted, so spaces survive: `"in 1h 30m"`.
        when: String,
    },

    /// List the open reminders, soonest first.
    Reminders,

    /// Complete a reminder.
    ///
    /// The identifier is the key the reminder is stored under.
    ReminderDone {
        /// The identifier from `yal reminders`.
        id: String,
    },

    /// Take a note into the local store.
    ///
    /// A reminder needs its due time; a note needs nothing but its text.
    Note {
        /// What the note says. Quote it, so spaces survive.
        text: String,
    },

    /// List the notes in the local store, newest first.
    Notes,

    /// Remove a note.
    NoteRemove {
        /// The identifier from `yal notes`.
        id: String,
    },

    /// Apply a text transform, such as `upper` or `base64encode`.
    Transform {
        /// The transform slug; `yal transforms` lists them.
        slug: String,
        /// The text to transform.
        text: String,
    },

    /// List every text transform.
    Transforms,

    /// Hash text.
    Hash {
        /// The text to hash.
        text: String,
        /// `sha256`, `sha1` or `md5`.
        #[arg(default_value = "sha256")]
        algorithm: String,
    },

    /// Generate an identifier.
    #[command(name = "uuid")]
    UuidGenerate,

    /// Generate placeholder text.
    Lorem {
        /// How many paragraphs.
        #[arg(default_value_t = 2)]
        paragraphs: usize,
    },

    /// Seed the bundled search templates into the store, once.
    SeedQuicklinks,

    /// Register the launcher's Start Menu identity, once.
    ///
    /// A shortcut to this program, with its user model identifier set, is
    /// how the shell learns the launcher's notifications. After this, the
    /// toast banners are delivered the way a packaged application's are.
    Identify,

    /// Rename an application with a short word of yours.
    ///
    /// Typing the term in the palette then finds the application by its
    /// alias. The target is matched against the installed applications
    /// by name.
    Alias {
        #[command(subcommand)]
        action: AliasAction,
    },
}

/// Alias operations.
#[derive(Subcommand, Debug)]
enum AliasAction {
    /// Set `term` to launch the application named `target`.
    Set {
        /// The word that finds it.
        term: String,
        /// The application's name.
        target: String,
    },
    /// List your aliases.
    List,
    /// Remove an alias.
    Remove {
        /// The word to forget.
        term: String,
    },
}

/// Volume operations.
#[derive(Subcommand, Debug)]
enum AudioAction {
    /// Read the master volume and mute state.
    Get,
    /// Set the volume, 0 to 100.
    Set {
        #[arg(value_name = "LEVEL")]
        level: f64,
    },
    /// Mute the output.
    Mute,
    /// Unmute the output.
    Unmute,
    /// Mute or unmute, whichever changes the state.
    Toggle,
}

/// Display operations.
#[derive(Subcommand, Debug)]
enum DisplayAction {
    /// Read the brightness of every controllable monitor.
    Brightness {
        /// A new level from 0 to 100; without it, read only.
        #[arg(value_name = "LEVEL")]
        level: Option<f64>,
    },
    /// Read or set the application theme.
    Dark {
        /// `on`, `off` or nothing to report the state.
        state: Option<String>,
    },
}

/// Snippet operations.
#[derive(Subcommand, Debug)]
enum SnippetAction {
    /// Add or replace a snippet.
    Add {
        /// A display name.
        name: String,
        /// The word that expands the snippet.
        keyword: String,
        /// The text the snippet expands to.
        expansion: String,
    },
    /// List the stored snippets.
    List,
    /// Remove a snippet by identifier.
    Remove {
        /// The snippet identifier.
        id: String,
    },
}

/// Quicklink operations.
#[derive(Subcommand, Debug)]
enum QuicklinkAction {
    /// Add or replace a quicklink.
    Add {
        /// A display name.
        name: String,
        /// The URL to open.
        url: String,
    },
    /// List the stored quicklinks.
    List,
    /// Remove a quicklink by identifier.
    Remove {
        /// The quicklink identifier.
        id: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Run the requested command. With none, open the palette window.
fn run(cli: &Cli) -> Result<()> {
    let Some(command) = &cli.command else {
        return run_palette();
    };
    match command {
        Command::Search { query, limit } => {
            let store = open_store(cli)?;
            let applications = discover();
            let mut engine = Engine::new().with_limit(*limit);
            let results = engine.search(&store, &applications, query, chrono::Utc::now())?;
            if cli.json {
                let payload: Vec<_> = results
                    .iter()
                    .map(|result| {
                        serde_json::json!({
                            "title": result.item.title,
                            "subtitle": result.item.subtitle,
                            "kind": result.kind.section_title(),
                            "score": result.score,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                for (index, result) in results.iter().enumerate() {
                    println!(
                        "{:>3}. [{:>16}] {}",
                        index + 1,
                        result.kind.section_title(),
                        result.item.title
                    );
                    if let Some(subtitle) = &result.item.subtitle {
                        println!("     {subtitle}");
                    }
                }
            }
        }

        Command::Calc { expression } => {
            let value = yal_calc::evaluate(expression)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("{}", format_value(&value));
        }

        Command::Apps { filter } => {
            let applications = discover();
            for application in &applications {
                if let Some(filter) = filter {
                    if !application
                        .name
                        .to_lowercase()
                        .contains(&filter.to_lowercase())
                    {
                        continue;
                    }
                }
                println!(
                    "{:<40} {:<10} {}",
                    application.name,
                    application.source.as_str(),
                    application.path.display()
                );
            }
            eprintln!("{} applications discovered", applications.len());
        }

        Command::Index => {
            let store = open_store(cli)?;
            let applications = discover();
            let now = chrono::Utc::now();
            let mut ids = Vec::with_capacity(applications.len());
            for application in &applications {
                ids.push(application.id.clone());
                store.upsert_application(&yal_store::IndexedApplication {
                    id: application.id.clone(),
                    name: application.name.clone(),
                    path: application.path.display().to_string(),
                    source: application.source.as_str().to_string(),
                    keywords: application.keywords.join(" "),
                    last_indexed: now,
                })?;
            }
            store.prune_applications(&ids)?;
            println!("indexed {} applications", applications.len());
        }

        Command::Snippet { action } => {
            let store = open_store(cli)?;
            match action {
                SnippetAction::Add {
                    name,
                    keyword,
                    expansion,
                } => {
                    let now = chrono::Utc::now();
                    let id = format!("{:x}", stable_id(keyword));
                    store.upsert_snippet(&yal_store::Snippet {
                        id,
                        name: name.clone(),
                        keyword: keyword.clone(),
                        expansion: expansion.clone(),
                        created_at: now,
                        updated_at: now,
                    })?;
                    println!("saved snippet `{keyword}`");
                }
                SnippetAction::List => {
                    for snippet in store.snippets()? {
                        println!("{:<20} {:<20} {}", snippet.id, snippet.keyword, snippet.name);
                    }
                }
                SnippetAction::Remove { id } => {
                    store.delete_snippet(id)?;
                    println!("removed snippet {id}");
                }
            }
        }

        Command::Quicklink { action } => {
            let store = open_store(cli)?;
            match action {
                QuicklinkAction::Add { name, url } => {
                    let now = chrono::Utc::now();
                    let id = format!("{:x}", stable_id(url));
                    store.upsert_quicklink(&yal_store::Quicklink {
                        id,
                        name: name.clone(),
                        url: url.clone(),
                        created_at: now,
                    })?;
                    println!("saved quicklink `{name}`");
                }
                QuicklinkAction::List => {
                    for quicklink in store.quicklinks()? {
                        println!("{:<20} {:<40} {}", quicklink.id, quicklink.name, quicklink.url);
                    }
                }
                QuicklinkAction::Remove { id } => {
                    store.delete_quicklink(id)?;
                    println!("removed quicklink {id}");
                }
            }
        }

        Command::Emoji { query } => {
            for emoji in yal_data::emoji::search(query, 50) {
                println!("{}\t{}", emoji.character, emoji.name);
            }
        }

        Command::Color { value, at } => {
            let colour = if let Some(at) = at {
                let (x, y) = parse_at(at)?;
                let capture = yal_system::screenshot::capture_region(x, y, 1, 1)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                let pixel = capture.image.get_pixel(0, 0);
                colors::Color::new(pixel[0], pixel[1], pixel[2])
            } else if let Some(value) = value {
                colors::parse(value)?
            } else {
                return Err(anyhow::anyhow!(
                    "give a colour to describe, or `--at x,y` to take one from the screen"
                ));
            };
            let hsl = colour.to_hsl();
            println!("hex      #{}", colour.to_hex());
            println!("rgb      {}", colour.to_rgb_string());
            println!(
                "hsl      hsl({:.0}, {:.0}%, {:.0}%)",
                hsl.hue,
                hsl.saturation * 100.0,
                hsl.lightness * 100.0
            );
        }

        Command::Password {
            length,
            no_symbols,
            avoid_ambiguous,
        } => {
            let options = PasswordOptions {
                length: *length,
                symbols: !no_symbols,
                avoid_ambiguous: *avoid_ambiguous,
                ..Default::default()
            };
            let password = generate(&options)
                .ok_or_else(|| anyhow::anyhow!("no character classes are enabled"))?;
            println!("{password}");
        }

        Command::Screenshot {
            region,
            window,
            clipboard,
            output,
        } => {
            if window.is_some() && region.is_some() {
                return Err(anyhow::anyhow!(
                    "a screenshot is either `--region` or `--window`, not both"
                ));
            }
            let capture =
                if let Some(region) = region {
                    let (x, y, width, height) =
                        yal_system::screenshot::parse_region(region).map_err(|err| anyhow::anyhow!("{err}"))?;
                    yal_system::screenshot::capture_region(x, y, width, height)
                } else if let Some(window) = window {
                    yal_system::screenshot::capture_window(*window)
                } else {
                    yal_system::capture_screen()
                }
                .map_err(|err| anyhow::anyhow!("{err}"))?;

            let output = match output {
                Some(path) => path.clone(),
                None => yal_system::screenshot::default_capture_path(),
            };
            yal_system::screenshot::write_png(&capture, &output)
                .map_err(|err| anyhow::anyhow!("{err}"))?;

            if *clipboard {
                let mut clipboard = yal_system::Clipboard::new()
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                clipboard
                    .write_image(
                        capture.image.as_raw(),
                        capture.width as usize,
                        capture.height as usize,
                    )
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("the pixels are on the clipboard");
            }
            println!(
                "captured {}x{} to {}",
                capture.width,
                capture.height,
                output.display()
            );
        }

        Command::Hotkeys => {
            // Registration is per thread and needs a message queue, so the
            // checks run on their own thread.
            let candidates: Vec<(&str, u32, yal_system::hotkey::HotkeyModifiers)> = vec![
                (
                    "Ctrl+Space",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        ..Default::default()
                    },
                ),
                (
                    "Ctrl+Alt+Space",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        alt: true,
                        ..Default::default()
                    },
                ),
                (
                    "Alt+Space",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        alt: true,
                        ..Default::default()
                    },
                ),
                (
                    "Ctrl+Shift+Space",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        shift: true,
                        ..Default::default()
                    },
                ),
                (
                    "Ctrl+Alt+K",
                    0x4B,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        alt: true,
                        ..Default::default()
                    },
                ),
                (
                    "Ctrl+Shift+K",
                    0x4B,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        shift: true,
                        ..Default::default()
                    },
                ),
                (
                    "Win+Space",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        super_key: true,
                        ..Default::default()
                    },
                ),
                (
                    "Ctrl+Shift+Space (2)",
                    0x20,
                    yal_system::hotkey::HotkeyModifiers {
                        control: true,
                        shift: true,
                        ..Default::default()
                    },
                ),
            ];

            for (label, key, modifiers) in candidates {
                let ok = std::thread::spawn(move || {
                    yal_system::hotkey::Hotkey::register(key, modifiers, 4242).is_ok()
                })
                .join()
                .unwrap_or(false);
                println!(
                    "{:<22} {}",
                    label,
                    if ok { "available" } else { "taken" }
                );
            }
        }

        Command::Qr { text, output } => {
            let code = yal_data::qr::generate(text, 8)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            match output {
                Some(path) => {
                    let (width, height) = (code.width, code.height);
                    let destination = path.display().to_string();
                    std::fs::write(path, code.png)
                        .map_err(|err| anyhow::anyhow!("{err}"))?;
                    println!("wrote {width}x{height} QR code to {destination}");
                }
                None => {
                    // Without a file to write to, the code is printed as the
                    // text it encodes so the command stays useful.
                    println!("{}", code.png.len());
                    println!("text: {text}");
                }
            }
        }

        Command::Totp { secret } => {
            let parsed = yal_data::totp::TotpSecret::parse(secret)
                .ok_or_else(|| anyhow::anyhow!("`{secret}` is not a base32 secret"))?;
            let now = chrono::Utc::now().timestamp() as u64;
            let (code, remaining) = parsed.code_and_remaining(
                now,
                yal_data::totp::DEFAULT_DIGITS,
                yal_data::totp::DEFAULT_PERIOD,
            );
            println!("{code}  ({remaining}s remaining)");
        }

        Command::Regex { pattern, subject } => {
            let result = yal_data::regex::match_pattern(pattern, subject);
            if let Some(error) = result.error {
                anyhow::bail!("{error}");
            }
            for matched in &result.matches {
                let groups: Vec<String> = matched
                    .groups
                    .iter()
                    .map(|group| match group {
                        Some(value) => value.clone(),
                        None => "-".to_string(),
                    })
                    .collect();
                println!(
                    "{:>4}..{:<4}  {:<30}  [{}]",
                    matched.start,
                    matched.end,
                    matched.text,
                    groups.join(", ")
                );
            }
            println!("{}", yal_data::regex::summarise(&result));
        }

        Command::Windows => {
            let windows = yal_system::list_windows()
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            for window in &windows {
                println!(
                    "{:<10} {:<8} {:<24} {}",
                    window.handle,
                    window.process_id,
                    window.process_name.as_deref().unwrap_or("?"),
                    window.title
                );
            }
            println!("{} windows", windows.len());
        }

        Command::Window { handle, action } => {
            use yal_system::window_controls::{self, WindowAction};
            if action.to_lowercase() == "rect" {
                let (x, y, width, height) = window_controls::rect(*handle)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("x {x}  y {y}  width {width}  height {height}");
                return Ok(());
            }
            let action = match action.to_lowercase().as_str() {
                "close" => WindowAction::Close,
                "minimize" => WindowAction::Minimize,
                "maximize" => WindowAction::Maximize,
                "restore" => WindowAction::Restore,
                "focus" => WindowAction::Focus,
                "left-half" => WindowAction::LeftHalf,
                "right-half" => WindowAction::RightHalf,
                "top-half" => WindowAction::TopHalf,
                "bottom-half" => WindowAction::BottomHalf,
                "top-left-quarter" => WindowAction::TopLeftQuarter,
                "top-right-quarter" => WindowAction::TopRightQuarter,
                "bottom-left-quarter" => WindowAction::BottomLeftQuarter,
                "bottom-right-quarter" => WindowAction::BottomRightQuarter,
                "left-third" => WindowAction::LeftThird,
                "middle-third" => WindowAction::CenterThird,
                "right-third" => WindowAction::RightThird,
                "center" => WindowAction::Center,
                "fullscreen" => WindowAction::Fullscreen,
                other => anyhow::bail!(
                    "`{other}` is not a window action (close, minimize, maximize, restore, \
                     focus, left-half, right-half, top-half, bottom-half, a quarter or third, \
                     center, fullscreen)"
                ),
            };
            window_controls::perform(*handle, action)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("{action:?} carried out on window {handle}");
        }

        Command::Files { query } => {
            let roots = yal_index::default_search_roots();
            let found = yal_index::search_files(&roots, query, 50, 20_000);
            for file in &found {
                println!("{:<40} {}", file.name, file.directory.display());
            }
            println!("{} files", found.len());
        }

        Command::Hotkey { spec } => {
            let store = open_store(cli)?;
            match spec {
                Some(text) => {
                    let parsed = yal_system::hotkey::HotkeySpec::parse(text)
                        .ok_or_else(|| {
                            anyhow::anyhow!("`{text}` is not a hotkey, such as `ctrl+space`")
                        })?;
                    let text = parsed.to_text();
                    store.set_setting("hotkey", &text)?;
                    println!("hotkey set to {text}");
                }
                None => {
                    let current = store
                        .setting("hotkey")
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| {
                            yal_system::hotkey::HotkeySpec::default_spec().to_text()
                        });
                    println!("hotkey: {current}");
                }
            }
        }

        Command::Watch { seconds } => {
            let path = cli
                .database
                .clone()
                .or_else(yal_store::default_path)
                .context("could not determine a database location")?;
            let watcher = yal_system::watcher::ClipboardWatcher::start(
                Some(path),
                std::time::Duration::from_millis(400),
                500,
                |entry| {
                    let first = entry.text.lines().next().unwrap_or("");
                    let first: String = first.chars().take(80).collect();
                    println!("clipboard: {first}");
                },
            );
            if watcher.is_none() {
                anyhow::bail!("the clipboard could not be opened");
            }
            println!("watching the clipboard for {seconds}s");
            std::thread::sleep(std::time::Duration::from_secs(*seconds));
        }

        Command::HotkeyWait {
            seconds,
            virtual_key,
            control,
            alt,
            shift,
        } => {
            use yal_system::hotkey::{HotkeyListener, HotkeyModifiers};
            let listener = HotkeyListener::spawn(
                *virtual_key,
                HotkeyModifiers {
                    control: *control,
                    alt: *alt,
                    shift: *shift,
                    super_key: false,
                },
                0x5241,
            );
            println!("registered: {}", listener.is_available());
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(*seconds);
            let mut presses = 0u32;
            while std::time::Instant::now() < deadline {
                if listener.poll() {
                    presses += 1;
                    println!("press {presses}");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            println!("total presses: {presses}");
        }

        Command::Doctor => {
            let applications = discover();
            let database = cli
                .database
                .clone()
                .or_else(yal_store::default_path)
                .unwrap_or_else(|| PathBuf::from("yal.db"));
            println!("platform       {}", std::env::consts::OS);
            println!("database       {}", database.display());
            println!("applications   {}", applications.len());
            println!(
                "store writable {}",
                if database.exists() { "yes" } else { "not created yet" }
            );
            println!(
                "emoji          {}",
                yal_data::emoji::all().len()
            );
            println!(
                "time zones     {}",
                yal_data::all_time_zones().len()
            );
        }

        Command::Audio { action } => match action {
            AudioAction::Get => {
                let state = yal_system::audio::volume()
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!(
                    "volume {:.0}{}",
                    state.level,
                    if state.muted { " (muted)" } else { "" }
                );
            }
            AudioAction::Set { level } => {
                yal_system::audio::set_volume(*level)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("volume set to {level:.0}");
            }
            AudioAction::Mute => {
                yal_system::audio::set_muted(true)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("muted");
            }
            AudioAction::Unmute => {
                yal_system::audio::set_muted(false)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("unmuted");
            }
            AudioAction::Toggle => {
                let state = yal_system::audio::toggle_mute()
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!(
                    "volume {:.0}{}",
                    state.level,
                    if state.muted { " (muted)" } else { "" }
                );
            }
        },

        Command::Display { action } => match action {
            DisplayAction::Brightness { level } => {
                if let Some(level) = level {
                    yal_system::display::set_brightness(*level)
                        .map_err(|err| anyhow::anyhow!("{err}"))?;
                }
                let readings =
                    yal_system::display::read_brightness()
                        .map_err(|err| anyhow::anyhow!("{err}"))?;
                for (monitor, percent) in readings {
                    println!("monitor {monitor}: {:.0}", percent);
                }
            }
            DisplayAction::Dark { state } => {
                match state.as_deref().map(str::to_lowercase).as_deref() {
                    Some("on") | Some("dark") => {
                        yal_system::display::set_dark_mode(true)
                            .map_err(|err| anyhow::anyhow!("{err}"))?;
                        println!("applications now dark");
                    }
                    Some("off") | Some("light") => {
                        yal_system::display::set_dark_mode(false)
                            .map_err(|err| anyhow::anyhow!("{err}"))?;
                        println!("applications now light");
                    }
                    _ => {
                        let dark = yal_system::display::is_dark_mode()
                            .map_err(|err| anyhow::anyhow!("{err}"))?;
                        println!("{}", if dark { "dark" } else { "light" });
                    }
                }
            }
        },

        Command::System { command } => {
            use yal_system::system as power_system;
            let outcome = match command.as_str() {
                "lock" => power_system::lock().map(|_| "locked the session".to_string()),
                "sleep" => power_system::sleep().map(|_| "sleeping".to_string()),
                "shutdown" => power_system::shutdown().map(|_| "shutdown begins in 5s".to_string()),
                "restart" => power_system::restart().map(|_| "restart begins in 5s".to_string()),
                "logout" => power_system::log_out().map(|_| "logged out".to_string()),
                "empty-bin" => {
                    power_system::empty_recycle_bin().map(|_| "recycle bin emptied".to_string())
                }
                other => anyhow::bail!(
                    "`{other}` is not a system command (lock, sleep, shutdown, restart, logout, empty-bin)"
                ),
            };
            println!("{}", outcome?);
        }

        Command::Processes => {
            let running = yal_system::processes::running();
            for process in &running {
                println!(
                    "{:<8} {:<28} {:.0} MB",
                    process.process_id,
                    process.name,
                    process.memory as f64 / 1_048_576.0
                );
            }
            println!("{} processes", running.len());
        }

        Command::Kill { pid } => {
            yal_system::processes::terminate(*pid)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("asked process {pid} to end");
        }

        Command::Tabs { query } => {
            let tabs = yal_system::tabs::open_tabs();
            let needle = query.to_lowercase();
            for tab in &tabs {
                if tab.title.to_lowercase().contains(&needle)
                    || tab.browser.to_lowercase().contains(&needle)
                {
                    println!("{:<20} {}", tab.browser, tab.title);
                }
            }
            println!("{} tabs", tabs.len());
        }

        Command::Fonts { query } => {
            let families = yal_system::fonts::families();
            let needle = query.to_lowercase();
            let shown: Vec<_> = families
                .iter()
                .filter(|font| {
                    needle.is_empty() || font.family.to_lowercase().contains(&needle)
                })
                .collect();
            for font in &shown {
                println!("{}", font.family);
            }
            println!("{} font families of {}", shown.len(), families.len());
        }

        Command::Notice { title, body, app_id } => {
            // Reports honestly: on a host that refuses toasts from this
            // launcher, the failure is the answer.
            match app_id {
                Some(app_id) => yal_system::notify::toast_with(app_id, title, body),
                None => yal_system::notify::toast(title, body),
            }
            .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("toast accepted: {title}");
        }

        Command::ScriptRun { name, detached } => {
            let folder = yal_system::scripts::resolve_folder(
                open_store(cli)?
                    .setting("scripts_folder")?
                    .map(std::path::PathBuf::from),
            );
            // By full path when it reads as one; otherwise by file stem or
            // title among the folder's own scripts.
            let script = yal_system::scripts::read(&std::path::PathBuf::from(name))
                .or_else(|| {
                    folder.and_then(|folder| {
                        yal_system::scripts::list(&folder)
                            .into_iter()
                            .find(|script| {
                                script
                                    .path
                                    .file_stem()
                                    .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(name))
                                    || script.title.eq_ignore_ascii_case(name)
                            })
                    })
                })
                .ok_or_else(|| anyhow::anyhow!("`{name}` is not a script command in your scripts folder"))?;

            if *detached || script.mode == yal_system::scripts::Mode::Silent {
                let (program, args) = yal_system::scripts::runner_command(&script);
                yal_system::launch::launch_program(&program.to_string_lossy(), &args)
                    .map_err(|err| anyhow::anyhow!("{err}"))?;
                println!("{} started detached", script.title);
                return Ok(());
            }

            let output = yal_system::scripts::run(&script)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !stdout.is_empty() {
                println!("{stdout}");
            }
            if !stderr.is_empty() {
                println!("(on stderr) {stderr}");
            }
            println!("exit status: {:?}", output.status.code());

            let summary = if output.status.success() {
                format!("{} finished", script.title)
            } else {
                format!("{} failed", script.title)
            };
            let detail = stdout
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let _ = yal_system::notify::toast(&summary, &detail);
        }

        Command::Scripts => {
            let store = open_store(cli)?;
            let folder = yal_system::scripts::resolve_folder(
                store
                    .setting("scripts_folder")?
                    .map(std::path::PathBuf::from),
            );
            let Some(folder) = folder else {
                anyhow::bail!("there is no scripts folder on this machine");
            };
            let scripts = yal_system::scripts::list(&folder);
            for script in &scripts {
                println!(
                    "{:<24} {:<8} {}",
                    script.title,
                    script.mode.slug(),
                    script.path.display()
                );
            }
            println!("{} script commands in {}", scripts.len(), folder.display());
        }

        Command::ScriptsFolder { path } => {
            let store = open_store(cli)?;
            match path {
                Some(path) => {
                    std::fs::create_dir_all(path)?;
                    store.set_setting("scripts_folder", path)?;
                    println!("script commands are read from {path} from now on");
                }
                None => {
                    store.clear_setting("scripts_folder")?;
                    println!("restored the default scripts folder");
                }
            }
        }

        Command::Exif { path } => {
            let Some(metadata) = yal_system::exif::read(path) else {
                anyhow::bail!(
                    "{} carries no EXIF metadata this launcher can read",
                    path.display()
                );
            };
            let lines = metadata.lines();
            if lines.is_empty() {
                println!("{}: the file is a JPEG without metadata fields", path.display());
                return Ok(());
            }
            for (label, value) in &lines {
                println!("{:<14} {}", format!("{label}:"), value);
            }
        }

        Command::Timer { duration, label } => {
            let token = duration.join(" ");
            let seconds = duration_seconds(&token)
                .ok_or_else(|| anyhow::anyhow!("`{token}` is not a duration, such as `10m`"))?;
            let store = open_store(cli)?;
            let id = chrono::Utc::now().timestamp_millis().to_string();
            let ends_at = chrono::Utc::now() + chrono::Duration::seconds(seconds as i64);
            let record = serde_json::json!({
                "label": label,
                "ends_at": ends_at.timestamp(),
                "seconds": seconds,
            });
            store.set_setting(&format!("timer:{id}"), &record.to_string())?;
            println!(
                "timer `{label}` stored as {} starts now",
                id
            );
            yal_system::notify::toast(
                label,
                &format!("Timer set for {}.", shown_duration(seconds)),
            )
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        }

        Command::Timers => {
            let store = open_store(cli)?;
            let now = chrono::Utc::now();
            let timers = yal_engine::sources::saved_timers(&store)?;
            for timer in &timers {
                if timer.ends_at > now {
                    println!(
                        "{:<20} {:<20} {}s remaining",
                        timer.id,
                        timer.label,
                        (timer.ends_at - now).num_seconds()
                    );
                } else {
                    println!("{:<20} {:<20} finished", timer.id, timer.label);
                    // The run is over; nothing can un-finish it, so the
                    // record leaves with the listing.
                    let _ = store.clear_setting(&format!("timer:{}", timer.id));
                }
            }
            println!("{} stored timers", timers.len());
        }

        Command::TimerCancel { id } => {
            let store = open_store(cli)?;
            store.clear_setting(&format!("timer:{id}"))?;
            println!("timer {id} cancelled");
        }

        Command::Reminder { text, when } => {
            let store = open_store(cli)?;
            let words = format!("remind {text} {when}");
            let spec = yal_data::remind::parse(&words, chrono::Utc::now()).ok_or_else(
                || {
                    anyhow::anyhow!(
                        "`{words}` is not a reminder, such as `yal reminder \"call the dentist\" \"in 20m\"`"
                    )
                },
            )?;
            let id = chrono::Utc::now().timestamp_millis().to_string();
            let record = serde_json::json!({
                "text": spec.text,
                "due_at": spec.due_at.timestamp(),
            });
            store.set_setting(&format!("reminder:{id}"), &record.to_string())?;
            let raised = spec
                .due_at
                .with_timezone(&chrono::Local)
                .format("%a %H:%M");
            println!("reminder `{}` stored as {id}", spec.text);
            println!("raised {raised}");
            yal_system::notify::toast("Reminder", &format!("A reminder is set for {raised}."))
                .map_err(|err| anyhow::anyhow!("{err}"))?;
        }

        Command::Reminders => {
            let store = open_store(cli)?;
            let now = chrono::Utc::now();
            let mut reminders = yal_engine::sources::saved_reminders(&store)?;
            reminders.sort_by_key(|reminder| reminder.due_at);
            for reminder in &reminders {
                let raised = reminder
                    .due_at
                    .with_timezone(&chrono::Local)
                    .format("%a %H:%M");
                if reminder.due_at > now {
                    println!("{:<20} {:<26} due {raised}", reminder.id, reminder.text);
                } else {
                    println!(
                        "{:<20} {:<26} overdue (due {raised})",
                        reminder.id, reminder.text
                    );
                    // Past its time, the reminder leaves with the
                    // listing, the same act the palette's watcher does.
                    let _ = store.clear_setting(&format!("reminder:{}", reminder.id));
                }
            }
            println!("{} open reminders", reminders.len());
        }

        Command::ReminderDone { id } => {
            let store = open_store(cli)?;
            store.clear_setting(&format!("reminder:{id}"))?;
            println!("reminder {id} completed");
        }

        Command::Note { text } => {
            let store = open_store(cli)?;
            let id = chrono::Utc::now().timestamp_millis().to_string();
            let record = serde_json::json!({
                "text": text,
                "created_at": chrono::Utc::now().timestamp(),
            });
            store.set_setting(&format!("note:{id}"), &record.to_string())?;
            println!("note {id} taken");
        }

        Command::Notes => {
            let store = open_store(cli)?;
            let mut notes = yal_engine::sources::saved_notes(&store)?;
            notes.sort_by_key(|note| note.created_at);
            notes.reverse();
            for note in &notes {
                let taken = note
                    .created_at
                    .with_timezone(&chrono::Local)
                    .format("%d %b %H:%M");
                let first = note.text.lines().next().unwrap_or("");
                println!("{:<20} {:<18} {first}", note.id, taken);
            }
            println!("{} notes", notes.len());
        }

        Command::NoteRemove { id } => {
            let store = open_store(cli)?;
            store.clear_setting(&format!("note:{id}"))?;
            println!("note {id} removed");
        }

        Command::Transform { slug, text } => {
            let result = yal_data::text::apply(slug, text)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("{result}");
        }

        Command::Transforms => {
            for transform in yal_data::text::TRANSFORMS {
                println!("{:<16} {}", transform.slug, transform.title);
            }
        }

        Command::Hash { algorithm, text } => {
            let digest = match algorithm.to_lowercase().as_str() {
                "sha256" | "sha-256" => yal_data::text::Digest::Sha256,
                "sha1" | "sha-1" => yal_data::text::Digest::Sha1,
                "md5" => yal_data::text::Digest::Md5,
                other => anyhow::bail!("`{other}` is not a digest (sha256, sha1, md5)"),
            };
            println!("{}", yal_data::text::hash(digest, text)?);
        }

        Command::UuidGenerate => {
            println!("{}", yal_data::text::uuid());
        }

        Command::Lorem { paragraphs } => {
            println!("{}", yal_data::text::lorem_ipsum(*paragraphs));
        }

        Command::SeedQuicklinks => {
            let store = open_store(cli)?;
            store.seed_default_quicklinks()?;
            println!("quicklinks bundled with the launcher are stored");
        }

        Command::Identify => {
            let target = std::env::current_exe()
                .map_err(|err| anyhow::anyhow!("the running program could not be located: {err}"))?;
            yal_system::identity::register_shortcut(
                yal_system::notify::APPLICATION_ID,
                "yal",
                &target,
            )
            .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!(
                "the Start Menu shortcut for {} is written from {}",
                yal_system::notify::APPLICATION_ID,
                target.display()
            );
        }

        Command::Alias { action } => {
            use yal_store::records::Alias;
            match action {
                AliasAction::Set { term, target } => {
                    anyhow::ensure!(
                        !term.is_empty() && !target.is_empty(),
                        "both the term and the target are needed"
                    );
                    let store = open_store(cli)?;
                    store.upsert_alias(&Alias {
                        id: format!("alias-{}", term.to_lowercase()),
                        term: term.clone(),
                        target: target.clone(),
                        created_at: chrono::Utc::now(),
                    })?;
                    println!("`{term}` now opens the application named `{target}`");
                }
                AliasAction::List => {
                    let store = open_store(cli)?;
                    for alias in store.aliases()? {
                        println!("{:<16} -> {}", alias.term, alias.target);
                    }
                }
                AliasAction::Remove { term } => {
                    let store = open_store(cli)?;
                    store.clear_alias(&format!("alias-{}", term.to_lowercase()))?;
                    println!("alias `{term}` removed");
                }
            }
        }
    }
    Ok(())
}

/// Open the palette window over the per-user store.
fn run_palette() -> Result<()> {
    let store = match yal_store::default_path() {
        Some(path) => Store::open(&path),
        None => Store::open_in_memory(),
    }?;
    // eframe's error carries a `Box<dyn Error>` that is not `Send`, so it
    // cannot fold into `anyhow` with `?`; the text is all a caller needs.
    yal_ui::run(store).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

/// Open the store at the requested location.
fn open_store(cli: &Cli) -> Result<Store> {
    let path = cli
        .database
        .clone()
        .or_else(yal_store::default_path)
        .context("could not determine a database location")?;
    Store::open(&path).map_err(Into::into)
}

/// A screen point such as `x,y`.
fn parse_at(text: &str) -> Result<(i32, i32)> {
    let (x, y) = text
        .split_once(',')
        .ok_or_else(|| anyhow::anyhow!("`--at` wants `x,y`, got `{text}`"))?;
    let x = x
        .trim()
        .parse::<i32>()
        .map_err(|_| anyhow::anyhow!("`--at` wants `x,y`, got `{text}`"))?;
    let y = y
        .trim()
        .parse::<i32>()
        .map_err(|_| anyhow::anyhow!("`--at` wants `x,y`, got `{text}`"))?;
    Ok((x, y))
}

/// A stable identifier derived from a string.
fn stable_id(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Duration tokens such as `90`, `10m`, `1h 30s` into seconds.
fn duration_seconds(token: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut any = false;
    for part in token.split_whitespace() {
        let split = part
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(part.len());
        let (digits, unit) = part.split_at(split);
        let multiplier = match unit.to_lowercase().as_str() {
            "" | "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => return None,
        };
        let amount = digits.parse::<u64>().ok()?;
        total += amount * multiplier;
        any = true;
    }
    if any && total > 0 && total <= 24 * 3_600 {
        Some(total)
    } else {
        None
    }
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
