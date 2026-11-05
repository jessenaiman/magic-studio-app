// The agent drawer: the app's own agent.
//
// Its settings are stored by the app (`agent_config`), its conversation lives in the project
// (`.agent/agent-log.json`) and every turn runs the configured command as a child process of this
// window, in the project folder, with the app's instructions appended to its system prompt.
// OpenCode uses its existing installed API/session route; no image settings are changed here.
class AgentDrawerController {
  open = false;
  turns: any[] = [];
  config: any = null;
  sending = false;
  attachments: any[] = [];
  loaded = false;
  newChat = true;
  models: any[] = [];
  timer: any = null;
  renderedReply: string | null = null;
  session: any = null;

  status(text: string) {
    $("agent-status").textContent = text;
    $("agent-status").title = text;
  }

  installedAgents(c: any) {
    return (c?.agents || []).filter((a: any) => a.installed);
  }

  runtimeLabel(c: any, chosen: string, isDefault: boolean) {
    if (!c) return "agent not configured";
    const def = isDefault ? " · system default" : "";
    const prov = c.provider ? ` · ${c.provider}` : "";
    const mod = c.model ? ` · ${c.model}` : " · CLI default (unverified)";
    return `${chosen || "no agent"}${def}${prov}${mod}`;
  }

  runtimeTooltip(c: any) {
    if (!c) return "Open settings to choose an installed agent.";
    const names = this.installedAgents(c).map((a: any) => a.name).join(", ");
    const extra = (c.extra_args || []).join(" ") || "none";
    return `Installed agents: ${names}\nSystem default: ${c.default || "none"}\nInstructions: ${c.instructions}\nExtra arguments: ${extra}`;
  }

  commandChoices(installed: any[]) {
    return installed.map((a: any) => {
      const def = a.default ? " (system default)" : "";
      const head = a.headless ? "" : " · no headless mode";
      return el("option", { value: a.name }, `${a.name}${def}${head}`);
    });
  }

  replaceChoices(select: any, options: any[]) {
    const signature = options.map((o: any) => o.value).join(",");
    if (select.dataset.signature !== signature) {
      select.replaceChildren(...options);
      select.dataset.signature = signature;
    }
  }

  fillDraft(c: any) {
    $("agent-provider").value = c?.provider || "";
    $("agent-model").value = c?.model || "";
    $("agent-thinking").value = c?.thinking || "";
    $("agent-extra").value = (c?.extra_args || []).join(" ");
    for (const id of ["agent-provider", "agent-model", "agent-extra"]) $(id).defaultValue = $(id).value;
  }

  renderConfig() {
    const c = this.config;
    const runtime = $("agent-runtime");
    const chosen = c?.agent || "";
    const isDefault = Boolean(c?.default && c.default === chosen);
    const installed = this.installedAgents(c);
    const ready = Boolean(c && installed.some((a: any) => a.name === chosen));
    runtime.textContent = this.runtimeLabel(c, chosen, isDefault);
    runtime.title = this.runtimeTooltip(c);
    $("agent-connection").textContent = ready ? "· connected" : "· not configured";
    $("agent-connection").title = ready ? `${chosen} is installed and selected` : "Open Settings to choose an installed agent";
    const select = $("agent-command");
    this.replaceChoices(select, this.commandChoices(installed));
    select.value = chosen;
    select.title = installed.some((a: any) => !a.headless)
      ? "Agents without a headless mode cannot answer from this window yet"
      : "The system agent this app runs";
    this.fillDraft(c);
  }

  async saveConfig() {
    const patch = {
      agent: $("agent-command").value,
      provider: $("agent-provider").value.trim(),
      model: $("agent-model").value.trim(),
      thinking: $("agent-thinking").value,
      extra_args: $("agent-extra").value.trim().split(/\s+/).filter(Boolean),
    };
    try {
      this.config = await invoke("agent_config_save", { patch });
      this.renderConfig();
      $("anim-agent-settings").hidden = true;
      $("agent-config-toggle").setAttribute("aria-expanded", "false");
      this.newChat = true;
      this.status("Chat selection saved · next message starts fresh");
      await this.refreshModels();
    } catch (error) { this.status(String(error)); }
  }

