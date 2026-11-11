import { t } from "./i18n";
import { useEffect, useState } from "react";
import { Camera, X } from "lucide-react";
import { api } from "./api";
import { fmtTime } from "./format";
import type { Screenshot } from "./types";

export default function ScreenshotCard({
  callId,
  image,
  onSeek,
}: {
  callId: string;
  image: Screenshot;
  onSeek?: (time: number) => void;
}) {
  const [preview, setPreview] = useState("");
  const [full, setFull] = useState("");
  const [open, setOpen] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const time = fmtTime(image.timestamp_ms);
  useEffect(() => {
    let alive = true;
    void api
      .screenshotImage(callId, image.id)
      .then((data) => {
        if (alive) setPreview(data);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, [callId, image.id]);
  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [open]);
  const show = async () => {
    setOpen(true);
    if (full || loading) return;
    setLoading(true);
    setError("");
    try {
      setFull(await api.screenshotImage(callId, image.id, false));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };
  return (
    <article className="segment screenshot-segment">
      <button type="button" className="timestamp-link mono" disabled={!onSeek} title={onSeek ? t("Ascolta da questo punto") : t("Audio non conservato")} onClick={()=>onSeek?.(image.timestamp_ms)}>{time}</button>
      <div>
        <strong>
          <Camera size={15} /> Screenshot
        </strong>
        <button
          className="screenshot-thumbnail"
          onClick={() => void show()}
          aria-label={t("Apri screenshot delle {0}", time)}
        >
          {preview ? (
            <img src={preview} alt={t("Schermo catturato a {0}", time)} />
          ) : (
            <span>{error || t("Caricamento immagine…")}</span>
          )}
        </button>
        <small>
          {image.width} × {image.height} {t("· Disponibile all’assistente su richiesta")}
        </small>
      </div>
      {open && (
        <div
          className="screenshot-viewer"
          role="dialog"
          aria-modal="true"
          aria-label={`Screenshot ${time}`}
          onClick={() => setOpen(false)}
        >
          <div
            className="screenshot-viewer-bar"
            onClick={(e) => e.stopPropagation()}
          >
            <strong>Screenshot · {time}</strong>
            <button
              autoFocus
              onClick={() => setOpen(false)}
              aria-label={t("Chiudi immagine")}
            >
              <X size={20} />
            </button>
          </div>
          {full ? (
            <img
              src={full}
              alt={t("Screenshot delle {0}", time)}
              onClick={(e) => e.stopPropagation()}
            />
          ) : (
            <p>{error || t("Caricamento immagine…")}</p>
          )}
        </div>
      )}
    </article>
  );
}
