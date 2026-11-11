import { t } from "./i18n";
import { useEffect, useState, type CSSProperties } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X, Sun, Moon, Settings } from "lucide-react";

const icon = new URL("./assets/parlo-titlebar-icon.png", import.meta.url).href;
const icon2x = new URL("./assets/parlo-titlebar-icon@2x.png", import.meta.url).href;

export default function AppTitlebar({
  color,
  theme,
  onTheme,
  onSettings,
  onError,
}: {
  color?: string;
  theme?: string;
  onTheme: () => void;
  onSettings: () => void;
  onError: (error: string) => void;
}) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const window = getCurrentWindow();
    let disposed = false;
    const update = async () => {
      try {
        const maximized = await window.isMaximized();
        if (!disposed) setMaximized(maximized);
      } catch {
        /* The window may be closing. */
      }
    };
    void update();
    const stop = window.onResized(() => void update());
    return () => {
      disposed = true;
      void stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);
  const run = (action: () => Promise<void>) => {
    void action().catch((error) => onError(String(error)));
  };
  let style: CSSProperties | undefined;
  if (color && /^#[0-9a-f]{6}$/i.test(color)) {
    const rgb = parseInt(color.slice(1), 16);
    const light =
      (rgb >> 16) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114 > 145000;
    style = { backgroundColor: color, color: light ? "#171c26" : "#edf0f6" };
  }
  return (
    <header
      className="app-titlebar"
      style={style}
      aria-label={t("Finestra PARLO")}
    >
      <div className="app-titlebar-drag" data-tauri-drag-region>
        <img src={icon} srcSet={`${icon2x} 2x`} width="20" height="20" alt="" draggable={false} />
      </div>
      <div className="app-titlebar-actions">
        <button
          type="button"
          aria-label={
            theme === "light" ? t("Passa al tema scuro") : t("Passa al tema chiaro")
          }
          title={theme === "light" ? t("Tema scuro") : t("Tema chiaro")}
          onClick={onTheme}
        >
          {theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
        </button>
        <button
          type="button"
          aria-label={t("Impostazioni")}
          title={t("Impostazioni")}
          onClick={onSettings}
        >
          <Settings size={16} />
        </button>
      </div>
      <div className="app-titlebar-controls">
        <button
          type="button"
          aria-label={t("Riduci a icona")}
          title={t("Riduci a icona")}
          onClick={() => run(() => getCurrentWindow().minimize())}
        >
          <Minus size={15} />
        </button>
        <button
          type="button"
          aria-label={
            maximized ? t("Ripristina finestra") : t("Ingrandisci finestra")
          }
          title={maximized ? t("Ripristina") : t("Ingrandisci")}
          onClick={() => run(() => getCurrentWindow().toggleMaximize())}
        >
          {maximized ? <Copy size={13} /> : <Square size={13} />}
        </button>
        <button
          type="button"
          className="app-titlebar-close"
          aria-label={t("Chiudi finestra")}
          title={t("Chiudi finestra")}
          onClick={() => run(() => getCurrentWindow().close())}
        >
          <X size={16} />
        </button>
      </div>
    </header>
  );
}