  formatCatalogStatus(reply: any) {
    if (!reply.available) return reply.reason || "Catalog unavailable; no model substituted";
    const enabled = this.models.filter((m: any) => m.enabled);
    const local = enabled.filter((m: any) => m.local).length;
    return `${enabled.length} available models${local ? ` · ${local} local` : ""} · selection is checked before sending`;
  }

  async refreshModels() {
    try {
      const reply = await motionCommand("agent.models", { agent: $("agent-command").value });
      this.models = Array.isArray(reply.models) ? reply.models : [];
      const providers = [...new Set(this.models.filter((m: any) => m.enabled).map((m: any) => m.providerID))];
      $("agent-providers").replaceChildren(...providers.map((p) => el("option", { value: p })));
      this.renderModelOptions();
      $("agent-catalog-status").textContent = this.formatCatalogStatus(reply);
    } catch (error) {
      this.models = [];
      $("agent-providers").replaceChildren();
      $("agent-models").replaceChildren();
      $("agent-catalog-status").textContent = String(error);
    }
  }

  renderModelOptions() {
    const provider = $("agent-provider").value.trim();
    const options = this.models.filter((m: any) => m.enabled && m.providerID === provider)
      .slice().sort((a: any, b: any) => (b.local ? 1 : 0) - (a.local ? 1 : 0) || String(a.id).localeCompare(String(b.id)));
    $("agent-models").replaceChildren(...options.map((m: any) => el("option", { value: m.id, title: m.local ? "Local model — runs on this machine" : "" },
      `${m.name || m.id}${m.local ? " · local" : ""}`)));
  }

  async load() {
    this.loaded = true;
    const [config, turns] = await Promise.allSettled([
      invoke("agent_config"),
      state.project ? motionCommand("agent.log") : Promise.resolve([]),
    ]);
    if (config.status === "fulfilled") this.config = config.value;
    else this.status(String(config.reason));
    this.turns = turns.status === "fulfilled" && Array.isArray(turns.value) ? turns.value : [];
    this.renderConfig();
    this.renderLog();
  }

  async reloadLog() {
    if (!state.project) return;
    try {
      const turns = await motionCommand("agent.log", { refresh: true });
      if (Array.isArray(turns)) this.turns = turns;
    } catch { /* the next send reports the real error */ }
    this.renderLog();
    await this.acknowledgeReply();
  }

  latestProjectReply(project: string | null) {
    return [...this.turns].reverse().find((t: any) => t.role === "assistant" && t.request_id && t.project === project);
  }

  isReplyVisible(text: string) {
    return [...$("agent-log").querySelectorAll(".msg.assistant")].some((n: any) => {
      const rect = n.getBoundingClientRect();
      return n.textContent === text && rect.width > 0 && rect.height > 0;
    });
  }

  async acknowledgeReply(revision?: number) {
    const project = state.project;
    const turn = this.latestProjectReply(project);
    if (!this.open || !turn) return;
    await new Promise(requestAnimationFrame);
    if (!this.open || state.project !== project || !this.isReplyVisible(turn.text)) return;
    if (revision) await acFeedback.acknowledge(revision, project);
    const key = `${project}:${turn.request_id}:${turn.session}`;
    if (this.renderedReply === key) return;
    await acFeedback.report(null, null, {
      stage: "frontend_ack", command: "agent.send", request_id: turn.request_id,
      session: turn.session, reply_rendered: true, ui_ack: "assistant reply displayed; visual review separate",
    });
    this.renderedReply = key;
  }

  turnRoleClass(role: string) {
    if (role === "user") return "msg user";
    if (role === "assistant") return "msg assistant";
    if (role === "error") return "msg tool";
    return null;
  }

  turnItems() {
    const items: any[] = [];
    if (!this.turns.length) {
      items.push({ key: "notice:empty", cls: "msg done",
        text: "Ask the agent anything about this project. Point at something to show it what you mean." });
    }
    this.turns.forEach((turn: any, i: number) => {
      const key = `t:${i}:${turn.at || 0}`;
      const cls = this.turnRoleClass(turn.role);
      if (cls) {
        const text = turn.role === "error" ? `· ${turn.text || "failed"}` : (turn.text || "");
        items.push({ key, cls, text });
      }
      if (turn.role === "assistant" && turn.tools?.length) {
        items.push({ key: `${key}:tools`, cls: "msg tool", text: `· ${turn.tools.join(", ")}` });
      }
    });
    return items;
  }

