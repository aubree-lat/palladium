const invoke = (cmd, args) => {
  const g = window.__TAURI__;
  const fn = (g && g.core && g.core.invoke) || (g && g.invoke);
  if (!fn) return Promise.reject(new Error("tauri ipc unavailable"));
  return fn(cmd, args || {});
};

const MODS = [
  ["equicord", "equicord", "vencord fork with a much larger plugin set"],
  ["vencord", "vencord", "the original client mod, smaller and more conservative"],
  ["none", "vanilla", "no client mod, plain discord in a tauri window"],
];

const THEMES = [
  ["palladium", "#d8b4fe", "#1a0030"],
  ["oxygen", "#52525b", "#f4f4f6"],
  ["hydrogen", "#d4d4d8", "#111113"],
  ["plutonium", "#6ba3ff", "#0a1533"],
  ["uranium", "#5fffa0", "#062b17"],
  ["iron", "#b8bcc4", "#1a1a1e"],
  ["gallium", "#ffffff", "#000000"],
];

let snapshot = null;

function applyTheme(name) {
  document.documentElement.dataset.theme = name;
}

function renderThemes(active) {
  const host = document.getElementById("themes");
  host.textContent = "";
  THEMES.forEach(([name, accent, bg]) => {
    const btn = document.createElement("button");
    btn.className = "swatch";
    btn.setAttribute("role", "radio");
    btn.setAttribute("aria-checked", String(active === name));
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.style.background =
      "linear-gradient(135deg, " + accent + " 0 50%, " + bg + " 50% 100%)";
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = name;
    btn.appendChild(chip);
    btn.appendChild(label);
    btn.addEventListener("click", () => {
      host.querySelectorAll(".swatch").forEach((n) =>
        n.setAttribute("aria-checked", "false")
      );
      btn.setAttribute("aria-checked", "true");
      applyTheme(name);
      patch({ theme: name });
    });
    host.appendChild(btn);
  });
}

function toast(message, kind) {
  const old = document.querySelector(".toast");
  if (old) old.remove();
  const el = document.createElement("div");
  el.className = "toast" + (kind ? " " + kind : "");
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}

function patch(body) {
  return invoke("palladium_update_config", { patch: body })
    .then((result) => {
      snapshot.config = result.config;
      if (result.reloading) toast("rebuilding the client...");
      else if (result.needs_restart) toast("saved, restart for this to take effect");
      else toast("saved");
      return result;
    })
    .catch((e) => {
      toast("could not save: " + e, "warn");
      throw e;
    });
}

function bindSwitch(id, value, onChange) {
  const el = document.getElementById(id);
  el.setAttribute("aria-checked", String(!!value));
  el.addEventListener("click", () => {
    const next = el.getAttribute("aria-checked") !== "true";
    el.setAttribute("aria-checked", String(next));
    onChange(next);
  });
}

function bindSelect(id, value, onChange) {
  const el = document.getElementById(id);
  el.value = value;
  el.addEventListener("change", () => onChange(el.value));
}

function renderMods(active) {
  const host = document.getElementById("mods");
  host.textContent = "";
  MODS.forEach(([id, name, desc]) => {
    const btn = document.createElement("button");
    btn.className = "choice";
    btn.setAttribute("role", "radio");
    btn.setAttribute("aria-checked", String(active === id));
    btn.innerHTML =
      '<span class="dot" aria-hidden="true"></span><span>' +
      '<span class="name"></span><span class="desc"></span></span>';
    btn.querySelector(".name").textContent = name;
    btn.querySelector(".desc").textContent = desc;
    btn.addEventListener("click", () => {
      if (btn.getAttribute("aria-checked") === "true") return;
      host.querySelectorAll(".choice").forEach((n) =>
        n.setAttribute("aria-checked", "false")
      );
      btn.setAttribute("aria-checked", "true");
      patch({ client_mod: id });
    });
    host.appendChild(btn);
  });
}

function renderImports(sources) {
  const host = document.getElementById("imports");
  host.textContent = "";

  if (!sources.length) {
    const row = document.createElement("div");
    row.className = "row";
    row.innerHTML =
      '<div><div class="name">nothing to import</div>' +
      '<div class="desc">no vesktop or equibop config found on this machine</div></div>';
    host.appendChild(row);
    return;
  }

  sources.forEach((s) => {
    const row = document.createElement("div");
    row.className = "row";
    const info = document.createElement("div");
    const name = document.createElement("div");
    name.className = "name";
    name.textContent = s.name;
    const desc = document.createElement("div");
    desc.className = "desc";
    const bits = [];
    if (s.plugins) bits.push(s.plugins + " plugins enabled");
    if (s.themes) bits.push(s.themes + " themes");
    if (s.quick_css) bits.push("quickcss");
    desc.textContent = bits.length ? bits.join(", ") : "client settings only";
    info.appendChild(name);
    info.appendChild(desc);

    const btn = document.createElement("button");
    btn.className = "btn";
    btn.textContent = "import";
    btn.addEventListener("click", () => {
      btn.disabled = true;
      btn.textContent = "importing...";
      invoke("palladium_import_settings", { source: s.id })
        .then((res) => {
          snapshot.config = res.config;
          document.getElementById("import-note").className = "note ok";
          document.getElementById("import-note").textContent =
            "imported from " + s.name + ", restart to apply";
          toast("imported, restart to apply");
          btn.textContent = "imported";
          hydrate(snapshot);
        })
        .catch((e) => {
          btn.disabled = false;
          btn.textContent = "import";
          toast("import failed: " + e, "warn");
        });
    });

    row.appendChild(info);
    row.appendChild(btn);
    host.appendChild(row);
  });
}

function hydrate(s) {
  const cfg = s.config;
  document.getElementById("version").textContent =
    "v" + s.app_version + " - discord on tauri, without electron";
  document.getElementById("port").textContent = s.arrpc_port;
  renderMods(cfg.client_mod);
}

invoke("palladium_snapshot")
  .then((s) => {
    snapshot = s;
    const cfg = s.config;
    applyTheme(cfg.theme);
    hydrate(s);
    renderThemes(cfg.theme);
    renderImports(s.import_sources || []);

    bindSwitch("theme-discord", cfg.theme_discord, (v) =>
      patch({ theme_discord: v })
    );

    bindSwitch("arrpc", cfg.arrpc_enabled, (v) => patch({ arrpc_enabled: v }));
    bindSwitch("always-update", cfg.always_update_mod, (v) =>
      patch({ always_update_mod: v })
    );
    bindSwitch("tray", cfg.minimize_to_tray, (v) => patch({ minimize_to_tray: v }));
    bindSelect("branch", cfg.discord_branch, (v) => patch({ discord_branch: v }));
    bindSelect("backend", cfg.linux_backend, (v) => patch({ linux_backend: v }));

    document.getElementById("refetch").addEventListener("click", () => {
      invoke("palladium_clear_mod_cache")
        .then(() => {
          toast("cache cleared, restarting...");
          return invoke("palladium_relaunch");
        })
        .catch((e) => toast("failed: " + e, "warn"));
    });

    document.getElementById("restart").addEventListener("click", () => {
      invoke("palladium_relaunch");
    });
  })
  .catch((e) => {
    document.getElementById("version").textContent = "could not load settings: " + e;
  });
