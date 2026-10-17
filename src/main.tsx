import { StrictMode, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { Gallery } from "./gallery/Gallery";
import { installMockBackend } from "./preview/installMockBackend";
import { isBrowserPreview } from "./preview/mode";
import { applyTheme, initialTheme } from "./theme";
import "./theme/base.css";
import "./styles.css";

// Outside the desktop app, serve a sample project so the UI can be reviewed in a browser.
if (isBrowserPreview) installMockBackend();

// Write theme variables before the first render so nothing paints unstyled.
void applyTheme(initialTheme());

const subscribeHash = (onChange: () => void) => {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
};

/** `#gallery` shows the design gallery; anything else shows the editor. */
function Root() {
  const hash = useSyncExternalStore(subscribeHash, () => window.location.hash);
  return hash === "#gallery" ? <Gallery /> : <App />;
}

const root = document.getElementById("root");
if (!root) throw new Error("index.html is missing #root");

createRoot(root).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