  findInsertAnchor(log: any, next: any[], startIndex: number) {
    for (let j = startIndex + 1; j < next.length; j++) {
      if (next[j].parentNode === log) return next[j];
    }
    return null;
  }

  reconcileLogNodes(log: any, existing: Map<string, any>, items: any[]) {
    return items.map((item) => {
      const node = existing.get(item.key);
      if (!node) {
        const made = el("div", { class: item.cls }, item.text);
        made.dataset.k = item.key;
        return made;
      }
      existing.delete(item.key);
      if (node.textContent !== item.text) node.textContent = item.text;
      return node;
    });
  }

  renderLog() {
    const log = $("agent-log");
    const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
    const top = log.scrollTop;
    const existing = new Map<string, any>();
    for (const child of log.children) if (child.dataset?.k) existing.set(child.dataset.k, child);
    const next = this.reconcileLogNodes(log, existing, this.turnItems());
    for (const gone of existing.values()) gone.remove();
    for (let i = 0; i < next.length; i++) {
      const node = next[i];
      if (node.parentNode !== log) log.insertBefore(node, this.findInsertAnchor(log, next, i));
    }
    log.scrollTop = atBottom ? log.scrollHeight : top;
  }

  renderAttachments() {
    $("agent-attachments").replaceChildren(...this.attachments.map((a: any, i: number) =>
      el("div", { class: "att", title: a.path },
        el("img", { src: convertFileSrc(a.path), alt: "" }), el("span", {}, a.what),
        el("button", { "aria-label": "Remove", title: "Remove", onclick: () => { this.attachments.splice(i, 1); this.renderAttachments(); } }, "×"))));
  }

  clearSent(input: any, raw: string, attachments: any[]) {
    if (input.value === raw) input.value = "";
    const sent = new Set(attachments);
    if (this.attachments.some((a) => sent.has(a))) {
      this.attachments = this.attachments.filter((a) => !sent.has(a));
      this.renderAttachments();
    }
  }

  async startNewChat() {
    if (this.sending) throw new Error("Wait for the current message to finish");
    const button = $("agent-new"), project = state.project;
    button.disabled = true;
    try {
      const reply = await motionCommand("agent.new_chat");
      if (state.project !== project) return;
      this.turns = Array.isArray(reply.turns) ? reply.turns : [];
      this.newChat = !reply.session;
      this.renderLog();
      this.status(`New chat ready${reply.session ? ` · ${reply.session}` : ""} · previous conversation saved`);
      $("agent-input").focus();
      if (reply._ui_engine_revision) await acFeedback.acknowledge(reply._ui_engine_revision, project);
    } finally { button.disabled = false; }
  }

  formatReplyStatus(reply: any) {
    if (!reply.ok) return String(reply.error || "Agent needs attention in its existing session; do not resend blindly.");
    const prov = reply.provider ? ` · ${reply.provider}` : "";
    const mod = reply.model ? ` / ${reply.model}` : " · CLI default unverified";
    return `Answered${prov}${mod} · ${reply.session || "fresh process"}`;
  }

  async send() {
    const input = $("agent-input");
    if (this.sending) return;
    const raw = input.value;
    const text = raw.trim();
    if (!text && !this.attachments.length) return;
    const attachments = this.attachments.slice();
    this.sending = true;
    $("agent-send").disabled = true;
    this.status("Sending…");
    try {
      const context = {
        project: state.project, view: state.view, storyboard_page: state.page + 1,
        deck: state.data?.storyboard?.deck,
        animation: { asset: motion.character, track: motion.track, time: motion.time },
      };
      const message = `${text}\n\n[Magic Studio context at send time: ${JSON.stringify(context)}]`;
      const reply = await motionCommand("agent.send", {
        message, new_chat: this.newChat, attachments: attachments.map((a) => a.path),
      });
      this.newChat = true;
      if (Array.isArray(reply.turns)) this.turns = reply.turns;
      this.renderLog();
      await this.acknowledgeReply(reply._ui_engine_revision);
      this.clearSent(input, raw, attachments);
      this.status(this.formatReplyStatus(reply));
    } catch (error) {
      this.status(String(error));
      input.focus();
    } finally {
      this.sending = false;
      $("agent-send").disabled = false;
    }
  }

