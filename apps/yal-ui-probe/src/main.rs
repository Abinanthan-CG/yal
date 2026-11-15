//! A probe for the native palette.
//!
//! It opens the palette window with a fixed set of rows, prints how long it
//! took to get there, and stays open so the footprint can be measured and the
//! window can be photographed. Press escape to close it.
//!
//! ```text
//! yal-ui-probe                     # the dark appearance
//! yal-ui-probe --light             # the light appearance
//! yal-ui-probe --style-pack <file> # a design system's tokens
//! ```

use std::path::PathBuf;
use std::time::Instant;

use tc_ui::{Color, Icon, Model, Palette, Row, Theme, Tokens};

fn main() {
    let started = Instant::now();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let style_pack = style_pack_path(&arguments);

    let (theme, name) = match style_pack {
        Some(path) => match Tokens::load(&path) {
            Ok(tokens) => {
                let name = tokens.name.clone();
                (tokens.theme(), name)
            }
            Err(error) => {
                eprintln!("the style pack could not be worn: {error}");
                std::process::exit(1);
            }
        },
        None => {
            let light = arguments.iter().any(|argument| argument == "--light");
            let theme = if light {
                Theme::light()
            } else {
                Theme::dark()
            };
            let name = if light { "light" } else { "dark" }.to_string();
            (theme, name)
        }
    };

    let blue = Color::rgb(0x0a, 0x63, 0xd8);
    let green = Color::rgb(0x2f, 0xa3, 0x6a);
    let orange = Color::rgb(0xe0, 0x7b, 0x28);
    let purple = Color::rgb(0x7a, 0x4f, 0xd6);
    let teal = Color::rgb(0x1c, 0x93, 0xa8);
    let red = Color::rgb(0xd6, 0x3b, 0x3b);

    let model = Model {
        query: String::new(),
        rows: vec![
            Row::detailed("Calculator", "2 + 2 × 7 = 16").with_icon(Icon::new("C", blue)),
            Row::detailed("Weather in Kochi", "31° · Light rain").with_icon(Icon::new("W", teal)),
            Row::detailed("Flights to Tokyo", "NH 8 · 06:40 · 11h 20m")
                .with_icon(Icon::new("F", orange)),
            Row::detailed("Search the web for \"rust windows\"", "Google")
                .with_icon(Icon::new("G", green)),
            Row::detailed("Time in Berlin", "04:18").with_icon(Icon::new("T", purple)),
            Row::detailed("Convert 10 kg to lb", "22.0462 lb").with_icon(Icon::new("C", blue)),
            Row::detailed("Money: 100 USD to EUR", "92.41 €").with_icon(Icon::new("€", green)),
            Row::detailed("Open the project folder", "Documents").with_icon(Icon::new("D", orange)),
            Row::detailed("Snippets", "14 saved").with_icon(Icon::new("S", purple)),
            Row::detailed("Clipboard history", "228 items").with_icon(Icon::new("H", red)),
        ],
        selected: 0,
        hovered: None,
        footer: "Enter to open  ·  Up and down to choose  ·  Esc to dismiss".to_string(),
    };

    let palette = match Palette::new(theme, model) {
        Ok(palette) => palette,
        Err(error) => {
            eprintln!("the palette could not be opened: {error}");
            std::process::exit(1);
        }
    };

    palette.show();
    let display = palette.display();
    println!(
        "palette open in {:?} (style pack: {})",
        started.elapsed(),
        name
    );
    println!(
        "display: system dpi {}, window dpi {}, surface {:.0}x{:.0} dip",
        display.system_dpi, display.window_dpi, display.target_width, display.target_height
    );
    println!("press escape to close");

    palette.run();
    println!("closed after {:?}", started.elapsed());
}

/// The style pack the arguments name, if they name one.
fn style_pack_path(arguments: &[String]) -> Option<PathBuf> {
    let position = arguments
        .iter()
        .position(|argument| argument == "--style-pack")?;
    arguments.get(position + 1).map(PathBuf::from)
}
