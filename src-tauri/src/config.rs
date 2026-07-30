
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClientMod {
    None,
    #[default]
    Vencord,
    Equicord,
}

impl ClientMod {
    pub fn display_name(self) -> &'static str {
        match self {
            ClientMod::None => "Vanilla",
            ClientMod::Vencord => "Vencord",
            ClientMod::Equicord => "Equicord",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ClientMod::None => "none",
            ClientMod::Vencord => "vencord",
            ClientMod::Equicord => "equicord",
        }
    }

    pub fn release_base(self) -> Option<&'static str> {
        match self {
            ClientMod::None => None,
            ClientMod::Vencord => {
                Some("https://github.com/Vendicated/Vencord/releases/download/devbuild")
            }
            ClientMod::Equicord => {
                Some("https://github.com/Equicord/Equicord/releases/download/latest")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub client_mod: ClientMod,
    pub arrpc_enabled: bool,
    pub arrpc_bridge_port: u16,
    pub always_update_mod: bool,
    pub minimize_to_tray: bool,
    pub discord_branch: DiscordBranch,
    pub linux_backend: LinuxBackend,
    pub theme: Theme,
    pub theme_discord: bool,
    pub zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Palladium,
    Oxygen,
    Hydrogen,
    Plutonium,
    Uranium,
    Iron,
    Gallium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LinuxBackend {
    #[default]
    Auto,
    Wayland,
    X11,
}

impl LinuxBackend {
    pub fn gdk_value(self) -> Option<&'static str> {
        match self {
            LinuxBackend::Auto => None,
            LinuxBackend::Wayland => Some("wayland"),
            LinuxBackend::X11 => Some("x11"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiscordBranch {
    #[default]
    Stable,
    Ptb,
    Canary,
}

impl DiscordBranch {
    pub fn url(self) -> &'static str {
        match self {
            DiscordBranch::Stable => "https://discord.com/channels/@me",
            DiscordBranch::Ptb => "https://ptb.discord.com/channels/@me",
            DiscordBranch::Canary => "https://canary.discord.com/channels/@me",
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_mod: ClientMod::Vencord,
            arrpc_enabled: true,
            arrpc_bridge_port: 1337,
            always_update_mod: false,
            minimize_to_tray: true,
            discord_branch: DiscordBranch::Stable,
            linux_backend: LinuxBackend::Auto,
            theme: Theme::Palladium,
            theme_discord: false,
            zoom: 1.0,
        }
    }
}

impl Config {
    pub fn start_url(&self) -> String {
        std::env::var("PALLADIUM_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| self.discord_branch.url().to_string())
    }

    pub fn migrate_legacy_data() {
        let Some(share) = dirs::data_dir() else {
            return;
        };

        for (old, new) in [
            ("dev.tauricord.app", "dev.palladium.app"),
            ("tauricord", "palladium"),
        ] {
            let legacy = share.join(old);
            let target = share.join(new);
            if !legacy.is_dir() {
                continue;
            }
            let target_empty = std::fs::read_dir(&target)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true);
            if !target_empty {
                continue;
            }
            let _ = std::fs::remove_dir_all(&target);
            match std::fs::rename(&legacy, &target) {
                Ok(()) => log::info!("migrated {} to {}", legacy.display(), target.display()),
                Err(e) => log::warn!("could not migrate {}: {e}", legacy.display()),
            }
        }
    }

    pub fn dir() -> Result<PathBuf> {
        let base = dirs::config_dir()
            .context("could not determine the platform config directory")?;
        let dir = base.join("palladium");

        let legacy = base.join("tauricord");
        if !dir.exists() && legacy.is_dir() {
            match fs::rename(&legacy, &dir) {
                Ok(()) => log::info!("migrated settings from {}", legacy.display()),
                Err(e) => log::warn!("could not migrate {}: {e}", legacy.display()),
            }
        }

        fs::create_dir_all(&dir)
            .with_context(|| format!("creating config dir {}", dir.display()))?;
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.json"))
    }

    pub fn load() -> (Self, bool) {
        let first_run = Self::path().map(|p| !p.exists()).unwrap_or(false);
        match Self::try_load() {
            Ok(cfg) => (cfg, first_run),
            Err(e) => {
                log::warn!("falling back to default config: {e:#}");
                (Self::default(), first_run)
            }
        }
    }

    fn try_load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}
