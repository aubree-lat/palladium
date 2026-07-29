
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::ClientMod;

const ASSETS: [&str; 2] = ["browser.js", "browser.css"];

#[derive(Debug, Default, Serialize, Deserialize)]
struct EtagStore(HashMap<String, String>);

#[derive(Debug, Clone)]
pub struct ModBundle {
    pub client_mod: ClientMod,
    pub js: String,
    pub css: String,
}

pub fn cache_dir(client_mod: ClientMod) -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .context("could not determine the platform data directory")?
        .join("tauricord")
        .join("mods")
        .join(client_mod.slug());
    fs::create_dir_all(&dir).with_context(|| format!("creating cache dir {}", dir.display()))?;
    Ok(dir)
}

fn etag_path(client_mod: ClientMod) -> Result<PathBuf> {
    Ok(cache_dir(client_mod)?.join("etags.json"))
}

fn load_etags(client_mod: ClientMod) -> EtagStore {
    etag_path(client_mod)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_etags(client_mod: ClientMod, store: &EtagStore) {
    let Ok(path) = etag_path(client_mod) else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(store) {
        let _ = fs::write(path, json);
    }
}

pub async fn fetch(client_mod: ClientMod, force: bool) -> Result<Option<ModBundle>> {
    let Some(base) = client_mod.release_base() else {
        return Ok(None);
    };

    let dir = cache_dir(client_mod)?;
    let mut etags = load_etags(client_mod);

    let client = reqwest::Client::builder()
        .user_agent(concat!("Tauricord/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()
        .context("building HTTP client")?;

    let mut changed = false;
    for asset in ASSETS {
        let dest = dir.join(asset);
        let url = format!("{base}/{asset}");

        let mut req = client.get(&url);
        if !force {
            if let (Some(tag), true) = (etags.0.get(asset), dest.exists()) {
                req = req.header(reqwest::header::IF_NONE_MATCH, tag);
            }
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) if dest.exists() => {
                log::warn!("{} unreachable ({e}), using cached {asset}", client_mod.display_name());
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("downloading {url}")),
        };

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            log::debug!("{asset} is up to date");
            continue;
        }

        if !resp.status().is_success() {
            if dest.exists() {
                log::warn!("{url} returned {}, using cached copy", resp.status());
                continue;
            }
            bail!("{url} returned {}", resp.status());
        }

        let new_tag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let body = resp
            .text()
            .await
            .with_context(|| format!("reading body of {url}"))?;

        if body.trim().is_empty() {
            bail!("{url} returned an empty body");
        }

        fs::write(&dest, &body).with_context(|| format!("writing {}", dest.display()))?;
        if let Some(tag) = new_tag {
            etags.0.insert(asset.to_string(), tag);
        }
        changed = true;
        log::info!("updated {} {asset} ({} bytes)", client_mod.display_name(), body.len());
    }

    if changed {
        save_etags(client_mod, &etags);
    }

    let js = fs::read_to_string(dir.join("browser.js"))
        .with_context(|| format!("reading cached browser.js for {}", client_mod.display_name()))?;
    let css = fs::read_to_string(dir.join("browser.css")).unwrap_or_default();

    Ok(Some(ModBundle {
        client_mod,
        js,
        css,
    }))
}
