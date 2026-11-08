interface DraftSnapshot {dirty: boolean; drafts: string[]; saving: boolean}

export function workspaceDrafts(root: Document = document): DraftSnapshot {
  const drafts: string[] = [];
  const saving = !!root.querySelector('[data-document-saving="true"]');
  for (const panel of root.querySelectorAll<HTMLElement>('[data-document-dirty="true"]')) {
    drafts.push(panel.dataset.documentLabel || "Document fields");
  }
  return { dirty: drafts.length > 0, drafts, saving };
}
