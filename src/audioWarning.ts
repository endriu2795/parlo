// Older captures only stored aggregate QPC gaps, not maximum sample loss.
// Keep the original available quietly instead of pretending it proves no loss.
export function quietLegacyAudioDiagnostic(error: string | null | undefined): boolean {
  if (!error?.startsWith("Discontinuità audio: ")) return false;
  const sources = error.slice("Discontinuità audio: ".length).split(/(?=Microfono:|Audio PC:)/).filter(s => s.trim());
  return sources.length > 0 && sources.every(source => {
    const match = source.match(/^(?:Microfono|Audio PC): \d+ segnalazioni di discontinuità del dispositivo\. (\d+) intervalli senza pacchetti, (\d+\.\d+) s totali\. Verifica in prossimità di [\d:, ealtripunt]+\. La registrazione è proseguita\.\s*$/);
    if (!match) return false;
    const count = Number(match[1]), seconds = Number(match[2]);
    return count > 0 && seconds < 1 && seconds / count <= 0.025;
  });
}
