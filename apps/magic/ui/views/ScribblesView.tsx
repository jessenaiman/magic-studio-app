import React, { useState } from "react";
import { executeCommand } from "../api";

export const SCRIBBLES_FLOW_URL =
  "https://flow.google.com/project/3102c6e1-22d8-49b0-abdf-6496f5819190/tool/2129f8ab-a33e-4ab7-a5da-c2454e714100?fromViewSource=tools";

interface Props {
  onToast: (msg: string) => void;
}

export const ScribblesView: React.FC<Props> = ({ onToast }) => {
  const [reloadKey, setReloadKey] = useState(0);

  const openNativeWindow = async () => {
    try {
      await executeCommand("scribbles.open", { url: SCRIBBLES_FLOW_URL, ingest: false });
      onToast("Opened Google Flow tool window");
    } catch (error) {
      onToast(`Could not open Google Flow window: ${error}`);
    }
  };

  return (
    <div id="view-scribbles" className="scribbles-view">
      <div id="scribbles-bar" className="scribbles-bar">
        <span className="scribbles-title">Scribbles</span>
        <span className="scribbles-url">flow.google.com · 2129f8ab</span>
        <button
          id="scribbles-reload"
          className="scribbles-btn"
          onClick={() => setReloadKey((k) => k + 1)}
        >
          Reload
        </button>
        <button
          id="scribbles-open-window"
          className="scribbles-btn"
          onClick={() => void openNativeWindow()}
        >
          Open in Window
        </button>
      </div>
      <iframe
        key={reloadKey}
        id="scribbles-frame"
        className="scribbles-frame"
        src={SCRIBBLES_FLOW_URL}
        title="Scribbles — Google Flow"
        allow="clipboard-read; clipboard-write"
      />
    </div>
  );
};
