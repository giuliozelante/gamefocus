const { invoke } = window.__TAURI__.core;

const els = {};

function showError(message) {
  els.error.hidden = !message;
  els.error.textContent = message || "";
}

function renderStatus(status) {
  if (status.active) {
    els.statusLine.textContent = `Active — ${status.profileName || "session"} · focus=${status.focusApplied} boost=${status.boostApplied} · parked ${status.suspended}`;
  } else {
    els.statusLine.textContent = "Idle — no active session";
  }
  els.focusToggle.checked = status.focus;
  els.boostToggle.checked = status.boost;
  if (status.profilePath) {
    els.profileSelect.value = status.profilePath;
  }
}

async function refresh() {
  showError("");
  try {
    const status = await invoke("get_status");
    renderStatus(status);
  } catch (e) {
    showError(String(e));
  }
}

async function loadProfiles() {
  const profiles = await invoke("list_profiles");
  els.profileSelect.innerHTML = "";
  for (const p of profiles) {
    const opt = document.createElement("option");
    opt.value = p.path;
    opt.textContent = p.name;
    els.profileSelect.appendChild(opt);
  }
  if (!profiles.length) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = "No profiles found";
    els.profileSelect.appendChild(opt);
  }
}

async function onToggle() {
  showError("");
  try {
    const status = await invoke("set_toggles", {
      focus: els.focusToggle.checked,
      boost: els.boostToggle.checked,
    });
    renderStatus(status);
  } catch (e) {
    showError(String(e));
  }
}

async function onProfileChange() {
  showError("");
  try {
    const status = await invoke("set_profile", { path: els.profileSelect.value });
    renderStatus(status);
  } catch (e) {
    showError(String(e));
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  els.statusLine = document.querySelector("#status-line");
  els.profileSelect = document.querySelector("#profile-select");
  els.focusToggle = document.querySelector("#focus-toggle");
  els.boostToggle = document.querySelector("#boost-toggle");
  els.startBtn = document.querySelector("#start-btn");
  els.endBtn = document.querySelector("#end-btn");
  els.refreshBtn = document.querySelector("#refresh-btn");
  els.error = document.querySelector("#error");

  els.focusToggle.addEventListener("change", onToggle);
  els.boostToggle.addEventListener("change", onToggle);
  els.profileSelect.addEventListener("change", onProfileChange);
  els.startBtn.addEventListener("click", async () => {
    showError("");
    try {
      renderStatus(await invoke("start_focus_session"));
    } catch (e) {
      showError(String(e));
    }
  });
  els.endBtn.addEventListener("click", async () => {
    showError("");
    try {
      renderStatus(await invoke("end_focus_session"));
    } catch (e) {
      showError(String(e));
    }
  });
  els.refreshBtn.addEventListener("click", refresh);

  try {
    await loadProfiles();
    await refresh();
  } catch (e) {
    showError(String(e));
  }
});
