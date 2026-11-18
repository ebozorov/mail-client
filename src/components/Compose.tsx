import { useRef, useState, type KeyboardEvent } from "react";
import { api } from "../api";
import type { ComposeInit, Outgoing } from "../types";

interface Props {
  init: ComposeInit;
  onSent: () => void;
  onClose: () => void;
  onDraftSaved: () => void;
}

const looksLikeEmail = (s: string) => /^[^@\s<>]+@[^@\s<>]+\.[^@\s<>]+$/.test(s.replace(/^.*<(.+)>\s*$/, "$1").trim());

export default function Compose({ init, onSent, onClose, onDraftSaved }: Props) {
  const [to, setTo] = useState<string[]>(init.to);
  const [toInput, setToInput] = useState("");
  const [subject, setSubject] = useState(init.subject);
  const [body, setBody] = useState(init.body);
  const [draftUid, setDraftUid] = useState<number | null>(init.draftUid ?? null);
  const [status, setStatus] = useState<null | "sending" | "saving">(null);
  const [error, setError] = useState<string | null>(null);
  const [savedNote, setSavedNote] = useState(false);
  const bodyRef = useRef<HTMLTextAreaElement>(null);

  const recipients = toInput.trim() ? [...to, toInput.trim()] : to;
  const invalid = recipients.filter((r) => !looksLikeEmail(r));
  const canSend = recipients.length > 0 && invalid.length === 0 && !status;

  const mail = (): Outgoing => ({ to: recipients, subject, body, inReplyTo: init.inReplyTo ?? null });

  const commitRecipient = () => {
    const v = toInput.trim().replace(/,$/, "");
    if (v && !to.includes(v)) setTo([...to, v]);
    setToInput("");
  };

  const onToKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === "Enter" || e.key === "," || e.key === "Tab") && toInput.trim()) {
      e.preventDefault();
      commitRecipient();
    } else if (e.key === "Backspace" && !toInput && to.length) {
      setTo(to.slice(0, -1));
    }
  };

  const send = async () => {
    if (!canSend) return;
    setStatus("sending");
    setError(null);
    try {
      await api.send(mail(), draftUid);
      onSent();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
      setStatus(null);
    }
  };

  const saveDraft = async () => {
    if (status) return;
    setStatus("saving");
    setError(null);
    try {
      const uid = await api.saveDraft(mail(), draftUid);
      setDraftUid(uid);
      setSavedNote(true);
      setTimeout(() => setSavedNote(false), 2000);
      onDraftSaved();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setStatus(null);
    }
  };

  return (
    <form
      className="compose"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          send();
        }
      }}
    >
      <div className="field">
        <label htmlFor="to">To:</label>
        {to.map((r) => (
          <span key={r} className={"chip" + (looksLikeEmail(r) ? "" : " chip--bad")} title={looksLikeEmail(r) ? r : "Invalid address"}>
            {r}
            <button type="button" className="chip__x" aria-label={`Remove ${r}`} onClick={() => setTo(to.filter((x) => x !== r))}>
              ×
            </button>
          </span>
        ))}
        <input
          id="to"
          className="field__input"
          value={toInput}
          autoFocus={!init.to.length}
          onChange={(e) => setToInput(e.target.value)}
          onKeyDown={onToKey}
          onBlur={commitRecipient}
        />
      </div>

      <div className="field">
        <label htmlFor="subject">Subject:</label>
        <input id="subject" className="field__input field__input--strong" value={subject} onChange={(e) => setSubject(e.target.value)} />
      </div>

      <textarea
        ref={bodyRef}
        className="compose__body"
        value={body}
        autoFocus={init.to.length > 0}
        onChange={(e) => setBody(e.target.value)}
      />

      {error && <p className="note note--error" role="alert">{error}</p>}

      <div className="actions">
        <button type="submit" className="btn btn--primary btn--lg" disabled={!canSend}>
          {status === "sending" ? "Sending…" : "Send"}
        </button>
        <button type="button" className="btn" onClick={saveDraft} disabled={!!status}>
          {status === "saving" ? "Saving…" : savedNote ? "Saved ✓" : "Save draft"}
        </button>
        <button type="button" className="btn btn--ghost" onClick={onClose} disabled={status === "sending"}>
          Discard
        </button>
      </div>
    </form>
  );
}
