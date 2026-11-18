import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { formatDate, formatSize } from "../format";
import { addrFull, addrLabel, type Folder, type FullMessage } from "../types";

interface Props {
  folder: Folder;
  uid: number;
  onLoaded: (m: FullMessage) => void;
  onBack: () => void;
  onReply: (m: FullMessage) => void;
  onArchive: (m: FullMessage) => void;
  onDelete: (m: FullMessage) => void;
  onMarkUnread: (m: FullMessage) => void;
}

// Skriptlar, tashqi rasmlar (kuzatuv piksellari), shriftlar va so'rovlar bloklanadi
const CSP =
  "default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'; font-src data:; form-action 'none'";

function HtmlBody({ html }: { html: string }) {
  const ref = useRef<HTMLIFrameElement>(null);
  const doc = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${CSP}">
<style>body{margin:16px;font:15px/1.5 system-ui,sans-serif;color:#222;background:#fff;word-wrap:break-word}img{max-width:100%;height:auto}</style>
</head><body>${html}</body></html>`;

  const onLoad = () => {
    const d = ref.current?.contentDocument;
    if (!d) return;
    // sandbox'da skript yo'q, shuning uchun havolalarni ota sahifadan ushlaymiz
    d.addEventListener("click", (e) => {
      const a = (e.target as Element | null)?.closest?.("a[href]");
      if (!a) return;
      e.preventDefault();
      const href = a.getAttribute("href") ?? "";
      if (/^(https?:|mailto:)/i.test(href)) api.openUrl(href);
    });
  };

  return <iframe ref={ref} className="html-body" title="Message" sandbox="allow-same-origin" srcDoc={doc} onLoad={onLoad} />;
}

export default function MailView(p: Props) {
  const [mail, setMail] = useState<FullMessage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"html" | "text">("html");
  const { onLoaded } = p;

  useEffect(() => {
    let live = true;
    setMail(null);
    setError(null);
    api
      .get(p.folder, p.uid)
      .then((m) => {
        if (!live) return;
        setMail(m);
        onLoaded(m);
      })
      .catch((e) => live && setError(String(e?.message ?? e)));
    return () => {
      live = false;
    };
  }, [p.folder, p.uid, onLoaded]);

  if (error) {
    return (
      <div className="empty">
        <p>{error}</p>
        <button className="btn" onClick={p.onBack}>Back</button>
      </div>
    );
  }
  if (!mail) return <p className="empty">Loading…</p>;

  return (
    <article className="mail-view">
      <div className="field">
        <label>From:</label>
        {mail.from && <span className="chip" title={mail.from.email}>{addrLabel(mail.from)}</span>}
        <span className="field__aside">{formatDate(mail.date, true)}</span>
      </div>
      <div className="field">
        <label>To:</label>
        {mail.to.map((t) => <span key={t.email} className="chip" title={addrFull(t)}>{addrLabel(t)}</span>)}
        {mail.cc.length > 0 && <label>Cc:</label>}
        {mail.cc.map((t) => <span key={t.email} className="chip" title={addrFull(t)}>{addrLabel(t)}</span>)}
      </div>

      {mail.html && mode === "html" ? (
        <HtmlBody html={mail.html} />
      ) : (
        <div className="mail-view__body">{mail.text}</div>
      )}

      {mail.attachments.length > 0 && (
        <div className="attachments">
          {mail.attachments.map((a, i) => (
            <span key={i} className="chip chip--file">{a.name} · {formatSize(a.size)}</span>
          ))}
        </div>
      )}

      <div className="actions">
        <button className="btn btn--primary" onClick={() => p.onReply(mail)}>Reply</button>
        {mail.folder !== "archive" && <button className="btn" onClick={() => p.onArchive(mail)}>Archive</button>}
        <button className="btn" onClick={() => p.onDelete(mail)}>Delete</button>
        <button className="btn btn--ghost" onClick={() => p.onMarkUnread(mail)}>Mark unread</button>
        {mail.html && (
          <button className="btn btn--ghost" onClick={() => setMode(mode === "html" ? "text" : "html")}>
            {mode === "html" ? "Plain text" : "Formatted"}
          </button>
        )}
        <button className="btn btn--ghost push-right" onClick={p.onBack}>Back</button>
      </div>
    </article>
  );
}
