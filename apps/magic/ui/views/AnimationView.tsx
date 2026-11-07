import React from "react";
import { RestoredWorkspace } from "../components/RestoredWorkspace";
import { ProjectAsset } from "../api";

interface Props { projectPath: string; assets: ProjectAsset[]; onRefresh: () => Promise<void>; onToast: (message: string) => void }
// Restore the pre-egui workspace, including its inspectors, action panels and timeline.
export const AnimationView: React.FC<Props> = ({ projectPath, onToast }) =>
  <RestoredWorkspace key={projectPath} project={projectPath} page="animation-workspace.html" title="Animation workspace" onToast={onToast} />;
