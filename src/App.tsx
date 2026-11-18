import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import AccountSetup from "./components/AccountSetup";
import Compose from "./components/Compose";
import MailList from "./components/MailList";
import MailView from "./components/MailView";
import Sidebar from "./components/Sidebar";
import {
  FOLDERS, addrFull, type Account, type ComposeInit, type Folder, type FullMessage, type Summary, type View,
} from "./types";

const POLL_MS = 60_000;

function quote(m: FullMessage): string {
  const who = m.from ? addrFull(m.from) : "someone";
  const when = new Date(m.date).toLocaleString();
  const quoted = m.text.trimEnd().split("\n").map((l) => `> ${l}`).join("\n");
  return `\n\nOn ${when}, ${who} wrote:\n${quoted}\n`;
}

export default function App() {
  const [account, setAccount] = useState<Account | null | undefined>(undefined);
  const [folder, setFolder] = useState<Folder>("inbox");
  const [view, setView] = useState<View>({ kind: "list" });
  const [lists, setLists] = useState<Partial<Record<Folder, Summary[]>>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqId = useRef(0);

  useEffect(() => {
    api.getAccount().then(setAccount).catch(() => setAccount(null));
  }, []);

  const refresh = useCallback(async (f: Folder) => {
    const id = ++reqId.current;
    setLoading(true);
    try {
      const items = await api.list(f);
      setLists((l) => ({ ...l, [f]: items }));
      if (id === reqId.current) setError(null);
    } catch (e) {
      if (id === reqId.current) setError(String((e as Error)?.message ?? e));
    } finally {
      if (id === reqId.current) setLoading(false);
    }
  }, []);

  // Papka almashganda yuklash
  useEffect(() => {
    if (account) refresh(folder);
  }, [account, folder, refresh]);

  // Inbox'ni (va ochiq papkani) vaqti-vaqti bilan yangilash
  useEffect(() => {
    if (!account) return;
    const t = setInterval(() => {
      if (document.hidden) return;
      refresh("inbox");
      if (folder !== "inbox" && view.kind === "list") refresh(folder);
    }, POLL_MS);
    return () => clearInterval(t);
  }, [account, folder, view.kind, refresh]);

  // Inbox o'qilmaganlar soni sidebar'da kerak — boshqa papkada bo'lsak ham yuklab qo'yamiz
  useEffect(() => {
    if (account && folder !== "inbox" && !lists.inbox) refresh("inbox");
  }, [account, folder, lists.inbox, refresh]);

  const patchItem = (f: Folder, uid: number, change: Partial<Summary> | null) =>
    setLists((l) => ({
      ...l,
      [f]: change === null
        ? l[f]?.filter((m) => m.uid !== uid)
        : l[f]?.map((m) => (m.uid === uid ? { ...m, ...change } : m)),
    }));

  const onLoaded = useCallback((m: FullMessage) => {
    setLists((l) => ({ ...l, [m.folder]: l[m.folder]?.map((x) => (x.uid === m.uid ? { ...x, seen: true } : x)) }));
  }, []);

  if (account === undefined) return <div className="app app--loading" />;
  if (account === null) return <AccountSetup onDone={setAccount} />;

  const openFolder = (f: Folder) => {
    setFolder(f);
    setView({ kind: "list" });
  };

  const compose = (init: ComposeInit) => setView({ kind: "compose", init });

  const openMail = async (m: Summary) => {
    if (m.folder === "drafts") {
      try {
        const d = await api.get("drafts", m.uid);
        compose({ to: d.to.map(addrFull), subject: d.subject, body: d.text, draftUid: d.uid });
      } catch (e) {
        setError(String((e as Error)?.message ?? e));
      }
      return;
    }
    setView({ kind: "read", folder: m.folder, uid: m.uid });
  };

  // Ro'yxatdan darhol olib tashlaymiz, xato bo'lsa qayta yuklaymiz
  const act = async (m: FullMessage, op: () => Promise<void>) => {
    patchItem(m.folder, m.uid, null);
    setView({ kind: "list" });
    try {
      await op();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    }
    refresh(m.folder);
  };

  const signOut = async () => {
    if (!confirm(`Sign out of ${account.email}? Mail stays on the server.`)) return;
    await api.removeAccount();
    setLists({});
    setView({ kind: "list" });
    setFolder("inbox");
    setAccount(null);
  };

  const unread = lists.inbox?.filter((m) => !m.seen).length ?? 0;
  const folderLabel = FOLDERS.find((f) => f.id === folder)?.label ?? folder;
  const current = view.kind === "read" ? lists[view.folder]?.find((m) => m.uid === view.uid) : undefined;
  const title =
    view.kind === "compose"
      ? view.init.draftUid != null ? "Draft" : "New message"
      : view.kind === "read"
        ? current?.subject || "(no subject)"
        : folderLabel;

  return (
    <div className="app">
      <Sidebar
        active={view.kind === "compose" ? null : folder}
        unread={unread}
        email={account.email}
        demo={api.demo}
        onSelect={openFolder}
        onCompose={() => compose({ to: [], subject: "", body: "" })}
        onSignOut={signOut}
      />

      <main className="panel">
        <header className="panel__header">
          <h1>{title}</h1>
          {view.kind === "list" ? (
            <button
              className={"refresh" + (loading ? " is-spinning" : "")}
              onClick={() => refresh(folder)}
              aria-label="Refresh"
              title="Refresh"
            >
              ↻
            </button>
          ) : (
            <div className="dots" aria-hidden><span /><span /></div>
          )}
        </header>

        <div className="panel__body">
          {view.kind === "list" && (
            <>
              {error && lists[folder]?.length ? <p className="note note--error">{error}</p> : null}
              <MailList
                mails={lists[folder]}
                loading={loading}
                error={error}
                onOpen={openMail}
                onRetry={() => refresh(folder)}
              />
            </>
          )}

          {view.kind === "read" && (
            <MailView
              key={`${view.folder}:${view.uid}`}
              folder={view.folder}
              uid={view.uid}
              onLoaded={onLoaded}
              onBack={() => setView({ kind: "list" })}
              onReply={(m) =>
                compose({
                  to: m.from ? [addrFull(m.from)] : [],
                  subject: /^re:/i.test(m.subject) ? m.subject : `Re: ${m.subject}`,
                  body: quote(m),
                  inReplyTo: m.messageId,
                })
              }
              onArchive={(m) => act(m, () => api.move(m.folder, m.uid, "archive"))}
              onDelete={(m) => act(m, () => api.remove(m.folder, m.uid))}
              onMarkUnread={(m) => {
                patchItem(m.folder, m.uid, { seen: false });
                setView({ kind: "list" });
                api.setSeen(m.folder, m.uid, false).catch((e) => setError(String(e?.message ?? e)));
              }}
            />
          )}

          {view.kind === "compose" && (
            <Compose
              init={view.init}
              onSent={() => {
                refresh("drafts");
                refresh("sent");
                openFolder("sent");
              }}
              onDraftSaved={() => refresh("drafts")}
              onClose={() => setView({ kind: "list" })}
            />
          )}
        </div>
      </main>
    </div>
  );
}
