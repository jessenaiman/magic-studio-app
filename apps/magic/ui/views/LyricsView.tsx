import React from "react";
import { RestoredWorkspace } from "../components/RestoredWorkspace";

interface Props { projectPath: string; audioPath?: string; onToast: (message: string) => void }
// Preserve the designer used by the native Lyrics webview before the Tauri migration.
export const LyricsView: React.FC<Props> = ({ projectPath, onToast }) =>
  <RestoredWorkspace key={projectPath} project={projectPath} page="lyrics.html" title="Lyrics designer" onToast={onToast} />;
