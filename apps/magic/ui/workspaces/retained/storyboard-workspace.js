const { invoke: boardInvoke, convertFileSrc: boardFileUrl } = window.__TAURI__.core;
const boardElement = id => document.getElementById(id);
let board = null;
let boardBusy = false;

function boardCommand(action, params = {}) {
  return boardInvoke("execute_command", {
    id: "ui.storyboard.workspace",
    params: { action, project: board?.project, ...params },
  });
}

function boardDraftKey() {
  return `magic.storyboard:${JSON.stringify([board.project, board.page, board.revision])}`;
}

function updateBoardDraft() {
  const data = Object.fromEntries(new FormData(boardElement("sb-fields-form")));
  sessionStorage.setItem(boardDraftKey(), JSON.stringify(data));
  return data;
}

function pageTitle(page) {
  return page.title || ({ cover: "Cover", world: "World", char: "Character Study" }[page.archetype]) ||
    (page.archetype === "scene" ? page.scene_id || "Scene" : `Page ${page.page + 1}`);
}

function renderBoardFields(page) {
  const fields = boardElement("sb-fields-list");
  const draft = JSON.parse(sessionStorage.getItem(boardDraftKey()) || "{}");
  const unique = new Map((board.slots || []).filter(slot => ["text", "image"].includes(slot.kind)).map(slot => [slot.label, slot]));
  const groups = ["text", "image"].map(kind => {
    const rows = [...unique.values()].filter(slot => slot.kind === kind).map(slot => {
      const name = `${kind}:${slot.label}`;
      const control = kind === "text" ? el("textarea", { name, rows: "2" }) : el("input", { name, type: "text" });
      control.value = draft[name] ?? (kind === "text" ? slot.text : slot.image) ?? "";
      const label = el("label", {}, el("code", {}, slot.label), control);
      if (kind === "text") return label;
      return el("div", { class: "panel-fields" }, label,
        el("button", { type: "button", class: "ghost", onclick: () => pickBoardImage(control) }, "Browse…"));
    });
    return rows.length ? el("details", { class: "motion-section", open: kind === "text" },
      el("summary", {}, kind === "text" ? "Text Fields" : "Image Fields"), el("div", { class: "motion-section-body" }, rows)) : null;
  });
  fields.replaceChildren(...groups.filter(Boolean));
}

function setBoardInspector(open) {
  boardElement("sb-fields").hidden = !open;
  boardElement("sb-edit").setAttribute("aria-expanded", String(open));
}

function renderBoard() {
  const page = board.pages[board.page];
  const hasPages = board.pages.length > 0;
  boardElement("sb-new").hidden = hasPages;
  boardElement("sb-add").hidden = !hasPages;
  boardElement("sb-page").textContent = page ? `${board.page + 1} / ${board.pages.length}` : "Storyboard";
  boardElement("sb-prev").disabled = !page || board.page === 0;
  boardElement("sb-next").disabled = !page || board.page + 1 >= board.pages.length;
  for (const id of ["sb-open", "sb-export", "sb-edit"]) boardElement(id).disabled = !page;
  boardElement("sb-pages").replaceChildren(...board.pages.map(item => el("button", {
    type: "button", id: `sb-nav-${item.page + 1}`, class: "ghost",
    "aria-current": item.page === board.page ? "page" : null,
    onclick: () => boardAction("select", { page: item.page }),
  }, pageTitle(item))));
  const cue = boardElement("sb-cue");
  cue.hidden = !page?.cue;
  cue.replaceChildren(...(page?.cue ? [el("strong", {}, "Cue"), el("p", {}, page.cue)] : []));
  const preview = board.preview ? el("img", {
    src: `${boardFileUrl(board.preview)}&v=${encodeURIComponent(board.preview_revision || "")}`,
    alt: page ? pageTitle(page) : "Storyboard spread",
  }) : el("p", { class: "panel-empty" }, hasPages ? "Refresh to render the storyboard spread." : "Create a storyboard from the neutral template.");
  boardElement("sb-cards").replaceChildren(preview);
  boardElement("sb-title-wrap").scrollTop = 0;
  if (!page) setBoardInspector(false);
  renderBoardFields(page);
}

async function boardAction(action, params = {}, message = "") {
  if (boardBusy) return;
  boardBusy = true;
  const status = boardElement("sb-status");
  status.textContent = action === "export" ? "Exporting storyboard…" : "";
  try {
    const next = await boardCommand(action, params);
    if (action === "save") sessionStorage.removeItem(boardDraftKey());
    board = next;
    if (!["open", "scribbles"].includes(action)) renderBoard();
    status.textContent = message;
  } catch (error) {
    status.textContent = String(error.message || error);
  } finally {
    boardBusy = false;
  }
}

async function pickBoardImage(control) {
  if (boardBusy) return;
  boardBusy = true;
  try {
    const file = await boardInvoke("execute_command", { id: "ui.storyboard.pick_image", params: {} });
    if (file?.path) {
      control.value = file.path;
      updateBoardDraft();
    }
  } catch (error) {
    boardElement("sb-status").textContent = String(error.message || error);
  } finally {
    boardBusy = false;
  }
}

function saveBoardFields(event) {
  event.preventDefault();
  const changed = updateBoardDraft();
  const fields = {}, images = {};
  for (const slot of board.slots || []) {
    const value = changed[`${slot.kind}:${slot.label}`];
    if (value === undefined) continue;
    if (slot.kind === "text" && value !== (slot.text || "")) fields[slot.label] = value;
    if (slot.kind === "image" && value.trim() && value.trim() !== slot.image) images[slot.label] = value.trim();
  }
  if (Object.keys(fields).length || Object.keys(images).length)
    boardAction("save", { page: board.page, revision: board.revision, fields, images }, "Edits saved.");
}

boardElement("sb-fields-form").addEventListener("input", updateBoardDraft);
boardElement("sb-fields-form").addEventListener("submit", saveBoardFields);
boardElement("sb-edit").onclick = () => setBoardInspector(boardElement("sb-fields").hidden);
boardElement("sb-prev").onclick = () => boardAction("select", { page: board.page - 1 });
boardElement("sb-next").onclick = () => boardAction("select", { page: board.page + 1 });
for (const [id, action, message] of [["sb-new", "new"], ["sb-add", "add"], ["sb-refresh", "refresh"], ["sb-open", "open"], ["sb-export", "export", "Saved storyboard.pdf beside the project."]]) {
  boardElement(id).onclick = () => boardAction(action, {}, message);
}
document.addEventListener("keydown", event => {
  if (event.target.closest("textarea,input,select,[contenteditable=true]")) return;
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    boardElement(event.key === "ArrowLeft" ? "sb-prev" : "sb-next").click();
  }
});
boardAction("state");
