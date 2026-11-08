import React from "react";
import ReactDOM from "react-dom/client";
import { attachConsole } from "@tauri-apps/plugin-log";
import App from "./App";
import "./global.css";
import "./style.css";
import "./app-bar.css";
import "./drawers.css";
import "./menu.css";
import "./animation-review.css";

// Pipe the webview console into the Rust logger, so JS failures land in the runtime log.
void attachConsole();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
