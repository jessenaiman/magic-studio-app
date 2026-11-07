// The original views now use the parent Tauri session directly, not the retired /exec server.
(() => {
  const host = parent.__MAGIC_WORKSPACE_HOSTS__?.get(window.frameElement?.id);
  if (!host) throw new Error("The workspace must be opened inside Magic Studio");
  window.__magicWorkspaceVisible = host.isActive();
  window.__magicAuthoringMode = host.authoringMode?.() || "design";
  // Lifecycle checks include mounted editors even when their workspace is hidden.
  window.magicDraftSnapshot = () => {
    if (typeof acFeedback === "undefined" || acFeedback.phase !== "ready") return {dirty:true,drafts:["Workspace is still loading"],saving:false};
    const drafts = [];
    if (typeof wordsPanel !== "undefined") {
      if (wordsPanel.designs.size) drafts.push("Lyrics or Motion design");
      if (Object.keys(wordsPanel.styleEdits).length) drafts.push("Lyrics styling");
      for (const field of document.querySelectorAll("#words-workspace-controls input, #words-workspace-controls textarea, #words-workspace-controls select[data-initial-value]")) {
        if (field.disabled || field.readOnly) continue;
        const changed = field.tagName === "SELECT" ? field.value !== field.dataset.initialValue : ["checkbox","radio"].includes(field.type) ? field.checked !== field.defaultChecked : field.value !== field.defaultValue;
        if (changed) drafts.push(field.getAttribute("aria-label") || field.id || "Authoring field");
      }
    }
    if (typeof artworkDraft !== "undefined" && artworkDraft?.dirty) drafts.push("Artwork parts");
    if (typeof animationReview !== "undefined" && animationReview.dirty) drafts.push("Animation review");
    if (typeof theatreFeedback === "function" && theatreFeedback().live_edits === "needs_save") drafts.push("Motion keyframes");
    return {dirty:drafts.length > 0,drafts,saving:typeof animationReview !== "undefined" && !!animationReview.saving};
  };
  window.addEventListener("message", event => {
    if (event.source !== parent || event.origin !== location.origin || !event.data?.magicAssetsImported || typeof state === "undefined" || event.data.magicAssetsImported !== state.project || state.view !== "project") return;
    void host.invoke("session_snapshot").then(snapshot => {
      if (snapshot.project !== state.project) return;
      state.data = snapshot.data;
      state.open = "assets";
      projectView.render();
    }).catch(error => host.report(String(error)));
  });
  window.addEventListener("message", event => {
    if (event.source !== parent || event.origin !== location.origin || !["design", "motion"].includes(event.data?.magicAuthoringMode)) return;
    window.__magicAuthoringMode = event.data.magicAuthoringMode;
    window.setMagicAuthoringMode?.(event.data.magicAuthoringMode);
  });
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
    dialog: { open: host.openDialog, save: host.saveDialog },
    opener: { openPath: host.openPath, openUrl: host.openUrl },
    event: { listen: host.listen },
    clipboardManager: host.clipboard,
  };
  window.addEventListener("error", event => host.report(String(event.error || event.message)));
  // A Content-Security-Policy block is otherwise silent; surface it as a workspace alert.
  document.addEventListener("securitypolicyviolation", event => host.report(`Blocked by CSP (${event.violatedDirective}): ${event.blockedURI || "inline"}`));
  window.addEventListener("unhandledrejection", event => host.report(String(event.reason)));
})();
