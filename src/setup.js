const invoke = (cmd, args) => {
  const g = window.__TAURI__;
  const fn = (g && g.core && g.core.invoke) || (g && g.invoke);
  if (!fn) return Promise.reject(new Error("tauri ipc unavailable"));
  return fn(cmd, args || {});
};

let selectedMod = "equicord";
let importFrom = null;

const choices = [...document.querySelectorAll(".choice")];
choices.forEach((btn) => {
  btn.addEventListener("click", () => {
    choices.forEach((b) => b.setAttribute("aria-checked", "false"));
    btn.setAttribute("aria-checked", "true");
    selectedMod = btn.dataset.mod;
  });
});

const arrpc = document.getElementById("arrpc");
arrpc.addEventListener("click", () => {
  arrpc.setAttribute(
    "aria-checked",
    arrpc.getAttribute("aria-checked") === "true" ? "false" : "true"
  );
});

function renderImports(sources) {
  if (!sources.length) return;

  document.getElementById("import-section").hidden = false;
  const host = document.getElementById("imports");

  const mk = (id, name, desc, checked) => {
    const row = document.createElement("div");
    row.className = "row";
    row.style.cursor = "pointer";
    row.innerHTML =
      '<div style="display:flex;gap:.8rem;align-items:flex-start">' +
      '<span class="dot" aria-hidden="true"></span>' +
      '<div><div class="name"></div><div class="desc"></div></div></div>';
    row.querySelector(".name").textContent = name;
    row.querySelector(".desc").textContent = desc;
    row.setAttribute("role", "radio");
    row.setAttribute("aria-checked", String(checked));
    row.addEventListener("click", () => {
      host.querySelectorAll('[role="radio"]').forEach((n) =>
        n.setAttribute("aria-checked", "false")
      );
      row.setAttribute("aria-checked", "true");
      importFrom = id;
    });
    return row;
  };

  sources.forEach((s) => {
    const bits = [];
    if (s.plugins) bits.push(s.plugins + " plugins");
    if (s.themes) bits.push(s.themes + " themes");
    if (s.quick_css) bits.push("quickcss");
    host.appendChild(
      mk(s.id, s.name, bits.length ? bits.join(", ") : "client settings only", false)
    );
  });

  host.appendChild(mk(null, "start fresh", "do not import anything", true));
}

const go = document.getElementById("go");
const hint = document.getElementById("hint");

go.addEventListener("click", () => {
  go.disabled = true;
  go.textContent = "setting up...";

  const finish = () =>
    invoke("palladium_finish_setup", {
      patch: {
        client_mod: selectedMod,
        arrpc_enabled: arrpc.getAttribute("aria-checked") === "true",
      },
    });

  const step = importFrom
    ? invoke("palladium_import_settings", { source: importFrom })
    : Promise.resolve();

  step
    .then(finish)
    .catch((e) => {
      go.disabled = false;
      go.textContent = "get started";
      hint.className = "note warn";
      hint.textContent = "could not finish setup: " + e;
    });
});

invoke("palladium_snapshot")
  .then((s) => {
    if (s.config && s.config.theme) {
      document.documentElement.dataset.theme = s.config.theme;
    }
    renderImports(s.import_sources || []);
  })
  .catch(() => {});
