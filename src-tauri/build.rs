fn main() {
    // Declaring the app's own commands generates `allow-*` / `deny-*`
    // permissions for them. This is what lets the injected settings panel —
    // which runs on a *remote* origin (discord.com) — reach Palladium's
    // commands, while keeping every command an explicit, auditable grant.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "palladium_snapshot",
            "palladium_update_config",
            "palladium_finish_setup",
            "palladium_clear_mod_cache",
            "palladium_relaunch",
            "palladium_import_settings",
            "palladium_open_settings",
        ])),
    )
    .expect("failed to run tauri-build");
}
