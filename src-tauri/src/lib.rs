
mod arrpc;
mod clipboard;
mod commands;
mod config;
mod import;
mod inject;
mod mods;
mod proxy;
mod theme;
mod webkit;

use std::sync::{Mutex, OnceLock};

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use arrpc::RpcEvent;
use config::Config;

pub(crate) struct AppState {
    pub(crate) config: Mutex<Config>,
}

static PROXY_BASE: OnceLock<String> = OnceLock::new();

pub(crate) fn proxy_base() -> Option<String> {
    PROXY_BASE.get().cloned()
}

pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("palladium=info,warn"),
    )
    .init();

    #[cfg(target_os = "linux")]
    if std::env::var_os("GTK_CSD").is_none() {
        std::env::set_var("GTK_CSD", "0");
    }

    Config::migrate_legacy_data();

    let (config, first_run) = Config::load();

    #[cfg(target_os = "linux")]
    if let Some(backend) = config.linux_backend.gdk_value() {
        if std::env::var_os("GDK_BACKEND").is_none() {
            log::info!("forcing GDK backend: {backend}");
            std::env::set_var("GDK_BACKEND", backend);
        }
    }
    log::info!(
        "starting Palladium with {} (arRPC {}{})",
        config.client_mod.display_name(),
        if config.arrpc_enabled { "on" } else { "off" },
        if first_run { ", first run" } else { "" }
    );

    match proxy::spawn() {
        Ok(p) => {
            let _ = PROXY_BASE.set(format!("http://127.0.0.1:{}/{}", p.port, p.token));
        }
        Err(e) => log::error!("could not start the csp bypass proxy: {e:#}"),
    }

    let rpc_events = if config.arrpc_enabled {
        match arrpc::spawn(config.arrpc_bridge_port) {
            Ok(rx) => Some(rx),
            Err(e) => {
                log::error!("could not start arRPC: {e:#}");
                None
            }
        }
    } else {
        None
    };

    let initial_config = config.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(config),
        })
        .invoke_handler(tauri::generate_handler![
            commands::palladium_snapshot,
            commands::palladium_update_config,
            commands::palladium_finish_setup,
            commands::palladium_clear_mod_cache,
            commands::palladium_relaunch,
            commands::palladium_import_settings,
            commands::palladium_open_settings,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            clipboard::register(&handle);
            build_tray(&handle)?;

            if first_run {
                WebviewWindowBuilder::new(&handle, "setup", WebviewUrl::App("setup.html".into()))
                    .title("welcome to palladium")
                    .inner_size(680.0, 620.0)
                    .min_inner_size(560.0, 520.0)
                    .center()
                    .build()?;
            } else {
                WebviewWindowBuilder::new(&handle, "splash", WebviewUrl::App("index.html".into()))
                    .title("palladium")
                    .inner_size(440.0, 280.0)
                    .resizable(false)
                    .decorations(false)
                    .center()
                    .build()?;
                start_client(handle.clone(), initial_config);
            }

            if let Some(rx) = rpc_events {
                watch_rpc_events(handle, rx);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let minimize = window
                    .state::<AppState>()
                    .config
                    .lock()
                    .map(|c| c.minimize_to_tray)
                    .unwrap_or(false);
                if minimize {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Palladium");
}

pub(crate) fn start_client(handle: AppHandle, cfg: Config) {
    tauri::async_runtime::spawn(async move {
        let bundle = match mods::fetch(cfg.client_mod, cfg.always_update_mod).await {
            Ok(bundle) => {
                match &bundle {
                    Some(b) => log::info!(
                        "injecting {} ({} KiB js, {} KiB css)",
                        b.client_mod.display_name(),
                        b.js.len() / 1024,
                        b.css.len() / 1024
                    ),
                    None => log::info!("no client mod selected, loading vanilla Discord"),
                }
                bundle
            }
            Err(e) => {
                log::error!("could not load {}: {e:#}", cfg.client_mod.display_name());
                report_splash_error(&handle, &format!("{e:#}"));
                return;
            }
        };

        let script = inject::build_script(bundle.as_ref(), &cfg);

        let open_handle = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if let Err(e) = open_main_window(&open_handle, &cfg, script) {
                log::error!("could not open the Discord window: {e}");
                report_splash_error(&open_handle, &e.to_string());
                return;
            }
            if let Some(splash) = open_handle.get_webview_window("splash") {
                let _ = splash.close();
            }
        });
    });
}

fn open_main_window(app: &AppHandle, cfg: &Config, script: String) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window("main") {
        let _ = existing.close();
    }

    let start_url = cfg.start_url();
    let target = tauri::Url::parse(&start_url).map_err(tauri::Error::InvalidUrl)?;

    let blank = tauri::Url::parse("about:blank").expect("about:blank is valid");

    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(blank))
        .title("palladium")
        .inner_size(1280.0, 800.0)
        .min_inner_size(660.0, 420.0)
        .user_agent(inject::user_agent())
        .devtools(true)
        .center()
        .background_color(tauri::window::Color(13, 0, 26, 255))
        .disable_drag_drop_handler()
        .on_new_window(|url, _features| {
            let host = url.host_str().unwrap_or_default().to_string();
            let internal = host == "discord.com"
                || host.ends_with(".discord.com")
                || host.ends_with(".discordapp.com")
                || host.ends_with(".discordapp.net");

            if internal {
                log::debug!("opening {url} in a child window");
                return tauri::webview::NewWindowResponse::Allow;
            }

            log::info!("opening {url} externally");
            if let Err(e) = tauri_plugin_opener::open_url(url.as_str(), None::<&str>) {
                log::warn!("could not open {url}: {e}");
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .initialization_script(&script)
        .on_document_title_changed(|window, title| {
            log::debug!("document title: {title}");
            let title = if title.trim().is_empty() {
                "palladium".to_string()
            } else {
                format!("{title} — palladium")
            };
            let _ = window.set_title(&title);
        })
        .build()?;

    webkit::tune(&window);

    log::info!("opening {start_url}");
    window.navigate(target)?;

    Ok(())
}

pub(crate) fn apply_zoom_step(step: f64) {
    let Some(app) = clipboard::app_handle() else {
        return;
    };

    let next = {
        let state = app.state::<AppState>();
        let Ok(mut cfg) = state.config.lock() else {
            return;
        };
        cfg.zoom = if step == 0.0 {
            1.0
        } else {
            (cfg.zoom + step).clamp(0.5, 3.0)
        };
        let _ = cfg.save();
        cfg.zoom
    };

    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window("main") {
            if let Err(e) = window.set_zoom(next) {
                log::warn!("could not set zoom: {e}");
            } else {
                log::debug!("zoom now {next:.2}");
            }
        }
    });
}

