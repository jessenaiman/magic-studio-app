import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./global.css";
import "./style.css";
import "./app-bar.css";
import "./drawers.css";
import "./menu.css";
import "./animation-review.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
