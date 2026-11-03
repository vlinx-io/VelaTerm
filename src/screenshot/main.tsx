//! Entry of the screenshot overlay window (screenshot.html). Kept separate from the application entry
//! so the overlay loads only the editor, the icon set, and the active dictionary.

import ReactDOM from "react-dom/client";
import { initI18n } from "../i18n";
import { ScreenshotOverlay } from "./ScreenshotApp";
import "./screenshot.css";

document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });

void initI18n().finally(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<ScreenshotOverlay />);
});
