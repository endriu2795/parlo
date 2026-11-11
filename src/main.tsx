import React, { lazy, Suspense } from "react";
import { createRoot } from "react-dom/client";
import { initializeLocale } from "./i18n";
import "./styles/tokens.css";
import "./styles/controls.css";
import "./styles/app.css";
import "./styles/settings.css";
import "./styles/assistant.css";
import "./styles/recorder.css";
import "./screen.css";
import "./video.css";

const App = lazy(() => import("./App"));
const RecorderOverlay = lazy(() => import("./RecorderOverlay"));
const DrawingOverlay = lazy(() => import("./DrawingOverlay"));
const NoteComposer = lazy(() => import("./NoteComposer"));
const VideoArea = lazy(() => import("./VideoArea"));
const ScreenshotArea = lazy(() => import("./ScreenshotArea"));

// Every Tauri window loads this entry; the query string selects its view.
const params = new URLSearchParams(location.search);
const View = params.has("screenshot-area") ? ScreenshotArea
  : params.has("video-area") ? VideoArea
  : params.has("note") ? NoteComposer
  : params.has("drawing") ? DrawingOverlay
  : params.has("overlay") ? RecorderOverlay
  : App;

void initializeLocale().then(() => {
  createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <Suspense fallback={null}>
        <View />
      </Suspense>
    </React.StrictMode>,
  );
});
