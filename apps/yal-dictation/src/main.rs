//! The dictation helper.
//!
//! Dictation turns speech into text. The model runs in its own helper process
//! so a large model cannot bloat the launcher and a crash in the recogniser
//! cannot take the app down.
//!
//! The helper reports the models it can see and whether the inference runtime
//! is present. The audio never touches the disk and never leaves the machine.

use std::path::{Path, PathBuf};

/// The models upstream ships, and the families they belong to.
pub const MODELS: [(&str, &str); 4] = [
    ("Parakeet Redux", "parakeet"),
    ("Parakeet Ultra", "parakeet"),
    ("Qwen 0.6B", "qwen"),
    ("Qwen 1.7B", "qwen"),
];

/// How long one dictation may run, in minutes.
pub const MAXIMUM_SESSION_MINUTES: u32 = 5;

/// How far the output volume is ducked while recording, as a fraction.
pub const DUCKED_VOLUME: f32 = 0.1;

/// What the helper knows about the installed models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    pub folder: PathBuf,
    pub present: Vec<String>,
    pub runtime_available: bool,
}

/// Look at the model folder and report what is installed.
pub fn catalog(folder: &Path) -> ModelCatalog {
    let present = MODELS
        .iter()
        .filter(|(name, _)| folder.join(model_file_name(name)).exists())
        .map(|(name, _)| (*name).to_string())
        .collect();
    ModelCatalog {
        folder: folder.to_path_buf(),
        present,
        runtime_available: runtime_available(folder),
    }
}

/// The file a model's weights live in.
pub fn model_file_name(name: &str) -> String {
    let stem: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    format!("{stem}.onnx")
}

/// Whether the inference runtime is present.
///
/// The runtime is a native library the helper loads; without it no model can
/// run, so the helper says so rather than failing at the first recording.
pub fn runtime_available(folder: &Path) -> bool {
    folder.join("onnxruntime.dll").exists() || folder.join("DirectML.dll").exists()
}

/// The catalog as JSON.
pub fn catalog_json(catalog: &ModelCatalog) -> String {
    let present: Vec<String> = catalog
        .present
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    format!(
        "{{\"folder\":\"{}\",\"installed\":[{}],\"runtime_available\":{}}}",
        catalog.folder.display().to_string().replace('\\', "\\\\"),
        present.join(","),
        catalog.runtime_available
    )
}

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let folder = arguments
        .iter()
        .position(|a| a == "--catalog")
        .and_then(|index| arguments.get(index + 1))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::args_os()
                .nth(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
        });

    if folder.as_os_str().is_empty() {
        eprintln!("yal-dictation: usage: yal-dictation --catalog <folder>");
        return std::process::ExitCode::FAILURE;
    }

    println!("{}", catalog_json(&catalog(&folder)));
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_four_models() {
        assert_eq!(MODELS.len(), 4);
    }

    #[test]
    fn the_model_file_names_are_filesystem_safe() {
        assert_eq!(model_file_name("Parakeet Redux"), "parakeetredux.onnx");
        assert_eq!(model_file_name("Qwen 0.6B"), "qwen06b.onnx");
    }

    #[test]
    fn an_empty_folder_holds_no_models() {
        let folder = std::env::temp_dir().join("yal-dictation-test-empty");
        std::fs::create_dir_all(&folder).unwrap();
        let catalog = catalog(&folder);
        assert!(catalog.present.is_empty());
        assert!(!catalog.runtime_available);
    }

    #[test]
    fn the_session_limit_is_upstreams_limit() {
        assert_eq!(MAXIMUM_SESSION_MINUTES, 5);
        assert!((DUCKED_VOLUME - 0.1).abs() < f32::EPSILON);
    }
}
