use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, DiscordBranch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    Vesktop,
    Equibop,
}

impl ImportSource {
    pub fn all() -> [ImportSource; 2] {
        [ImportSource::Equibop, ImportSource::Vesktop]
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ImportSource::Vesktop => "vesktop",
            ImportSource::Equibop => "equibop",
        }
    }

    fn dir_names(self) -> &'static [&'static str] {
        match self {
            ImportSource::Vesktop => &["vesktop", "Vesktop"],
            ImportSource::Equibop => &["equibop", "Equibop"],
        }
    }

    fn root(self) -> Option<PathBuf> {
        let base = dirs::config_dir()?;
        self.dir_names()
            .iter()
            .map(|n| base.join(n))
            .find(|p| p.is_dir())
    }

    fn client_settings_path(self) -> Option<PathBuf> {
        let p = self.root()?.join("settings.json");
        p.is_file().then_some(p)
    }

    fn mod_settings_path(self) -> Option<PathBuf> {
        let p = self.root()?.join("settings").join("settings.json");
        p.is_file().then_some(p)
    }

    fn quick_css_path(self) -> Option<PathBuf> {
        let p = self.root()?.join("settings").join("quickCss.css");
        p.is_file().then_some(p)
    }
}

#[derive(Debug, Serialize)]
pub struct Available {
    pub id: ImportSource,
    pub name: &'static str,
    pub plugins: usize,
    pub themes: usize,
    pub quick_css: bool,
}

pub fn available() -> Vec<Available> {
    ImportSource::all()
        .into_iter()
        .filter(|s| s.root().is_some())
        .map(|s| {
            let mod_settings = s
                .mod_settings_path()
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());

            let plugins = mod_settings
                .as_ref()
                .and_then(|v| v.get("plugins"))
                .and_then(Value::as_object)
                .map(|o| {
                    o.values()
                        .filter(|p| p.get("enabled").and_then(Value::as_bool).unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);

            let themes = mod_settings
                .as_ref()
                .and_then(|v| v.get("enabledThemes"))
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);

            Available {
                id: s,
                name: s.display_name(),
                plugins,
                themes,
                quick_css: s.quick_css_path().is_some(),
            }
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct Imported {
    pub mod_settings: Option<String>,
    pub quick_css: Option<String>,
}

/// Copy what maps cleanly onto Palladium's own config, and hand back the client
/// mod's settings for seeding into the webview.
pub fn import(source: ImportSource, config: &mut Config) -> Result<Imported> {
    if source.root().is_none() {
        bail!("no {} config found", source.display_name());
    }

    if let Some(path) = source.client_settings_path() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let v: Value = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;

        if let Some(branch) = v.get("discordBranch").and_then(Value::as_str) {
            config.discord_branch = match branch {
                "ptb" => DiscordBranch::Ptb,
                "canary" => DiscordBranch::Canary,
                _ => DiscordBranch::Stable,
            };
        }
        if let Some(tray) = v.get("minimizeToTray").and_then(Value::as_bool) {
            config.minimize_to_tray = tray;
        }
        // Equibop tracks the inverse under a separate key, so honour whichever
        // one is present rather than assuming a single spelling.
        if let Some(disabled) = v.get("arRPCDisabled").and_then(Value::as_bool) {
            config.arrpc_enabled = !disabled;
        } else if let Some(on) = v.get("arRPC").and_then(Value::as_bool) {
            config.arrpc_enabled = on;
        }
    }

    let mod_settings = source
        .mod_settings_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .filter(|raw| serde_json::from_str::<Value>(raw).is_ok());

    let quick_css = source.quick_css_path().and_then(|p| fs::read_to_string(p).ok());

    config.save().context("saving imported config")?;

    Ok(Imported {
        mod_settings,
        quick_css,
    })
}

fn pending_dir() -> Result<PathBuf> {
    Config::dir()
}

/// Stashed alongside the config so the next launch can seed it into the webview,
/// where the browser build reads its settings out of localStorage.
pub fn stash_pending(imported: &Imported) -> Result<()> {
    let dir = pending_dir()?;
    if let Some(settings) = &imported.mod_settings {
        fs::write(dir.join("pending-mod-settings.json"), settings)?;
    }
    if let Some(css) = &imported.quick_css {
        fs::write(dir.join("pending-quick-css.css"), css)?;
    }
    Ok(())
}

pub struct Pending {
    pub mod_settings: Option<String>,
    pub quick_css: Option<String>,
}

pub fn take_pending() -> Pending {
    let Ok(dir) = pending_dir() else {
        return Pending {
            mod_settings: None,
            quick_css: None,
        };
    };

    let settings_path = dir.join("pending-mod-settings.json");
    let css_path = dir.join("pending-quick-css.css");

    let mod_settings = fs::read_to_string(&settings_path).ok();
    let quick_css = fs::read_to_string(&css_path).ok();

    if mod_settings.is_some() {
        let _ = fs::remove_file(&settings_path);
    }
    if quick_css.is_some() {
        let _ = fs::remove_file(&css_path);
    }

    Pending {
        mod_settings,
        quick_css,
    }
}