  describe(node: any) {
    const label = node.getAttribute("aria-label") || node.title || node.closest("[aria-label]")?.getAttribute("aria-label");
    const text = (node.innerText || "").trim().split("\n")[0].slice(0, 60);
    const where = node.closest("section, .block, header, aside")?.getAttribute("aria-label")
      || node.closest(".block")?.querySelector("h2, .title")?.textContent;
    return [label || text || node.tagName.toLowerCase(), where && where !== label ? `in ${where.trim()}` : ""].filter(Boolean).join(" ");
  }

  pick(x: number, y: number) {
    const overlay = $("point-overlay");
    overlay.classList.add("picking");
    const node = document.elementFromPoint(x, y);
    overlay.classList.remove("picking");
    return node && !node.closest("#agent") ? node : null;
  }

  highlightPointBox(box: any, target: any) {
    if (!target) {
      box.style.setProperty("--point-width", "0px");
      return;
    }
    const r = target.getBoundingClientRect();
    const bounds: Record<string, number> = { left: r.left - 3, top: r.top - 3, width: r.width + 6, height: r.height + 6 };
    for (const [key, value] of Object.entries(bounds)) box.style.setProperty(`--point-${key}`, `${value}px`);
  }

  async captureNodeSnapshot(node: any, previousDrawer: string | null) {
    const r = node.getBoundingClientRect(), pad = 12;
    const rect = [
      Math.max(0, r.left - pad),
      Math.max(0, r.top - pad),
      Math.min(innerWidth, r.right + pad) - Math.max(0, r.left - pad),
      Math.min(innerHeight, r.bottom + pad) - Math.max(0, r.top - pad),
    ];
    this.status("Capturing…");
    await new Promise((ok) => setTimeout(ok, 380));
    try {
      const path = await invoke("snapshot", { project: state.project, rect });
      this.attachments.push({ path, what: this.describe(node) });
      this.renderAttachments();
      this.status("Picture attached.");
      $("agent-input").focus();
    } catch (err) {
      this.status(String(err));
    } finally {
      if (previousDrawer) workshopDrawers.open(previousDrawer);
    }
  }

  pointStart() {
    const overlay = $("point-overlay"), box = $("point-box");
    overlay.classList.add("on");
    const previousDrawer = workshopDrawers.active;
    workshopDrawers.close(false);
    let target: any = null;
    const move = (e: MouseEvent) => {
      target = this.pick(e.clientX, e.clientY);
      this.highlightPointBox(box, target);
    };
    const stop = (resume = true) => {
      overlay.classList.remove("on");
      overlay.removeEventListener("mousemove", move);
      overlay.removeEventListener("click", click);
      document.removeEventListener("keydown", esc, true);
      box.style.setProperty("--point-width", "0px");
      if (resume && previousDrawer) workshopDrawers.open(previousDrawer);
    };
    const click = async (e: MouseEvent) => {
      e.preventDefault();
      const node = target || this.pick(e.clientX, e.clientY);
      stop(false);
      if (!node) {
        if (previousDrawer) workshopDrawers.open(previousDrawer);
        return;
      }
      await this.captureNodeSnapshot(node, previousDrawer);
    };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); stop(); } };
    overlay.addEventListener("mousemove", move);
    overlay.addEventListener("click", click);
    document.addEventListener("keydown", esc, true);
  }

  toggle(open = !this.open) {
    if (open) workshopDrawers.open("sproket");
    else if (workshopDrawers.active === "sproket") workshopDrawers.close();
  }

  async attachFiles(files: File[]) {
    for (const file of files) {
      if (!file.type.startsWith("image/")) {
        this.status(`Only pictures attach (${file.name || file.type} skipped).`);
        continue;
      }
      try {
        const name = file.name && file.name !== "image.png" ? file.name : `image.${file.type.split("/")[1] || "png"}`;
        const path = await invoke("save_attachment", new Uint8Array(await file.arrayBuffer()),
          { headers: { "x-project": state.project, "x-name": encodeURIComponent(name) } });
        this.attachments.push({ path, what: file.name && file.name !== "image.png" ? file.name : "a pasted picture" });
        this.renderAttachments();
        this.status("Picture attached.");
      } catch (err) { this.status(String(err)); }
    }
  }
}

const agent = new AgentDrawerController();

