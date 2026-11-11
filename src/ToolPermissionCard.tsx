import { t } from "./i18n";
import { useState } from "react";
import { Check, FileText, MessageCircle, ShieldCheck } from "lucide-react";
import { toolLabel } from "./ChatActivity";
import type { ToolPermission } from "./types";

type Question = {
  id?: string;
  header?: string;
  question: string;
  multiSelect?: boolean;
  options?: { label: string; description?: string }[];
};

function QuestionField({
  question: q,
  index,
  count,
  disabled,
  onChange,
}: {
  question: Question;
  index: number;
  count: number;
  disabled: boolean;
  onChange: (answer: string) => void;
}) {
  const [selected, setSelected] = useState<string[]>([]);
  const [custom, setCustom] = useState(!q.options?.length);
  const [text, setText] = useState("");
  const name = `question-${index}`;
  const update = (choices: string[], other: boolean, value: string) => {
    setSelected(choices);
    setCustom(other);
    setText(value);
    onChange(
      [...choices, ...(other && value.trim() ? [value.trim()] : [])].join(", "),
    );
  };
  return (
    <fieldset className="tool-question" disabled={disabled}>
      <legend>
        <span className="tool-question-count">
          {t("Domanda {0} di {1}", index + 1, count)}
          {q.header ? ` · ${q.header}` : ""}
        </span>
        <span>{q.question}</span>
      </legend>
      {q.multiSelect && (
        <p className="tool-question-hint">{t("Puoi scegliere più opzioni.")}</p>
      )}
      <div className="tool-question-options">
        {q.options?.map((option) => (
          <label className="tool-question-option" key={option.label}>
            <input
              type={q.multiSelect ? "checkbox" : "radio"}
              name={name}
              checked={selected.includes(option.label)}
              onChange={() =>
                update(
                  q.multiSelect
                    ? selected.includes(option.label)
                      ? selected.filter((item) => item !== option.label)
                      : [...selected, option.label]
                    : [option.label],
                  q.multiSelect ? custom : false,
                  text,
                )
              }
            />
            <span
              className={`tool-option-mark${q.multiSelect ? " multiple" : ""}`}
              aria-hidden="true"
            >
              <Check size={12} />
            </span>
            <span className="tool-option-copy">
              <strong>{option.label}</strong>
              {option.description && <span>{option.description}</span>}
            </span>
          </label>
        ))}
        {!!q.options?.length && (
          <label className="tool-question-option custom">
            <input
              type={q.multiSelect ? "checkbox" : "radio"}
              name={name}
              checked={custom}
              onChange={() =>
                update(
                  q.multiSelect ? selected : [],
                  !custom || !q.multiSelect,
                  text,
                )
              }
            />
            <span
              className={`tool-option-mark${q.multiSelect ? " multiple" : ""}`}
              aria-hidden="true"
            >
              <Check size={12} />
            </span>
            <span className="tool-option-copy">
              <strong>{t("Scrivi una risposta diversa")}</strong>
            </span>
          </label>
        )}
      </div>
      {custom && (
        <textarea
          className="tool-question-answer"
          rows={3}
          aria-label={t("La tua risposta: {0}", q.question)}
          placeholder={t("Scrivi qui la tua risposta…")}
          value={text}
          onChange={(event) => update(selected, true, event.target.value)}
        />
      )}
    </fieldset>
  );
}

const actions: Record<string, string> = {
  Read: t("Leggere un file"),
  Write: t("Creare un file"),
  Edit: t("Modificare un file"),
  Bash: t("Eseguire un comando"),
  exec_command: t("Eseguire un comando"),
  apply_patch: t("Modificare i file"),
  Glob: t("Cercare file"),
  Grep: t("Cercare nei file"),
  WebSearch: t("Cercare sul web"),
  WebFetch: t("Aprire una pagina web"),
};

export default function ToolPermissionCard({
  permission,
  busy,
  answers,
  onAnswer,
  onDecide,
}: {
  permission: ToolPermission;
  busy: boolean;
  answers: Record<string, string>;
  onAnswer: (key: string, answer: string) => void;
  onDecide: (allow: boolean, always?: boolean) => void;
}) {
  const isQuestion = ["AskUserQuestion", "request_user_input"].includes(
    permission.tool,
  );
  const questions = (
    Array.isArray(permission.input.questions) ? permission.input.questions : []
  ) as Question[];
  const complete =
    questions.length > 0 &&
    questions.every((q) => answers[q.id || q.question]?.trim());
  const file = permission.input.file_path || permission.input.path;
  const command = permission.input.command;
  const title =
    actions[permission.tool] ||
    (permission.title !== "Uso di uno strumento"
      ? permission.title
      : toolLabel(permission.tool));
  const canAlways = !isQuestion && permission.can_always !== false;
  return (
    <section
      className={`tool-permission${isQuestion ? " tool-permission-questions" : ""}`}
      aria-label={
        isQuestion ? t("Domande dell’assistente") : t("Autorizzazione richiesta")
      }
      aria-busy={busy}
    >
      <header className="tool-permission-heading">
        <span className="tool-permission-icon">
          {isQuestion ? <MessageCircle size={17} /> : <ShieldCheck size={17} />}
        </span>
        <div>
          <strong>{isQuestion ? t("Prima di procedere") : title}</strong>
          <p>
            {isQuestion
              ? t("Scegli le risposte e inviale all’assistente.")
              : t("L’assistente chiede la tua autorizzazione.")}
          </p>
        </div>
      </header>
      <div className="tool-permission-body">
        {isQuestion ? (
          questions.map((q, index) => (
            <QuestionField
              key={`${permission.id}:${index}`}
              question={q}
              index={index}
              count={questions.length}
              disabled={busy}
              onChange={(answer) => onAnswer(q.id || q.question, answer)}
            />
          ))
        ) : (
          <>
            {typeof file === "string" && (
              <div className="tool-permission-resource">
                <FileText size={16} />
                <span>
                  <strong>
                    {file.split(/[\\/]/).filter(Boolean).pop() || file}
                  </strong>
                  <small>{t("File locale · percorso completo nei dettagli")}</small>
                </span>
              </div>
            )}
            {typeof command === "string" && (
              <pre className="tool-permission-command">{command}</pre>
            )}
            {canAlways && (
              <p className="permission-scope">
                {t("Puoi consentire solo questa volta oppure ricordare la scelta per questo strumento. Potrai revocarla da Connettori.")}
              </p>
            )}
            <details className="tool-permission-details">
              <summary>{t("Dettagli tecnici")}</summary>
              <pre>
                {JSON.stringify(
                  { strumento: permission.tool, ...permission.input },
                  null,
                  2,
                )}
              </pre>
            </details>
          </>
        )}
      </div>
      <footer>
        <button
          className="permission-dismiss"
          disabled={busy}
          onClick={() => onDecide(false)}
        >
          {isQuestion ? t("Annulla") : t("Rifiuta")}
        </button>
        {canAlways && (
          <button disabled={busy} onClick={() => onDecide(true, true)}>
            {t("Consenti sempre")}
          </button>
        )}
        <button
          className="permission-confirm"
          disabled={busy || (isQuestion && !complete)}
          onClick={() => onDecide(true)}
        >
          {busy ? t("Invio…") : isQuestion ? t("Invia risposte") : t("Consenti")}
        </button>
      </footer>
    </section>
  );
}
