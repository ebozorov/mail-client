import { useState, type FormEvent } from "react";
import { api, ApiError } from "../api";
import type { Account, Discovery, Security, Server } from "../types";

interface Props {
  onDone: (a: Account) => void;
}

const SECURITY: { id: Security; label: string }[] = [
  { id: "tls", label: "SSL/TLS" },
  { id: "starttls", label: "STARTTLS" },
  { id: "plain", label: "None" },
];

function ServerFields({ label, value, onChange }: { label: string; value: Server; onChange: (s: Server) => void }) {
  return (
    <div className="field">
      <label>{label}:</label>
      <input
        className="field__input"
        value={value.host}
        placeholder="host"
        aria-label={`${label} host`}
        onChange={(e) => onChange({ ...value, host: e.target.value })}
      />
      <input
        className="field__input field__input--port"
        value={value.port}
        inputMode="numeric"
        aria-label={`${label} port`}
        onChange={(e) => onChange({ ...value, port: Number(e.target.value.replace(/\D/g, "")) || 0 })}
      />
      <select
        className="field__select"
        value={value.security}
        aria-label={`${label} security`}
        onChange={(e) => onChange({ ...value, security: e.target.value as Security })}
      >
        {SECURITY.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
      </select>
    </div>
  );
}

export default function AccountSetup({ onDone }: Props) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [found, setFound] = useState<Discovery | null>(null);
  const [foundFor, setFoundFor] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [username, setUsername] = useState("");
  const [imap, setImap] = useState<Server>({ host: "", port: 993, security: "tls" });
  const [smtp, setSmtp] = useState<Server>({ host: "", port: 465, security: "tls" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const validEmail = /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email.trim());

  const lookup = async () => {
    const e = email.trim();
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(e) || e === foundFor) return found;
    try {
      const d = await api.discover(e);
      setFound(d);
      setFoundFor(e);
      setUsername(d.username);
      setImap(d.imap);
      setSmtp(d.smtp);
      return d;
    } catch {
      return null;
    }
  };

  const submit = async (ev: FormEvent) => {
    ev.preventDefault();
    if (!validEmail || !password || busy) return;
    setBusy(true);
    setError(null);
    try {
      await lookup();
      const account = await api.addAccount({
        email: email.trim(),
        name,
        password,
        manual: advanced ? { username, imap, smtp, saveSentCopy: found?.saveSentCopy ?? true } : undefined,
      });
      onDone(account);
    } catch (e) {
      const err = e instanceof ApiError ? e : new ApiError("other", String(e));
      setError(err.message);
      // Ulanib bo'lmasa — qo'lda sozlashni ochamiz
      if (err.kind === "connect" || err.kind === "tls") setAdvanced(true);
    } finally {
      setBusy(false);
    }
  };

  const hint = found?.hint;

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img src="/icon.svg" alt="" />
          <span>Mail</span>
        </div>
        <p className="sidebar__note">
          Your password is stored in the system keyring and only sent to your mail server.
        </p>
      </aside>

      <main className="panel">
        <header className="panel__header">
          <h1>Add account</h1>
          <div className="dots" aria-hidden><span /><span /></div>
        </header>

        <form className="panel__body setup" onSubmit={submit}>
          <div className="field">
            <label htmlFor="name">Name:</label>
            <input id="name" className="field__input" value={name} placeholder="Your name" onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="field">
            <label htmlFor="email">Email:</label>
            <input
              id="email"
              className="field__input field__input--strong"
              type="email"
              autoFocus
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              onBlur={lookup}
            />
            {found && foundFor === email.trim() && <span className="field__aside">{found.imap.host}</span>}
          </div>
          <div className="field">
            <label htmlFor="password">Password:</label>
            <input
              id="password"
              className="field__input"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>

          {hint?.kind === "appPassword" && (
            <p className="note">
              {hint.provider} needs an <b>app password</b>, not your normal one.{" "}
              <a href={hint.url} onClick={(e) => { e.preventDefault(); api.openUrl(hint.url); }}>Create one here</a>.
            </p>
          )}
          {hint?.kind === "oAuthOnly" && (
            <p className="note note--warn">
              {hint.provider} accounts need OAuth sign-in. This is coming in the next version.
            </p>
          )}

          {advanced && (
            <div className="advanced">
              <div className="field">
                <label htmlFor="username">Username:</label>
                <input id="username" className="field__input" value={username} onChange={(e) => setUsername(e.target.value)} />
              </div>
              <ServerFields label="IMAP" value={imap} onChange={setImap} />
              <ServerFields label="SMTP" value={smtp} onChange={setSmtp} />
            </div>
          )}

          {error && <p className="note note--error" role="alert">{error}</p>}

          <div className="actions">
            <button type="submit" className="btn btn--primary btn--lg" disabled={!validEmail || !password || busy}>
              {busy ? "Connecting…" : "Connect"}
            </button>
            <button
              type="button"
              className="btn btn--ghost"
              onClick={async () => {
                if (!advanced) await lookup();
                setAdvanced(!advanced);
              }}
            >
              {advanced ? "Hide settings" : "Server settings"}
            </button>
          </div>
        </form>
      </main>
    </div>
  );
}
