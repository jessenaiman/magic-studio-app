// The original views now use the parent Tauri session directly, not the retired /exec server.
(() => {
  const host = parent.__MAGIC_WORKSPACE_HOSTS__?.get(window.frameElement?.id);
  if (!host) throw new Error("The workspace must be opened inside Magic Studio");
  window.__magicWorkspaceVisible = host.isActive();
  window.addEventListener("message", event => {
    if (event.source !== parent || event.origin !== location.origin || !event.data?.magicWorkspaceVisibility) return;
    const wasVisible = window.__magicWorkspaceVisible;
    window.__magicWorkspaceVisible = event.data.active;
    if (event.data.active && !wasVisible && typeof state !== "undefined") {
      void host.invoke("session_snapshot").then(async snapshot => {
        if (!window.__magicWorkspaceVisible || snapshot.project !== state.project) return;
        state.data = snapshot.data;
        if (state.view === "project") { await loadSongLyrics(); projectView.render(); }
        if (state.view === "design" && !wordsPanel.designs.size && !Object.keys(wordsPanel.styleEdits).length) await wordsOpen();
      }).catch(error => host.report(String(error)));
    }
    if (!event.data.active) {
      if (typeof animationPause === "function") animationPause();
      if (typeof wordsPanel !== "undefined") { wordsPanel.playing = false; if (typeof wordsVideo === "function") wordsVideo("pause"); }
      for (const media of document.querySelectorAll("audio,video")) media.pause();
      if (typeof magicTheatre !== "undefined" && magicTheatre.ready) magicTheatre.seek(motion.time);
    }
  });
  window.__TAURI__ = {
    core: { invoke: host.invoke, convertFileSrc: host.fileUrl },
    dialog: { open: host.openDialog },
    opener: { openPath: host.openPath, openUrl: host.openUrl },
    event: { listen: host.listen },
  };
  window.addEventListener("error", event => host.report(String(event.error || event.message)));
  window.addEventListener("unhandledrejection", event => host.report(String(event.reason)));
})();
