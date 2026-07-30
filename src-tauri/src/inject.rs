use serde_json::Value;

use crate::config::Config;
use crate::mods::ModBundle;

const THEME_RUNTIME: &str = r#"(function(){
  if (window.__PALLADIUM_SET_THEME__) return;
  var sheet = null;
  var tag = null;
  window.__PALLADIUM_SET_THEME__ = function (css) {
    try {
      if (!sheet) {
        sheet = new CSSStyleSheet();
        document.adoptedStyleSheets = document.adoptedStyleSheets.concat(sheet);
      }
      sheet.replaceSync(css || "");
      return;
    } catch (e) {}
    var target = document.head || document.documentElement;
    if (!target) return;
    if (!tag) {
      tag = document.createElement("style");
      tag.id = "palladium-theme";
      target.appendChild(tag);
    }
    tag.textContent = css || "";
  };
})();
"#;

pub fn theme_css_for(cfg: &Config) -> String {
    if cfg.theme_discord {
        crate::theme::discord_css(cfg.theme)
    } else {
        String::new()
    }
}

fn theme_script(cfg: &Config) -> String {
    let css = theme_css_for(cfg);
    format!(
        "{THEME_RUNTIME}window.__PALLADIUM_SET_THEME__({});\n",
        Value::String(css)
    )
}

const PROXY_SHIM: &str = include_str!("proxy_shim.js");

const VOICE_SHIM: &str = include_str!("voice_shim.js");

fn proxy_script() -> String {
    match crate::proxy_base() {
        Some(base) => PROXY_SHIM.replace("__PALLADIUM_PROXY_BASE__", &base),
        None => String::new(),
    }
}

fn seed_script() -> String {
    let pending = crate::import::take_pending();
    if pending.mod_settings.is_none() && pending.quick_css.is_none() {
        return String::new();
    }

    let mut out = String::from("(function(){try{\n");
    if let Some(settings) = pending.mod_settings {
        out.push_str(&format!(
            "localStorage.setItem('VencordSettings', {});\n",
            Value::String(settings)
        ));
    }
    if let Some(css) = pending.quick_css {
        out.push_str(&format!(
            "localStorage.setItem('VencordQuickCss', {});\n",
            Value::String(css)
        ));
    }
    out.push_str("}catch(e){console.warn('[palladium] seed failed',e);}})();\n");
    out
}

pub fn build_script(bundle: Option<&ModBundle>, cfg: &Config) -> String {
    let seed = seed_script();
    if !seed.is_empty() {
        log::info!("seeding imported client mod settings into the webview");
    }

    let proxy = proxy_script();
    let theme = theme_script(cfg);
    if cfg.theme_discord {
        log::info!("applying the {} theme to discord", cfg.theme.slug());
    }

    let Some(bundle) = bundle else {
        return format!("{seed}{proxy}{VOICE_SHIM}{theme}");
    };

    let css_literal = Value::String(bundle.css.clone()).to_string();
    let name_literal = Value::String(bundle.client_mod.display_name().to_string()).to_string();

    format!(
        r#"{seed}{proxy}{voice}{theme}
(function () {{
  "use strict";

  if (window.__PALLADIUM__) return;
  window.__PALLADIUM__ = {{ mod: {name_literal}, version: {version} }};

  var css = {css_literal};
  if (!css) return;

  function injectCss() {{
    try {{
      var sheet = new CSSStyleSheet();
      sheet.replaceSync(css);
      document.adoptedStyleSheets = document.adoptedStyleSheets.concat(sheet);
      return true;
    }} catch (e) {{
      var target = document.head || document.documentElement;
      if (!target) return false;
      var style = document.createElement("style");
      style.id = "palladium-mod-css";
      style.textContent = css;
      target.appendChild(style);
      return true;
    }}
  }}

  if (!injectCss()) {{
    document.addEventListener("DOMContentLoaded", injectCss, {{ once: true }});
  }}
}})();

{js}
"#,
        seed = seed,
        proxy = proxy,
        voice = VOICE_SHIM,
        theme = theme,
        version = Value::String(env!("CARGO_PKG_VERSION").to_string()),
        css_literal = css_literal,
        name_literal = name_literal,
        js = bundle.js,
    )
}

pub fn user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/150.0.0.0 Safari/537.36"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClientMod;

    #[test]
    fn css_containing_quotes_and_newlines_is_escaped() {
        let bundle = ModBundle {
            client_mod: ClientMod::Vencord,
            js: "/* mod */".into(),
            css: "body::after { content: \"</script>\\n\"; }".into(),
        };
        let script = build_script(Some(&bundle), &Config::default());
        assert!(!script.contains("content: \"</script>"));
        assert!(script.contains(r#"\"<\/script>"#) || script.contains(r#"\"</script>"#));
        assert!(script.contains("/* mod */"));
    }

    #[test]
    fn user_agent_is_exact() {
        assert_eq!(
            user_agent(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36"
        );
    }

    #[test]
    fn vanilla_mode_still_gets_the_theme_runtime() {
        let script = build_script(None, &Config::default());
        assert!(script.contains("__PALLADIUM_SET_THEME__"));
    }

    #[test]
    fn mod_js_is_appended_after_the_bootstrap() {
        let bundle = ModBundle {
            client_mod: ClientMod::Equicord,
            js: "var Equicord = 1;".into(),
            css: String::new(),
        };
        let script = build_script(Some(&bundle), &Config::default());
        let theme = script.find("__PALLADIUM_SET_THEME__").unwrap();
        let bootstrap = script.find("__PALLADIUM__ =").unwrap();
        let mod_js = script.find("var Equicord = 1;").unwrap();
        assert!(theme < bootstrap, "theme runtime is set up first");
        assert!(bootstrap < mod_js, "mod js must come after the bootstrap");
    }

    #[test]
    fn discord_theming_is_off_unless_opted_in() {
        let off = Config::default();
        assert!(!off.theme_discord);
        assert!(theme_css_for(&off).is_empty());

        let on = Config {
            theme_discord: true,
            theme: crate::config::Theme::Gallium,
            ..Config::default()
        };
        let css = theme_css_for(&on);
        assert!(css.contains("--background-primary:#000000"));
        assert!(css.contains("--bg-base-primary"));
    }

    #[test]
    fn every_theme_produces_css() {
        use crate::config::Theme;
        for theme in [
            Theme::Palladium,
            Theme::Oxygen,
            Theme::Hydrogen,
            Theme::Plutonium,
            Theme::Uranium,
            Theme::Iron,
            Theme::Gallium,
        ] {
            let cfg = Config {
                theme,
                theme_discord: true,
                ..Config::default()
            };
            let css = theme_css_for(&cfg);
            assert!(!css.is_empty(), "{} produced no css", theme.slug());
            assert!(css.contains("--text-normal"), "{}", theme.slug());
        }
    }
}
