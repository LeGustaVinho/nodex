fn main() {
    // App commands must be declared to be granted to the UI, which is a remote (localhost) origin.
    let manifest = tauri_build::AppManifest::new().commands(&["save_export", "check_for_updates"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build failed");
}
