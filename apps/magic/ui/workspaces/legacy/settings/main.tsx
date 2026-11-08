import { createRoot } from "react-dom/client";
import { SettingsPanel } from "./SettingsPanel";

const host = document.getElementById("settings-root");
if (host) createRoot(host).render(<SettingsPanel />);
