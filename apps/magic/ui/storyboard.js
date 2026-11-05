function showPage(index) {
  const board = state.data?.storyboard;
  const pages = board?.previews || [];
  state.page = Math.max(0, Math.min(index, pages.length - 1));
  const title = (i) => board?.page_titles?.[i] || `Page ${i + 1}`;
  const open = () => openPath(board.deck);
  const rail = $("storyboard-rail"), cards = $("sb-cards");
  rail.replaceChildren();
  cards.replaceChildren();
  $("sb-page").textContent = pages.length ? `${state.page + 1} / ${pages.length}` : "Storyboard";
  $("sb-prev").disabled = !state.page;
  $("sb-next").disabled = state.page + 1 >= pages.length;
  $("sb-open-deck").disabled = !board?.deck;
  if (board?.deck)
    rail.append(el("button", { id: "storyboard-edit-deck", onclick: open }, "Edit in DeckCraft"));
  if (!pages.length) {
    cards.append(el("p", { class: "panel-empty" }, board?.error || (board?.deck ? "Open the deck to view its pages." : "No saved storyboard for this project.")));
    return;
  }
  const nav = el("nav", { "aria-label": "Storyboard contents" }, ...pages.map((_, i) => el("button", {
    id: `sb-nav-${i + 1}`,
    box: "storyboard-page-nav",
    "aria-current": state.page === i ? "page" : null,
    onclick: () => showPage(i)
  }, title(i))));
  rail.append(nav);
  const cue = board.page_cues?.[state.page];
  if (cue?.length)
    rail.append(el("section", { "aria-label": "Selected scene lyrics" }, el("strong", {}, "Lyrics"), ...cue.map((line) => el("p", {}, line.text))));
  state.pageTitles = Object.fromEntries(pages.map((_, i) => [i, title(i)]));
  const [width, height] = board.page_size || [1080, 1600];
  const sheet = el("div", { id: `sb-sheet-${state.page}`, box: "storyboard-page-sheet" });
  sheet.style.setProperty("--ar", `${width}/${height}`);
  sheet.append(el("img", { src: convertFileSrc(pages[state.page]), alt: title(state.page) }));
  for (const [i, link] of (board.layout?.links || []).entries()) {
    if (link.page !== state.page + 1 || !pages[link.to_page - 1])
      continue;
    const [x, y, r, b] = link.rect;
    const button = el("button", {
      id: `sb-link-${state.page}-${i}`,
      class: "sb-link",
      title: title(link.to_page - 1),
      "aria-label": "Go to " + title(link.to_page - 1),
      onclick: () => showPage(link.to_page - 1)
    });
    for (const [key, value] of Object.entries({ x: x / width, y: y / height, w: (r - x) / width, h: (b - y) / height }))
      button.style.setProperty("--" + key, `${100 * value}%`);
    sheet.append(button);
  }
  cards.classList.add("document-pages");
  cards.append(sheet);
  $("sb-title-wrap").scrollTop = 0;
  updateAppFooter();
}
function renderStoryboard() {
  showPage(state.page || 0);
}
$("sb-prev").onclick = () => showPage(state.page - 1);
$("sb-next").onclick = () => showPage(state.page + 1);
$("sb-open-deck").onclick = () => state.data?.storyboard?.deck && openPath(state.data.storyboard.deck);
window.addEventListener("keydown", (event) => {
  if (state.view !== "storyboard" || event.target?.closest("textarea,input,select,[contenteditable=true]"))
    return;
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    showPage(state.page + (event.key === "ArrowRight" ? 1 : -1));
  }
});
