use serde_json::Value;

use crate::mods::ModBundle;

const SETTINGS_UI: &str = include_str!("settings_ui.js");

pub fn build_script(bundle: Option<&ModBundle>) -> String {
    let Some(bundle) = bundle else {
        return SETTINGS_UI.to_string();
    };

    let css_literal = Value::String(bundle.css.clone()).to_string();
    let name_literal = Value::String(bundle.client_mod.display_name().to_string()).to_string();

    format!(
        r#"(function () {{
  "use strict";

  if (window.__TAURICORD__) return;
  window.__TAURICORD__ = {{ mod: {name_literal}, version: {version} }};

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
      style.id = "tauricord-mod-css";
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

{settings_ui}
"#,
        version = Value::String(env!("CARGO_PKG_VERSION").to_string()),
        css_literal = css_literal,
        name_literal = name_literal,
        js = bundle.js,
        settings_ui = SETTINGS_UI,
    )
}

pub fn user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/144.0.0.0 Safari/537.36"
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
        let script = build_script(Some(&bundle));
        assert!(!script.contains("content: \"</script>"));
        assert!(script.contains(r#"\"<\/script>"#) || script.contains(r#"\"</script>"#));
        assert!(script.contains("/* mod */"));
    }

    #[test]
    fn user_agent_is_exact() {
        assert_eq!(
            user_agent(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
        );
    }

    #[test]
    fn vanilla_mode_still_gets_the_settings_panel() {
        let script = build_script(None);
        assert!(script.contains("tauricord_snapshot"));
    }

    #[test]
    fn settings_panel_comes_after_the_mod_bundle() {
        let bundle = ModBundle {
            client_mod: ClientMod::Equicord,
            js: "var Equicord = 1;".into(),
            css: String::new(),
        };
        let script = build_script(Some(&bundle));
        let bootstrap = script.find("__TAURICORD__").unwrap();
        let mod_js = script.find("var Equicord = 1;").unwrap();
        let settings = script.find("tauricord_snapshot").unwrap();
        assert!(bootstrap < mod_js, "mod js must come after the bootstrap");
        assert!(mod_js < settings, "settings panel comes last");
    }
}
