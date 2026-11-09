// Keep the pre-egui view implementation; the Tauri shell owns navigation and the session.
window.addEventListener("DOMContentLoaded", async () => {
  const host = parent.__MAGIC_WORKSPACE_HOSTS__?.get(window.frameElement?.id);
  try {
    const view = location.pathname.includes("animation-workspace") ? "animation" : "project";
    const snapshot = await host.invoke("session_snapshot");
    state.project = snapshot.project;
    state.data = snapshot.data;
    state.view = view;
    document.body.classList.remove("at-home");
    document.querySelectorAll(".view").forEach(node => node.classList.toggle("active", node.id === `view-${view}`));
    // Parent navigation stays available regardless of completion/readiness indicators.
    setView = async next => {
      if (typeof motion !== "undefined" && motion.character) sessionStorage.setItem(`magic.restore.character:${state.project}`, motion.character);
      await host.navigate(next); return true;
    };
    appShell.renderFilmTarget = () => {};
    projectView.prerequisite = id => {
      const stages = state.data?.status?.stages || [];
      const done = id === "lyrics" ? state.lyrics?.source === "canonical"
        : id === "assets" ? !!state.data?.assets?.assets?.length
        : id === "storyboard" ? state.data?.storyboard?.complete
        : stages.find(stage => stage.id === id)?.done;
      return { id, state: done ? "ready" : "needs-attention" };
    };
    await loadComponents();
    await loadSongLyrics();
    const closeApps = document.getElementById("project-close-apps");
    if (closeApps) { closeApps.disabled = true; closeApps.title = "Close each native editor through its own window"; }
    if (view === "project") projectView.render();
    else {
      connectRestoredAnimation();
      window.addEventListener("message", event => {
        if (event.origin !== location.origin || !event.data?.magicOpenAnimationCharacter) return;
        const { character } = event.data.magicOpenAnimationCharacter;
        if (character && typeof animationView !== "undefined") {
          sessionStorage.setItem(`magic.restore.character:${state.project}`, character);
          void animationView.openCharacter(character);
        }
      });
      const character = sessionStorage.getItem(`magic.restore.character:${state.project}`);
      if (character) { motion.character = character; motion.explicitCharacter = true; }
      await animationOpen();
    }
    window.workshopDrawers?.close(false);
    acFeedback.report("ready");
  } catch (error) {
    const message = String(error);
    acFeedback?.errors?.push({ message: message.slice(0, 300), source: location.pathname.split("/").pop() || "embedded.js" });
    acFeedback?.report("error");
    host?.report(message);
  }
});
