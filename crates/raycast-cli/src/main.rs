//! `rc` — the headless front end.
//!
//! Every command the palette can answer is also answerable from the terminal.
//! That is not a convenience: it is how the behaviour is tested, and how the
//! launcher can be scripted.

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

/// The launcher, driven from the terminal.
#[derive(Parser, Debug)]
#[command(name = "rc", version, about = "A keyboard launcher for Windows")]
struct Cli {
    /// The database to use. Defaults to the per-user store.
    #[arg(long, global = true, value_name = "PATH")]
    database: Option<PathBuf>,

    /// Print results as JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
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
    Color {
        /// A colour name or hexadecimal value.
        value: String,
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
    Screenshot {
        /// Where to write the image.
        #[arg(long, default_value = "screen.png")]
        output: PathBuf,
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

/// Run the requested command.
fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
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

        Command::Color { value } => {
            let color = colors::parse(value)?;
            let hsl = color.to_hsl();
            println!("hex      #{}", color.to_hex());
            println!("rgb      {}", color.to_rgb_string());
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

        Command::Screenshot { output } => {
            let capture = yal_system::capture_screen()
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            yal_system::screenshot::write_png(&capture, output)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
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
                    "{:<8} {:<24} {}",
                    window.process_id,
                    window.process_name.as_deref().unwrap_or("?"),
                    window.title
                );
            }
            println!("{} windows", windows.len());
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
                .unwrap_or_else(|| PathBuf::from("legacy.db"));
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
    }
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

/// A stable identifier derived from a string.
fn stable_id(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
