(function () {
  "use strict";

  var invoke = null;
  function tauri(cmd, args) {
    if (!invoke) {
      var g = window.__TAURI__;
      invoke = (g && g.core && g.core.invoke) || (g && g.invoke);
    }
    if (!invoke) return Promise.reject(new Error("Tauricord IPC unavailable"));
    return invoke(cmd, args || {});
  }

  var SECTION_ID = "tauricord-settings-tab";
  var PANEL_ID = "tauricord-settings-panel";
  var snapshot = null;

  var STYLE = `
#${PANEL_ID} { padding: 60px 40px 80px; max-width: 740px; color: var(--text-normal, #dbdee1); }
#${PANEL_ID} h1 { font-size: 20px; font-weight: 600; margin: 0 0 4px; color: var(--header-primary, #f2f3f5); }
#${PANEL_ID} .tc-sub { font-size: 13px; color: var(--text-muted, #949ba4); margin-bottom: 28px; }
#${PANEL_ID} h2 { font-size: 12px; font-weight: 700; letter-spacing: .02em; text-transform: uppercase;
  color: var(--header-secondary, #b5bac1); margin: 28px 0 10px; }
#${PANEL_ID} .tc-card { background: var(--background-secondary, #2b2d31); border-radius: 8px; padding: 4px 16px; }
#${PANEL_ID} .tc-row { display: flex; align-items: center; justify-content: space-between; gap: 16px;
  padding: 14px 0; border-bottom: 1px solid var(--background-modifier-accent, rgba(255,255,255,.06)); }
#${PANEL_ID} .tc-row:last-child { border-bottom: none; }
#${PANEL_ID} .tc-label { font-size: 15px; font-weight: 500; color: var(--header-primary, #f2f3f5); }
#${PANEL_ID} .tc-desc { font-size: 13px; color: var(--text-muted, #949ba4); margin-top: 2px; line-height: 1.4; }
#${PANEL_ID} .tc-choices { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 10px; margin: 6px 0 2px; }
#${PANEL_ID} .tc-choice { text-align: left; cursor: pointer; border-radius: 8px; padding: 14px;
  background: var(--background-tertiary, #1e1f22); border: 2px solid transparent; color: inherit; font: inherit; }
#${PANEL_ID} .tc-choice:hover { background: var(--background-modifier-hover, #35373c); }
#${PANEL_ID} .tc-choice[aria-checked="true"] { border-color: var(--brand-experiment, #5865f2); }
#${PANEL_ID} .tc-choice-name { font-size: 15px; font-weight: 600; }
#${PANEL_ID} .tc-choice-desc { font-size: 12px; color: var(--text-muted, #949ba4); margin-top: 4px; line-height: 1.4; }
#${PANEL_ID} .tc-switch { flex: 0 0 auto; width: 40px; height: 24px; border-radius: 12px; border: none; cursor: pointer;
  background: var(--background-tertiary, #80848e); position: relative; transition: background .15s ease; }
#${PANEL_ID} .tc-switch[aria-checked="true"] { background: var(--brand-experiment, #5865f2); }
#${PANEL_ID} .tc-switch::after { content: ""; position: absolute; top: 4px; left: 4px; width: 16px; height: 16px;
  border-radius: 50%; background: #fff; transition: transform .15s ease; }
#${PANEL_ID} .tc-switch[aria-checked="true"]::after { transform: translateX(16px); }
#${PANEL_ID} select { background: var(--background-tertiary, #1e1f22); color: inherit; border: none;
  border-radius: 4px; padding: 8px 10px; font: inherit; cursor: pointer; }
#${PANEL_ID} .tc-actions { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 12px; }
#${PANEL_ID} button.tc-btn { border: none; border-radius: 4px; padding: 9px 16px; font-size: 14px; font-weight: 500;
  cursor: pointer; background: var(--brand-experiment, #5865f2); color: #fff; }
#${PANEL_ID} button.tc-btn.tc-secondary { background: var(--background-tertiary, #4e5058); }
#${PANEL_ID} button.tc-btn:hover { filter: brightness(1.1); }
#${PANEL_ID} .tc-note { margin-top: 10px; font-size: 13px; line-height: 1.5; border-radius: 6px; padding: 12px 14px;
  background: var(--background-tertiary, #1e1f22); border-left: 3px solid var(--brand-experiment, #5865f2); }
#${PANEL_ID} .tc-note.tc-warn { border-left-color: var(--status-danger, #f23f43); }
#${PANEL_ID} .tc-toast { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%);
  background: var(--background-floating, #111214); color: var(--text-normal, #dbdee1); padding: 12px 18px;
  border-radius: 8px; font-size: 14px; box-shadow: 0 8px 16px rgba(0,0,0,.24); z-index: 10000; }
`;

  function ensureStyle() {
    if (document.getElementById("tauricord-settings-style")) return;
    var el = document.createElement("style");
    el.id = "tauricord-settings-style";
    el.textContent = STYLE;
    (document.head || document.documentElement).appendChild(el);
  }

  function h(tag, props, children) {
    var el = document.createElement(tag);
    Object.keys(props || {}).forEach(function (k) {
      if (k === "class") el.className = props[k];
      else if (k === "text") el.textContent = props[k];
      else if (k.slice(0, 2) === "on") el.addEventListener(k.slice(2), props[k]);
      else el.setAttribute(k, props[k]);
    });
    (children || []).forEach(function (c) {
      if (c) el.appendChild(c);
    });
    return el;
  }

  function toast(message) {
    var existing = document.querySelector(".tc-toast");
    if (existing) existing.remove();
    var el = h("div", { class: "tc-toast", text: message });
    document.body.appendChild(el);
    setTimeout(function () {
      el.remove();
    }, 4000);
  }

  function row(label, desc, control) {
    return h("div", { class: "tc-row" }, [
      h("div", {}, [
        h("div", { class: "tc-label", text: label }),
        desc ? h("div", { class: "tc-desc", text: desc }) : null,
      ]),
      control,
    ]);
  }

  function toggle(checked, onChange) {
    var btn = h("button", {
      class: "tc-switch",
      role: "switch",
      "aria-checked": String(!!checked),
      onclick: function () {
        var next = btn.getAttribute("aria-checked") !== "true";
        btn.setAttribute("aria-checked", String(next));
        onChange(next);
      },
    });
    return btn;
  }

  function applyPatch(patch) {
    return tauri("tauricord_update_config", { patch: patch })
      .then(function (result) {
        snapshot.config = result.config;
        if (result.reloading) {
          toast("Rebuilding the client…");
        } else if (result.needs_restart) {
          toast("Saved. Restart Tauricord for this to take effect.");
        } else {
          toast("Saved.");
        }
        return result;
      })
      .catch(function (e) {
        toast("Could not save: " + e);
        throw e;
      });
  }

  var MODS = [
    ["equicord", "Equicord", "Vencord fork with a much larger plugin set."],
    ["vencord", "Vencord", "The original client mod. Smaller and more conservative."],
    ["none", "Vanilla", "No client mod. Plain Discord in a Tauri window."],
  ];

  var BRANCHES = [
    ["stable", "Stable"],
    ["ptb", "PTB"],
    ["canary", "Canary"],
  ];

  function buildPanel() {
    var cfg = snapshot.config;

    var choices = h(
      "div",
      { class: "tc-choices" },
      MODS.map(function (m) {
        var btn = h("button", {
          class: "tc-choice",
          role: "radio",
          "aria-checked": String(cfg.client_mod === m[0]),
          onclick: function () {
            if (cfg.client_mod === m[0]) return;
            choices.querySelectorAll(".tc-choice").forEach(function (n) {
              n.setAttribute("aria-checked", "false");
            });
            btn.setAttribute("aria-checked", "true");
            applyPatch({ client_mod: m[0] });
          },
        });
        btn.appendChild(h("div", { class: "tc-choice-name", text: m[1] }));
        btn.appendChild(h("div", { class: "tc-choice-desc", text: m[2] }));
        return btn;
      })
    );

    var branchSelect = h(
      "select",
      {
        onchange: function (e) {
          applyPatch({ discord_branch: e.target.value });
        },
      },
      BRANCHES.map(function (b) {
        var o = h("option", { value: b[0], text: b[1] });
        if (cfg.discord_branch === b[0]) o.setAttribute("selected", "selected");
        return o;
      })
    );

    var panel = h("div", { id: PANEL_ID }, [
      h("h1", { text: "Tauricord" }),
      h("div", {
        class: "tc-sub",
        text: "v" + snapshot.app_version + " · Discord on Tauri, without Electron",
      }),

      h("h2", { text: "Client mod" }),
      h("div", { class: "tc-card" }, [
        h("div", { class: "tc-row" }, [choices]),
      ]),
      h("div", {
        class: "tc-note",
        text: "Changing the client mod rebuilds the window, because the mod has to be injected before Discord starts loading.",
      }),

      h("h2", { text: "Rich Presence" }),
      h("div", { class: "tc-card" }, [
        row(
          "arRPC server",
          "Lets games set your Discord status. Serves discord-ipc and a bridge on port " +
            snapshot.arrpc_port +
            ". Needs a restart to change.",
          toggle(cfg.arrpc_enabled, function (v) {
            applyPatch({ arrpc_enabled: v });
          })
        ),
      ]),

      h("h2", { text: "Client" }),
      h("div", { class: "tc-card" }, [
        row("Discord branch", "Which flavour of Discord to load.", branchSelect),
        row(
          "Always update the mod",
          "Re-download the client mod on every launch instead of using the cache.",
          toggle(cfg.always_update_mod, function (v) {
            applyPatch({ always_update_mod: v });
          })
        ),
        row(
          "Minimise to tray",
          "Closing the window hides it instead of quitting.",
          toggle(cfg.minimize_to_tray, function (v) {
            applyPatch({ minimize_to_tray: v });
          })
        ),
      ]),

      h("h2", { text: "Maintenance" }),
      h("div", { class: "tc-actions" }, [
        h("button", {
          class: "tc-btn tc-secondary",
          text: "Re-download client mod",
          onclick: function () {
            tauri("tauricord_clear_mod_cache")
              .then(function () {
                toast("Cache cleared. Restarting…");
                return tauri("tauricord_relaunch");
              })
              .catch(function (e) {
                toast("Failed: " + e);
              });
          },
        }),
        h("button", {
          class: "tc-btn tc-secondary",
          text: "Restart Tauricord",
          onclick: function () {
            tauri("tauricord_relaunch");
          },
        }),
      ]),
    ]);

    if (typeof RTCPeerConnection === "undefined") {
      panel.appendChild(
        h("div", {
          class: "tc-note tc-warn",
          text:
            "Voice and video are unavailable: this system's WebKitGTK was built without WebRTC support, " +
            "so RTCPeerConnection does not exist. Text, file uploads and Rich Presence are unaffected. " +
            "Fixing this needs a WebKitGTK built with ENABLE_WEB_RTC=ON.",
        })
      );
    }

    return panel;
  }

  function contentRegion() {
    return (
      document.querySelector('[class*="contentRegion"]') ||
      document.querySelector('[class*="contentColumn"]')
    );
  }

  function showPanel() {
    var region = contentRegion();
    if (!region) return false;

    var scroller = region.querySelector('[class*="contentColumn"]') || region;
    Array.prototype.forEach.call(scroller.children, function (child) {
      child.style.display = "none";
    });
    var old = document.getElementById(PANEL_ID);
    if (old) old.remove();

    ensureStyle();
    scroller.appendChild(buildPanel());
    return true;
  }

  function markSelected(item) {
    var nav = item.parentElement;
    if (!nav) return;
    Array.prototype.forEach.call(nav.querySelectorAll('[role="tab"]'), function (t) {
      if (t !== item) t.setAttribute("aria-selected", "false");
    });
    item.setAttribute("aria-selected", "true");
  }

  function injectTab() {
    if (document.getElementById(SECTION_ID)) return;

    var tabs = document.querySelectorAll('nav [role="tab"], [class*="sidebar"] [role="tab"]');
    if (!tabs.length) return;

    var template = tabs[tabs.length - 1];
    var item = template.cloneNode(true);
    item.id = SECTION_ID;
    item.setAttribute("aria-selected", "false");
    item.removeAttribute("data-list-item-id");

    var labelled = item.querySelector('[class*="text"]') || item;
    labelled.textContent = "Tauricord";

    item.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      markSelected(item);
      if (!showPanel()) toast("Could not open the Tauricord panel");
    });

    template.parentElement.appendChild(item);
  }

  var observer = new MutationObserver(function () {
    if (document.querySelector('[class*="sidebar"] [role="tab"]')) injectTab();
  });

  function start() {
    tauri("tauricord_snapshot")
      .then(function (s) {
        snapshot = s;
        ensureStyle();
        observer.observe(document.body, { childList: true, subtree: true });
        window.__TAURICORD__ = window.__TAURICORD__ || {};
        window.__TAURICORD__.openSettings = function () {
          injectTab();
          var tab = document.getElementById(SECTION_ID);
          if (tab) tab.click();
          else toast("Open Discord's settings first, then pick Tauricord.");
        };
      })
      .catch(function (e) {
        console.warn("[Tauricord] settings panel unavailable:", e);
      });
  }

  if (document.body) start();
  else document.addEventListener("DOMContentLoaded", start, { once: true });
})();
