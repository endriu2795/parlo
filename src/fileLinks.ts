export function localFileReference(href: string) {
  let path = href;
  try {
    if (/^file:/i.test(path)) {
      const url = new URL(path);
      if (url.hostname && url.hostname !== "localhost") return null;
      path = url.pathname;
    }
    path = decodeURIComponent(path);
  } catch {
    return null;
  }
  if (/^\/[a-z]:[\\/]/i.test(path)) path = path.slice(1);
  if (/[\0\r\n]/.test(path) || path.startsWith("//") || path.startsWith("\\\\"))
    return null;
  if (!/^[a-z]:[\\/]/i.test(path) && !path.startsWith("/")) return null;
  // Coding agents sometimes append a source line to a file citation.
  path = path.replace(/#L\d+(?:C\d+)?$/, "").replace(/:\d+(?::\d+)?$/, "");
  const name =
    path.replace(/\\/g, "/").split("/").filter(Boolean).at(-1) || path;
  return { path, name };
}
