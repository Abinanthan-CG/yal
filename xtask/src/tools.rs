//! Build orchestration helpers: release packaging and self-test.

use crate::{repo_root, run_tool};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Find NSIS's `makensis`.
///
/// It is on `PATH` after a normal install, but a per-machine install does
/// not always add it, so the standard locations are checked as well.
fn find_makensis() -> Option<String> {
    if let Ok(path) = std::env::var("PATH") {
        for directory in path.split(';') {
            if directory.is_empty() {
                continue;
            }
            let candidate = Path::new(directory).join("makensis.exe");
            if candidate.is_file() {
                return Some(candidate.display().to_string());
            }
        }
    }
    [
        r"C:\Program Files (x86)\NSIS\makensis.exe",
        r"C:\Program Files\NSIS\makensis.exe",
    ]
    .iter()
    .find(|candidate| Path::new(candidate).is_file())
    .map(|candidate| candidate.to_string())
}

/// Check that the Visual Studio ARM64 cross-compiler is installed.
///
/// The ARM64 build needs the `VC.Tools.ARM64` component of the Visual Studio
/// Build Tools, which is not installed by default. Without it the link fails
/// with "link.exe not found", which says nothing about what to do.
fn check_arm64_toolchain() -> Result<()> {
    let roots = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC",
    ];
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let arm64 = entry.path().join("bin").join("Hostx64").join("arm64");
            if arm64.join("link.exe").is_file() {
                return Ok(());
            }
        }
    }
    bail!(
        "the ARM64 cross-compiler is not installed.\n\
         \n\
         The ARM64 build needs the 'MSVC v143 ARM64 build tools' component of the\n\
         Visual Studio Build Tools. Install it with:\n\
         \n\
           winget install --id Microsoft.VisualStudio.2022.BuildTools -e --force \\\n\
             --override \"--wait --passive --norestart --add Microsoft.VisualStudio.Component.VC.Tools.ARM64\"\n\
         \n\
         Or build the other architecture with '--arch x64'. The release workflow on\n\
         windows-latest has the ARM64 tools, so it builds both."
    )
}

/// One release artifact to produce.
struct Artifact {
    /// File name inside `dist/`.
    file: &'static str,
    /// Rust target triple.
    target: &'static str,
    /// NSIS architecture token.
    nsis_arch: &'static str,
}

const ARTIFACTS: &[Artifact] = &[
    Artifact {
        file: "x64",
        target: "x86_64-pc-windows-msvc",
        nsis_arch: "x64",
    },
    Artifact {
        file: "arm64",
        target: "aarch64-pc-windows-msvc",
        nsis_arch: "arm64",
    },
];

/// Build release binaries, the installer, the portable zip and the checksums.
pub fn dist(args: &[String]) -> Result<()> {
    let channel = flag_value(args, "--channel").unwrap_or_else(|| "stable".to_string());
    let only = flag_value(args, "--arch");
    let version = crate_version()?;
    let root = repo_root();
    let dist_dir = root.join("dist");
    std::fs::create_dir_all(&dist_dir).context("creating dist/")?;

    let binaries = ["yal"];
    let mut produced: Vec<PathBuf> = Vec::new();

    for artifact in ARTIFACTS {
        let file = artifact.file;
        let target = artifact.target;
        let nsis_arch = artifact.nsis_arch;
        if let Some(only) = &only {
            if only != file {
                continue;
            }
        }
        println!("dist: building {} ({})", target, file);
        if file == "arm64" {
            check_arm64_toolchain()?;
        }
        run_tool(
            "cargo",
            &[
                "build",
                "--release",
                "--target",
                artifact.target,
                "--bin",
                "yal",
            ],
            "release build",
        )?;

        let stage = dist_dir.join(format!("stage-{file}"));
        if stage.exists() {
            std::fs::remove_dir_all(&stage).ok();
        }
        std::fs::create_dir_all(&stage).context("creating staging directory")?;

        let profile_dir = root.join("target").join(artifact.target).join("release");
        for bin in binaries {
            let from = profile_dir.join(format!("{bin}.exe"));
            if !from.exists() {
                bail!(
                    "expected release binary {} was not produced",
                    from.display()
                );
            }
            std::fs::copy(&from, stage.join(format!("{bin}.exe")))
                .with_context(|| format!("staging {}", from.display()))?;
        }
        copy_if_present(&root.join("assets"), &stage)?;
        copy_if_present(&root.join("installer").join("assets"), &stage)?;
        // The portable build keeps its data beside the executable.
        std::fs::write(stage.join("portable"), b"yal portable marker\n")
            .context("writing portable marker")?;

        // Installer
        let nsi = root.join("installer").join("yal.nsi");
        if !nsi.exists() {
            bail!("installer script {} is missing", nsi.display());
        }
        let setup = dist_dir.join(format!("yal-setup-{version}-{file}.exe"));
        println!("dist: building installer {}", setup.display());
        let installer = std::path::Path::new(&nsi);
        let installer_display = installer.display().to_string();
        let setup_display = setup.display().to_string();
        // The installer script lives in `installer/`, and NSIS resolves
        // relative paths against the script's own directory.
        let stage_display = format!("..\\dist\\stage-{file}");
        let makensis = find_makensis().ok_or_else(|| {
            anyhow::anyhow!(
                "NSIS was not found. Install it with 'winget install --id NSIS.NSIS -e'."
            )
        })?;
        run_tool(
            &makensis,
            &[
                "-V3",
                &format!("/DVERSION={version}"),
                &format!("/DARCH={file}"),
                &format!("/DNSIS_ARCH={nsis_arch}"),
                &format!("/DCHANNEL={channel}"),
                &format!("/DSTAGE={stage_display}"),
                &format!("/DOUTFILE={setup_display}"),
                &installer_display,
            ],
            "NSIS installer build",
        )?;
        produced.push(setup);

        // Portable zip
        let zip_path = dist_dir.join(format!("yal-{version}-{file}-portable.zip"));
        println!("dist: building portable zip {}", zip_path.display());
        make_zip(&stage, &zip_path)?;
        produced.push(zip_path);
    }

    // Checksums
    let mut sums = String::new();
    let mut digests: Vec<(String, String)> = Vec::new();
    for path in &produced {
        let digest = sha256_file(path)?;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        digests.push((name.clone(), digest.clone()));
        sums.push_str(&format!("{digest}  {name}\n"));
    }
    let sums_path = dist_dir.join("SHA256SUMS.txt");
    std::fs::write(&sums_path, sums).context("writing SHA256SUMS.txt")?;
    println!("dist: wrote {}", sums_path.display());

    // The packaging manifests, with the checksums of what was just built.
    write_packaging_manifests(&root, &dist_dir, &version, &digests)?;

    for path in &produced {
        println!(
            "  {} ({})",
            path.display(),
            crate::human_bytes(std::fs::metadata(path)?.len())
        );
    }
    Ok(())
}