pub(crate) fn apply_discord_theme(app: &AppHandle, cfg: &Config) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let css = serde_json::Value::String(inject::theme_css_for(cfg)).to_string();
    let _ = window.eval(&format!(
        "window.__PALLADIUM_SET_THEME__ && window.__PALLADIUM_SET_THEME__({css})"
    ));
    log::info!(
        "discord theme now {}",
        if cfg.theme_discord {
            cfg.theme.slug()
        } else {
            "off"
        }
    );
}

pub(crate) fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(existing) = app.get_webview_window("settings") {
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("palladium settings")
        .inner_size(760.0, 780.0)
        .min_inner_size(600.0, 520.0)
        .center()
        .build()?;
    Ok(())
}

fn report_splash_error(app: &AppHandle, message: &str) {
    let Some(splash) = app.get_webview_window("splash") else {
        return;
    };
    let payload = serde_json::Value::String(message.to_string()).to_string();
    let _ = splash.eval(&format!(
        "window.palladiumError && window.palladiumError({payload})"
    ));
}

fn watch_rpc_events(app: AppHandle, mut rx: tokio::sync::mpsc::UnboundedReceiver<RpcEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                RpcEvent::Invite { code, is_invite } => {
                    if !code
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        log::warn!("ignoring malformed invite code from RPC client");
                        continue;
                    }
                    let Some(window) = app.get_webview_window("main") else {
                        continue;
                    };
                    let path = if is_invite { "invite" } else { "template" };
                    let _ = window.eval(&format!(
                        "location.assign('https://discord.com/{path}/{code}')"
                    ));
                    let _ = window.set_focus();
                }
                RpcEvent::DeepLink { args } => {
                    log::debug!("ignoring deep link request: {args}");
                }
            }
        }
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open", "open discord").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "palladium settings").build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "reload").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "quit palladium").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&open, &settings, &reload])
        .separator()
        .item(&quit)
        .build()?;

    let mut tray = TrayIconBuilder::with_id("palladium")
        .menu(&menu)
        .tooltip("palladium")
        .on_menu_event(on_tray_menu);

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

fn on_tray_menu(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let window = app.get_webview_window("main");
    match event.id().as_ref() {
        "quit" => app.exit(0),
        "open" => {
            if let Some(window) = window {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "reload" => {
            if let Some(window) = window {
                let _ = window.eval("location.reload()");
            }
        }
        "settings" => {
            if let Err(e) = open_settings_window(app) {
                log::error!("could not open the settings window: {e}");
            }
        }
        _ => {}
    }
}
