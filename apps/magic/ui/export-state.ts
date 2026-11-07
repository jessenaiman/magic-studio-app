export interface ExportReport {
  export_ready?: boolean;
  export?: {path: string; exists: boolean};
  validation?: {valid?: boolean; reason?: string; error?: string};
}

export function exportIconState(report: ExportReport | null, running: boolean, error: string, drafts: boolean) {
  if (running) return "running";
  if (error) return "error";
  if (!drafts && report?.export_ready && report.validation?.valid && report.export?.exists && report.export.path) return "ready";
  return "idle";
}
