
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::config::{ClientMod, Config, DiscordBranch};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    config: Config,
    app_version: String,
    arrpc_port: u16,
}

#[tauri::command]
pub fn tauricord_snapshot(state: State<'_, AppState>) -> Snapshot {
    let config = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    log::info!("settings panel connected");
    Snapshot {
        arrpc_port: config.arrpc_bridge_port,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        config,
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ConfigPatch {
    pub client_mod: Option<ClientMod>,
    pub arrpc_enabled: Option<bool>,
    pub always_update_mod: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub discord_branch: Option<DiscordBranch>,
}

#[derive(Debug, Serialize)]
pub struct ApplyResult {
    config: Config,
    reloading: bool,
    needs_restart: bool,
}

#[tauri::command]
pub fn tauricord_update_config(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: ConfigPatch,
) -> Result<ApplyResult, String> {
    let (config, mod_changed, branch_changed, arrpc_changed) = {
        let mut guard = state
            .config
            .lock()
            .map_err(|_| "config lock poisoned".to_string())?;

        let mod_changed = patch.client_mod.is_some_and(|m| m != guard.client_mod);
        let branch_changed = patch
            .discord_branch
            .is_some_and(|b| b != guard.discord_branch);
        let arrpc_changed = patch.arrpc_enabled.is_some_and(|a| a != guard.arrpc_enabled);

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

        guard.save().map_err(|e| format!("{e:#}"))?;
        (guard.clone(), mod_changed, branch_changed, arrpc_changed)
    };

    let reloading = mod_changed || branch_changed;
    if reloading {
        crate::start_client(app, config.clone());
    }

    Ok(ApplyResult {
        config,
        reloading,
        needs_restart: arrpc_changed,
    })
}

#[tauri::command]
pub fn tauricord_finish_setup(
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
pub fn tauricord_clear_mod_cache(state: State<'_, AppState>) -> Result<(), String> {
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
pub fn tauricord_relaunch(app: AppHandle) {
    app.restart()
}
