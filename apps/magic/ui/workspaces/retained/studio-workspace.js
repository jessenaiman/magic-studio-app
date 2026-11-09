const studioInvoke = window.__TAURI__.core.invoke;
const studioToolbar = document.querySelector("#film-head");
const studioStatus = document.querySelector("#film-status");
const studioButtons = [...studioToolbar.querySelectorAll("[data-action]")];
let studioState = null;
let studioRequest = false;
let studioError = "";

function studioCommand(action) {
  return studioInvoke("execute_command", {
    id: "ui.studio.toolbar",
    params: { action, project: studioState?.project },
  });
}

function updateStudioToolbar() {
  if (!studioState) return;
  const { path, exists, dirty, reload, busy, last_render, preview, error, status } = studioState;
  const availability = {
    create: !exists, assemble: !exists, reload, save: dirty,
    "add-last": last_render, "open-preview": preview?.exists,
  };
  for (const button of studioButtons) {
    const action = button.dataset.action;
    button.hidden = Object.hasOwn(availability, action) && !availability[action];
    button.disabled = studioRequest || Boolean(busy) ||
      (dirty && !["save", "open-preview"].includes(action)) ||
      (!exists && ["open", "preview", "export"].includes(action));
  }
  const text = studioError || error || (busy ? `Running ${busy}…` : status === "Ready" ? "" : status);
  const label = text || path?.split("/").pop() || "";
  if (studioStatus.textContent !== label) studioStatus.textContent = label;
  studioStatus.title = text || path || "";
}

async function refreshStudioToolbar() {
  if (!studioRequest) {
    try {
      studioState = await studioCommand("state");
      updateStudioToolbar();
    } catch (error) {
      if (!studioState) studioStatus.textContent = String(error.message || error);
    }
  }
  setTimeout(refreshStudioToolbar, studioState?.busy ? 300 : 750);
}

studioToolbar.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-action]");
  if (!button || button.disabled || !studioState || studioRequest) return;
  studioRequest = true;
  studioError = "";
  updateStudioToolbar();
  try {
    await studioCommand(button.dataset.action);
    studioState = await studioCommand("state");
  } catch (error) {
    studioError = String(error.message || error);
  } finally {
    studioRequest = false;
    updateStudioToolbar();
  }
});
refreshStudioToolbar();
