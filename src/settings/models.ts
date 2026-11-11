import { t } from "../i18n";
import type { Health, Settings } from "../types";

export type WhisperModel = NonNullable<Health["models"]>[number];

const friendly: Record<string, { title: string; description: string }> = {
  turbo: { title: t("Massima precisione"), description: t("Consigliata. Whisper Turbo: più accurata, richiede più risorse.") },
  small: { title: t("Bilanciata"), description: t("Whisper Small: buon compromesso tra precisione e velocità.") },
  tiny: { title: t("Veloce"), description: t("Whisper Tiny: leggera per computer meno potenti, meno precisa.") },
};
const order = ["turbo", "small", "tiny"];

export const modelTitle = (id: string) => friendly[id]?.title || id;
export const modelDescription = (id: string) => friendly[id]?.description || "";

export function availableModels(health: Health | null, draft: Settings): WhisperModel[] {
  const models = health?.models || order.map(id => ({
    id, ready: true, path: draft.whisper_model.replace(/whisper-[^\\/]+$/, `whisper-${id}`),
  }));
  const rank = (id: string) => (order.indexOf(id) + 1 || order.length + 1);
  return [...models].sort((a, b) => rank(a.id) - rank(b.id));
}

function normalizePath(path: string, engineRoot?: string) {
  let normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  if (normalized.startsWith("@engine/") && engineRoot)
    normalized = `${engineRoot.replace(/\\/g, "/").replace(/\/+$/, "")}/${normalized.slice(8)}`;
  return /^[A-Za-z]:\//.test(normalized) ? normalized.toLowerCase() : normalized;
}

/** Model id for a configured path, or "custom" when the path is not a bundled model. */
export function modelIdFor(path: string, models: WhisperModel[], engineRoot?: string) {
  const target = normalizePath(path, engineRoot);
  return models.find(model => normalizePath(model.path, engineRoot) === target)?.id || "custom";
}

/** Live preview needs its own build of the model for the chosen device. */
export const previewReady = (model: WhisperModel, liveMode: string | undefined) =>
  liveMode === "cpu" ? (model.preview_cpu_ready ?? model.ready) : (model.preview_ready ?? model.ready);

export function calibrationLabel(health: Health | null, modelId: string, computeMode: Settings["compute_mode"]) {
  const calibrated = health?.acceleration?.models[modelId];
  if (computeMode !== "cpu") return calibrated?.label;
  return calibrated?.results
    ?.filter(result => result.plan.device.toLowerCase() === "cpu")
    .sort((a, b) => a.inference_seconds - b.inference_seconds)[0]?.label;
}
