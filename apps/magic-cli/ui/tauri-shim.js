// Stand-in for window.__TAURI__ so the Magic UI runs in Tauri or a browser against the live engine bridge.
(function () {
  const base = () => window.__MAGIC_BRIDGE_URL__ || "";
  async function exec(id, params) {
    const r = await fetch(`${base()}/exec`, { method: "POST", body: JSON.stringify({ id, params: params || {} }) });
    const j = await r.json();
    if (j.error) throw new Error(j.error);
    return j.result;
  }
  window.__TAURI__ = {
    core: {
      invoke: (cmd, args) => cmd === "execute_command" ? exec(args.id, args.params) : exec(cmd, args || {}).catch(() => null),
      convertFileSrc: (p) => `${base()}/file?p=${encodeURIComponent(p)}`,
    },
    dialog: { open: async () => null },
    opener: { openUrl: async () => null, openPath: async () => null },
    event: { listen: async () => () => {} },
  };
})();
