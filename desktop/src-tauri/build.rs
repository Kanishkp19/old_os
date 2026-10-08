fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "hub_request", "local_list", "local_save", "local_trash", "local_restore",
            "local_export", "local_import", "browser_open", "browser_action", "browser_list",
            "browser_permissions", "browser_clear", "open_external", "save_download", "save_hub_file"
        ])
    )).expect("desktop build configuration is invalid");
}
