// The original views now use the parent Tauri session directly, not the retired /exec server.
(() => {
  const host = parent.__MAGIC_WORKSPACE_HOSTS__?.get(window.frameElement?.id);
  if (!host) throw new Error("The workspace must be opened inside Magic Studio");
  window.__TAURI__ = {
    core: { invoke: host.invoke, convertFileSrc: host.fileUrl },
    dialog: { open: host.openDialog },
    opener: { openPath: host.openPath, openUrl: host.openUrl },
    event: { listen: host.listen },
  };
  window.addEventListener("error", event => host.report(String(event.error || event.message)));
  window.addEventListener("unhandledrejection", event => host.report(String(event.reason)));
})();