const agentStatus = (text: string) => agent.status(text);
const installedAgents = (c: any) => agent.installedAgents(c);
function runtimeLabel(c: any, chosen: string, isDefault: boolean) { return agent.runtimeLabel(c, chosen, isDefault); }
function runtimeTooltip(c: any) { return agent.runtimeTooltip(c); }
function commandChoices(installed: any[]) { return agent.commandChoices(installed); }
function replaceChoices(select: any, options: any[]) { agent.replaceChoices(select, options); }
function fillAgentDraft(c: any) { agent.fillDraft(c); }
function renderConfig() { agent.renderConfig(); }
async function configSave() { return agent.saveConfig(); }
async function refreshModels() { return agent.refreshModels(); }
function renderModelOptions() { agent.renderModelOptions(); }
async function agentLoad() { return agent.load(); }
async function agentReloadLog() { return agent.reloadLog(); }
async function acknowledgeAgentReply(revision?: number) { return agent.acknowledgeReply(revision); }
function turnItems() { return agent.turnItems(); }
function renderLog() { agent.renderLog(); }
function renderAttachments() { agent.renderAttachments(); }
function clearSent(input: any, raw: string, attachments: any[]) { agent.clearSent(input, raw, attachments); }
async function agentNewChat() { return agent.startNewChat(); }
$("agent-new").addEventListener("click", () => agentNewChat().catch((e) => agentStatus(String(e))));
async function agentSend() { return agent.send(); }
function describe(node: any) { return agent.describe(node); }
function pick(x: number, y: number) { return agent.pick(x, y); }
function pointStart() { agent.pointStart(); }

const workshopDrawers = createTopDrawers({
  canOpen: () => { if (state.project) return true; toast("Open a project first"); return false; },
  onLayout: (height: number) => invoke("drawer_layout", { height }).catch((e: any) => agentStatus(String(e))),
  onSettings: (id: string) => { if (id === "anim-agent-settings") refreshModels(); },
  onChange: (name: string) => {
    agent.open = name === "sproket";
    $("agent").setAttribute("aria-hidden", String(!agent.open));
    window.magicCompanions?.sync();
    clearInterval(agent.timer);
    if (agent.open) {
      if (agent.loaded) agentReloadLog(); else agentLoad();
      agent.timer = setInterval(() => { if (!agent.sending) agentReloadLog(); }, 1500);
    }
    if (name === "sproket" || name === "gaffer") $(name === "sproket" ? "agent-input" : "image-prompt")?.focus();
  },
});
window.workshopDrawers = workshopDrawers;
function agentToggle(open = !agent.open) { agent.toggle(open); }
async function attachFiles(files: File[]) { return agent.attachFiles(files); }

$("agent-input").addEventListener("paste", (e: ClipboardEvent) => {
  const files = [...(e.clipboardData?.files || [])];
  if (files.length) { e.preventDefault(); attachFiles(files); }
});
$("agent").addEventListener("dragover", (e: DragEvent) => { e.preventDefault(); $("agent").classList.add("drop"); });
$("agent").addEventListener("dragleave", (e: DragEvent) => { if (!$("agent").contains(e.relatedTarget as Node)) $("agent").classList.remove("drop"); });
$("agent").addEventListener("drop", (e: DragEvent) => {
  e.preventDefault();
  $("agent").classList.remove("drop");
  attachFiles([...(e.dataTransfer?.files || [])]);
});
$("agent-send").addEventListener("click", agentSend);
$("agent-point").addEventListener("click", pointStart);
$("agent-catalog-refresh").addEventListener("click", refreshModels);
$("agent-command").addEventListener("change", refreshModels);
$("agent-provider").addEventListener("input", renderModelOptions);
$("agent-config-save").addEventListener("click", configSave);
$("agent-input").addEventListener("keydown", (e: KeyboardEvent) => {
  // An IME candidate is not a send: Enter only picks the candidate.
  if (e.isComposing || e.key === "Process") return;
  if (e.key !== "Enter") return;
  if (e.shiftKey) return; // Shift+Enter is a line break.
  e.preventDefault();
  agentSend();
});
document.addEventListener("keydown", (e: KeyboardEvent) => {
  if ((e.key === "/" || e.key === "\\") && (e.ctrlKey || e.metaKey)) { e.preventDefault(); agentToggle(); return; }
});
