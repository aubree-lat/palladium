
mod arrpc;
mod commands;
mod config;
mod inject;
mod mods;
mod webkit;

use std::sync::Mutex;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use arrpc::RpcEvent;
use config::Config;

pub(crate) struct AppState {
    pub(crate) config: Mutex<Config>,
}

pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("tauricord=info,warn"),
    )
    .init();

    let (config, first_run) = Config::load();
    log::info!(
        "starting Tauricord with {} (arRPC {}{})",
        config.client_mod.display_name(),
        if config.arrpc_enabled { "on" } else { "off" },
        if first_run { ", first run" } else { "" }
    );

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
            commands::tauricord_snapshot,
            commands::tauricord_update_config,
            commands::tauricord_finish_setup,
            commands::tauricord_clear_mod_cache,
            commands::tauricord_relaunch,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;

            if first_run {
                WebviewWindowBuilder::new(&handle, "setup", WebviewUrl::App("setup.html".into()))
                    .title("Welcome to Tauricord")
                    .inner_size(680.0, 620.0)
                    .min_inner_size(560.0, 520.0)
                    .center()
                    .build()?;
            } else {
                WebviewWindowBuilder::new(&handle, "splash", WebviewUrl::App("index.html".into()))
                    .title("Tauricord")
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
        .expect("error while running Tauricord");
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

        let script = inject::build_script(bundle.as_ref());

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
        .title("Tauricord")
        .inner_size(1280.0, 800.0)
        .min_inner_size(660.0, 420.0)
        .user_agent(inject::user_agent())
        .devtools(true)
        .center()
        .background_color(tauri::window::Color(13, 0, 26, 255))
        .disable_drag_drop_handler()
        .initialization_script(&script)
        .on_document_title_changed(|window, title| {
            log::debug!("document title: {title}");
            let title = if title.trim().is_empty() {
                "Tauricord".to_string()
            } else {
                format!("{title} — Tauricord")
            };
            let _ = window.set_title(&title);
        })
        .build()?;

    webkit::tune(&window);

    log::info!("opening {start_url}");
    window.navigate(target)?;

    Ok(())
}

fn report_splash_error(app: &AppHandle, message: &str) {
    let Some(splash) = app.get_webview_window("splash") else {
        return;
    };
    let payload = serde_json::Value::String(message.to_string()).to_string();
    let _ = splash.eval(&format!(
        "window.tauricordError && window.tauricordError({payload})"
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
    let open = MenuItemBuilder::with_id("open", "Open Discord").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Tauricord Settings").build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "Reload").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Tauricord").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&open, &settings, &reload])
        .separator()
        .item(&quit)
        .build()?;

    let mut tray = TrayIconBuilder::with_id("tauricord")
        .menu(&menu)
        .tooltip("Tauricord")
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
            if let Some(window) = window {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.eval(
                    "window.__TAURICORD__ && window.__TAURICORD__.openSettings \
                     && window.__TAURICORD__.openSettings()",
                );
            }
        }
        _ => {}
    }
}
