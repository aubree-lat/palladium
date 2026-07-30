
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::config::{ClientMod, Config, DiscordBranch, LinuxBackend, Theme};
use crate::import::{self, ImportSource};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    config: Config,
    app_version: String,
    arrpc_port: u16,
    import_sources: Vec<import::Available>,
}

#[tauri::command]
pub fn palladium_snapshot(state: State<'_, AppState>) -> Snapshot {
    let config = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    let import_sources = import::available();
    log::info!(
        "settings requested; import sources: {:?}",
        import_sources
            .iter()
            .map(|s| format!("{} ({} plugins, {} themes)", s.name, s.plugins, s.themes))
            .collect::<Vec<_>>()
    );
    Snapshot {
        arrpc_port: config.arrpc_bridge_port,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        import_sources,
        config,
    }
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    config: Config,
    imported_mod_settings: bool,
    imported_quick_css: bool,
}

#[tauri::command]
pub fn palladium_import_settings(
    state: State<'_, AppState>,
    source: ImportSource,
) -> Result<ImportResult, String> {
    let mut guard = state
        .config
        .lock()
        .map_err(|_| "config lock poisoned".to_string())?;

    let imported = import::import(source, &mut guard).map_err(|e| format!("{e:#}"))?;
    import::stash_pending(&imported).map_err(|e| format!("{e:#}"))?;

    log::info!(
        "imported settings from {} (mod settings: {}, quickcss: {})",
        source.display_name(),
        imported.mod_settings.is_some(),
        imported.quick_css.is_some()
    );

    Ok(ImportResult {
        config: guard.clone(),
        imported_mod_settings: imported.mod_settings.is_some(),
        imported_quick_css: imported.quick_css.is_some(),
    })
}

#[tauri::command]
pub fn palladium_open_settings(app: AppHandle) -> Result<(), String> {
    crate::open_settings_window(&app).map_err(|e| e.to_string())
}

#[derive(Debug, Default, Deserialize)]
pub struct ConfigPatch {
    pub client_mod: Option<ClientMod>,
    pub arrpc_enabled: Option<bool>,
    pub always_update_mod: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub discord_branch: Option<DiscordBranch>,
    pub linux_backend: Option<LinuxBackend>,
    pub theme: Option<Theme>,
    pub theme_discord: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ApplyResult {
    config: Config,
    reloading: bool,
    needs_restart: bool,
}

#[tauri::command]
pub fn palladium_update_config(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: ConfigPatch,
) -> Result<ApplyResult, String> {
    let (config, mod_changed, branch_changed, arrpc_changed, theme_changed) = {
        let mut guard = state
            .config
            .lock()
            .map_err(|_| "config lock poisoned".to_string())?;

        let mod_changed = patch.client_mod.is_some_and(|m| m != guard.client_mod);
        let branch_changed = patch
            .discord_branch
            .is_some_and(|b| b != guard.discord_branch);
        let arrpc_changed = patch.arrpc_enabled.is_some_and(|a| a != guard.arrpc_enabled);
        let backend_changed = patch
            .linux_backend
            .is_some_and(|b| b != guard.linux_backend);
        let theme_changed = patch.theme.is_some_and(|t| t != guard.theme)
            || patch.theme_discord.is_some_and(|d| d != guard.theme_discord);

        if let Some(v) = patch.client_mod {
            guard.client_mod = v;
        }
        if let Some(v) = patch.arrpc_enabled {
            guard.arrpc_enabled = v;
        }
        if let Some(v) = patch.always_update_mod {
            guard.always_update_mod = v;
        }
        if let Some(v) = patch.minimize_to_tray {
            guard.minimize_to_tray = v;
        }
        if let Some(v) = patch.discord_branch {
            guard.discord_branch = v;
        }
        if let Some(v) = patch.linux_backend {
            guard.linux_backend = v;
        }
        if let Some(v) = patch.theme {
            guard.theme = v;
        }
        if let Some(v) = patch.theme_discord {
            guard.theme_discord = v;
        }

        guard.save().map_err(|e| format!("{e:#}"))?;
        (
            guard.clone(),
            mod_changed,
            branch_changed,
            arrpc_changed || backend_changed,
            theme_changed,
        )
    };

    let reloading = mod_changed || branch_changed;
    if reloading {
        crate::start_client(app, config.clone());
    } else if theme_changed {
        crate::apply_discord_theme(&app, &config);
    }

    Ok(ApplyResult {
        config,
        reloading,
        needs_restart: arrpc_changed,
    })
}

#[tauri::command]
pub fn palladium_finish_setup(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: ConfigPatch,
) -> Result<(), String> {
    let config = {
        let mut guard = state
            .config
            .lock()
            .map_err(|_| "config lock poisoned".to_string())?;
        if let Some(v) = patch.client_mod {
            guard.client_mod = v;
        }
        if let Some(v) = patch.arrpc_enabled {
            guard.arrpc_enabled = v;
        }
        guard.save().map_err(|e| format!("{e:#}"))?;
        guard.clone()
    };

    log::info!(
        "first-run setup complete: {} (arRPC {})",
        config.client_mod.display_name(),
        if config.arrpc_enabled { "on" } else { "off" }
    );

    if let Some(setup) = app.get_webview_window("setup") {
        let _ = setup.close();
    }
    crate::start_client(app, config);
    Ok(())
}

#[tauri::command]
pub fn palladium_clear_mod_cache(state: State<'_, AppState>) -> Result<(), String> {
    let client_mod = state
        .config
        .lock()
        .map(|c| c.client_mod)
        .map_err(|_| "config lock poisoned".to_string())?;

    let dir = crate::mods::cache_dir(client_mod).map_err(|e| format!("{e:#}"))?;
    for asset in ["browser.js", "browser.css", "etags.json"] {
        let _ = std::fs::remove_file(dir.join(asset));
    }
    log::info!("cleared cached {} bundle", client_mod.display_name());
    Ok(())
}

#[tauri::command]
pub fn palladium_relaunch(app: AppHandle) {
    app.restart()
}
