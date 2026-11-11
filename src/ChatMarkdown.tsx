import { t } from "./i18n";
import { useState, type ReactNode } from "react";
import Markdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { FileText, FolderOpen } from "lucide-react";
import { api, desktop } from "./api";
import { localFileReference } from "./fileLinks";

function FileLink({ href, children }: { href: string; children: ReactNode }) {
  const file = localFileReference(href)!;
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <span className="chat-file-citation">
      <button
        type="button"
        className="chat-file-link"
        title={t("Mostra nella cartella: {0}", file.path)}
        aria-label={t("Mostra {0} nella cartella", file.name)}
        disabled={busy || !desktop}
        onClick={async () => {
          setBusy(true);
          setError("");
          try {
            await api.revealChatFile(file.path);
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <FileText size={14} aria-hidden="true" />
        <span>
          {typeof children === "string" && localFileReference(children)
            ? file.name
            : children || file.name}
        </span>
        <FolderOpen size={12} aria-hidden="true" />
      </button>
      {error && (
        <span role="alert" className="chat-file-error">
          {error}
        </span>
      )}
    </span>
  );
}

export default function ChatMarkdown({ children }: { children: string }) {
  return (
    <Markdown
      remarkPlugins={[remarkGfm]}
      urlTransform={(url, key) =>
        key === "href" && localFileReference(url)
          ? url
          : defaultUrlTransform(url)
      }
      components={{
        a: ({ href, children }) =>
          href && localFileReference(href) ? (
            <FileLink href={href}>{children}</FileLink>
          ) : (
            <a href={href}>{children}</a>
          ),
        table: ({ children }) => (
          <div className="markdown-table">
            <table>{children}</table>
          </div>
        ),
      }}
    >
      {children}
    </Markdown>
  );
}
