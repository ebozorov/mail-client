import { formatDate } from "../format";
import { addrLabel, type Summary } from "../types";

interface Props {
  mails: Summary[] | undefined;
  loading: boolean;
  error: string | null;
  onOpen: (m: Summary) => void;
  onRetry: () => void;
}

export default function MailList({ mails, loading, error, onOpen, onRetry }: Props) {
  if (error && !mails?.length) {
    return (
      <div className="empty">
        <p>{error}</p>
        <button className="btn" onClick={onRetry}>Try again</button>
      </div>
    );
  }
  if (!mails) return <p className="empty">{loading ? "Loading…" : ""}</p>;
  if (mails.length === 0) return <p className="empty">Nothing here yet</p>;

  return (
    <ul className="mail-list">
      {mails.map((m) => {
        const outgoing = m.folder === "sent" || m.folder === "drafts";
        const who = outgoing ? `To: ${m.to.map(addrLabel).join(", ") || "—"}` : addrLabel(m.from) || "(unknown)";
        return (
          <li key={m.uid}>
            <button className={"mail-row" + (m.seen ? "" : " is-unread")} onClick={() => onOpen(m)}>
              <span className="mail-row__who">{who}</span>
              <span className="mail-row__subject">{m.subject || "(no subject)"}</span>
              <span className="mail-row__date">{formatDate(m.date)}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
