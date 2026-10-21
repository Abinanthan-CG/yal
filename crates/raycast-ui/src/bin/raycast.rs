//! The launcher window.
//!
//! Opens the palette over the local store. The palette is a single
//! always-on-top window; it takes over the screen when focused and closes
//! when a result is chosen.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = match yal_store::default_path() {
        Some(path) => yal_store::Store::open(&path),
        None => yal_store::Store::open_in_memory(),
    }?;

    yal_ui::run(store)?;
    Ok(())
}
