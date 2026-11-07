interface DraftSnapshot {dirty: boolean; drafts: string[]; saving: boolean}

export function workspaceDrafts(root: Document = document): DraftSnapshot {
  const drafts: string[] = [];
  let saving = !!root.querySelector('[data-document-saving="true"]');
  for (const panel of root.querySelectorAll<HTMLElement>('[data-document-dirty="true"]')) drafts.push(panel.dataset.documentLabel || "Document fields");
  for (const frame of root.querySelectorAll<HTMLIFrameElement>('iframe[id^="restored-"]')) {
    try {
      const editor = frame.contentWindow as unknown as {magicDraftSnapshot?: () => DraftSnapshot};
      const state = editor?.magicDraftSnapshot?.();
      if (!state || typeof state.dirty !== "boolean" || typeof state.saving !== "boolean" || !Array.isArray(state.drafts) || !state.drafts.every(value => typeof value === "string")) drafts.push(`${frame.title}: draft state unavailable`);
      else {
        if (state.dirty || state.drafts.length) drafts.push(...(state.drafts.length ? state.drafts : [frame.title]));
        saving ||= !!state.saving;
      }
    } catch { drafts.push(`${frame.title}: draft state unavailable`); }
  }
  return {dirty:drafts.length > 0,drafts,saving};
}
