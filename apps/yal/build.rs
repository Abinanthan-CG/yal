fn main() {
    // The launcher carries its own manifest: it runs as the user who started
    // it, it is drawn per monitor, it takes long paths, and it reads its own
    // files as UTF-8. The claims in `docs/WINDOWS-MAPPING.md` are only true
    // while this manifest is in the binary, and TauriBuild's own manifest
    // carries none of them.
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows/yal.manifest"));
    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attributes).expect("failed to run the build script")
}