/// Fill in the packaging templates and write them next to the artifacts.
///
/// The templates in `packaging/` hold the shape; the checksums and the
/// repository slug are only known here, once the artifacts exist. A published
/// release's artifacts are signed, so their checksums differ from a local
/// build's, and the release workflow uploads the filled-in copies.
fn write_packaging_manifests(
    root: &std::path::Path,
    dist_dir: &std::path::Path,
    version: &str,
    digests: &[(String, String)],
) -> Result<()> {
    let digest_of = |name: &str| -> Option<String> {
        digests
            .iter()
            .find(|(file, _)| file == name)
            .map(|(_, digest)| digest.clone())
    };
    let installer_x64 = digest_of(&format!("yal-setup-{version}-x64.exe"));
    let installer_arm64 = digest_of(&format!("yal-setup-{version}-arm64.exe"));
    let portable_x64 = digest_of(&format!("yal-{version}-x64-portable.zip"));
    let (Some(installer_x64), Some(installer_arm64), Some(portable_x64)) =
        (installer_x64, installer_arm64, portable_x64)
    else {
        // A single architecture was built, so there is nothing complete to
        // write. The release workflow builds both.
        return Ok(());
    };

    let replacements: Vec<(&str, String)> = vec![
        ("{{VERSION}}", version.to_string()),
        ("{{REPOSITORY}}", tc_core::REPOSITORY.to_string()),
        ("{{RELEASE_DATE}}", release_date()),
        ("{{INSTALLER_SHA256_X64}}", installer_x64),
        ("{{INSTALLER_SHA256_ARM64}}", installer_arm64),
        ("{{PORTABLE_SHA256_X64}}", portable_x64),
    ];

    let out_dir = dist_dir.join("packaging");
    for (template, relative) in [
        ("packaging/winget/YAL.YAML", "winget/YAL.YAML"),
        ("packaging/scoop/yal.json", "scoop/yal.json"),
    ] {
        let source = root.join(template);
        let text = std::fs::read_to_string(&source)
            .with_context(|| format!("reading {}", source.display()))?;
        let mut filled = text.clone();
        for (placeholder, value) in &replacements {
            filled = filled.replace(placeholder, value);
        }
        let destination = out_dir.join(relative);
        std::fs::create_dir_all(destination.parent().expect("a parent directory"))
            .with_context(|| format!("creating {}", destination.parent().unwrap().display()))?;
        std::fs::write(&destination, filled)
            .with_context(|| format!("writing {}", destination.display()))?;
        println!("dist: wrote {}", destination.display());
    }
    Ok(())
}

/// Today's date, for the manifests' release date.
fn release_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn copy_if_present(dir: &Path, stage: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            std::fs::copy(&path, stage.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn make_zip(stage: &Path, zip_path: &Path) -> Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(zip_path).context("creating portable zip")?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    let mut stack = vec![stage.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(stage)
                .expect("staged path is under the staging directory")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                stack.push(path);
            } else {
                zip.start_file(name, options)?;
                let bytes = std::fs::read(&path)?;
                zip.write_all(&bytes)?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

/// Compute the SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Read `--flag value` from an argument list.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The workspace version, taken from the root manifest.
pub fn crate_version() -> Result<String> {
    let manifest = repo_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).context("reading Cargo.toml")?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version = ") {
            if !line.starts_with(' ') {
                return Ok(rest.trim_matches('"').to_string());
            }
        }
    }
    bail!("could not find the workspace version in Cargo.toml")
}

/// Run the built launcher's self-test and print its JSON report.
pub fn selftest() -> Result<()> {
    let exe = repo_root().join("target").join("release").join("yal.exe");
    if !exe.exists() {
        bail!(
            "{} does not exist; run `cargo build --release --bin yal` first",
            exe.display()
        );
    }
    let output = std::process::Command::new(&exe)
        .arg("--self-test")
        .output()
        .with_context(|| format!("running {}", exe.display()))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        bail!("self-test failed with status {}", output.status);
    }
    Ok(())
}
