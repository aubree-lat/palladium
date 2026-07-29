fn main() {
    // Declaring the app's own commands generates `allow-*` / `deny-*`
    // permissions for them. This is what lets the injected settings panel —
    // which runs on a *remote* origin (discord.com) — reach Tauricord's
    // commands, while keeping every command an explicit, auditable grant.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "tauricord_snapshot",
            "tauricord_update_config",
            "tauricord_finish_setup",
            "tauricord_clear_mod_cache",
            "tauricord_relaunch",
        ])),
    )
    .expect("failed to run tauri-build");
}
