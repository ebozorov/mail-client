// Brauzerda (make web) ishlatiladigan soxta backend — haqiqiy serverga ulanmaydi
import type { Backend } from "./api";
import type { Account, Folder, FullMessage, Outgoing } from "./types";

const KEY = "mail-client:demo";
const ago = (h: number) => new Date(Date.now() - h * 3600_000).toISOString();
const me = { name: "Demo User", email: "demo@example.com" };

const account: Account = {
  email: me.email,
  name: me.name,
  username: me.email,
  imap: { host: "imap.example.com", port: 993, security: "tls" },
  smtp: { host: "smtp.example.com", port: 465, security: "tls" },
  saveSentCopy: true,
};

function msg(uid: number, folder: Folder, from: { name: string; email: string }, subject: string, text: string, h: number, seen: boolean, html: string | null = null): FullMessage {
  return {
    uid, folder, from, to: [me], cc: [], subject, text, html, date: ago(h), seen,
    messageId: `demo-${uid}@example.com`, attachments: [],
  };
}

const SEED: FullMessage[] = [
  msg(1, "inbox", { name: "Aziza Karimova", email: "aziza@example.com" }, "Weekly sync notes",
    "Hi!\n\nHere are the notes from today's sync. Let's finalize the roadmap by Friday.\n\nThanks,\nAziza", 2, false),
  msg(2, "inbox", { name: "Design Team", email: "design@example.com" }, "New mockups are ready",
    "The new minimal mockups are uploaded.", 26, true,
    "<h2 style='color:#2f5b45'>New mockups</h2><p>The new <b>minimal</b> mockups are uploaded. <a href='https://example.com'>Open them</a>.</p>"),
  { ...msg(3, "sent", me, "Re: Server migration", "Sounds good, let's do it on Sunday night.", 50, true), to: [{ name: "Bekzod", email: "bekzod@example.com" }] },
  msg(4, "archive", { name: "Billing", email: "billing@example.com" }, "Your invoice for October", "Your invoice is attached. Thank you!", 480, true),
];

interface State { signedIn: boolean; mails: FullMessage[]; nextUid: number }

function load(): State {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return JSON.parse(raw) as State;
  } catch { /* yangidan boshlaymiz */ }
  return { signedIn: false, mails: SEED, nextUid: 100 };
}

let state = load();
const save = () => {
  try { localStorage.setItem(KEY, JSON.stringify(state)); } catch { /* xotirada qoladi */ }
};
const wait = (ms = 250) => new Promise((r) => setTimeout(r, ms));
const find = (folder: Folder, uid: number) => {
  const m = state.mails.find((x) => x.folder === folder && x.uid === uid);
  if (!m) throw new Error("Message not found");
  return m;
};
const toAddr = (s: string) => {
  const m = s.match(/^\s*(.*?)\s*<(.+)>\s*$/);
  return m ? { name: m[1] || null, email: m[2] } : { name: null, email: s.trim() };
};
const fromOutgoing = (out: Outgoing, folder: Folder): FullMessage => ({
  uid: state.nextUid++, folder, from: me, to: out.to.map(toAddr), cc: [], subject: out.subject,
  text: out.body, html: null, date: new Date().toISOString(), seen: true,
  messageId: null, attachments: [],
});

export const demoBackend: Backend = {
  demo: true,
  async getAccount() { return state.signedIn ? account : null; },
  async discover(email) {
    await wait();
    const domain = email.split("@")[1] ?? "example.com";
    return {
      imap: { host: `imap.${domain}`, port: 993, security: "tls" },
      smtp: { host: `smtp.${domain}`, port: 465, security: "tls" },
      username: email, saveSentCopy: true, source: "guess",
      hint: domain === "gmail.com" ? { kind: "appPassword", provider: "Google", url: "https://myaccount.google.com/apppasswords" } : null,
    };
  },
  async addAccount(input) {
    await wait(600);
    state.signedIn = true; save();
    return { ...account, email: input.email, name: input.name || account.name };
  },
  async removeAccount() { state = { signedIn: false, mails: SEED, nextUid: 100 }; save(); },
  async list(folder) {
    await wait(150);
    return state.mails.filter((m) => m.folder === folder).sort((a, b) => b.date.localeCompare(a.date));
  },
  async get(folder, uid) {
    await wait(150);
    const m = find(folder, uid);
    m.seen = true; save();
    return m;
  },
  async setSeen(folder, uid, seen) { find(folder, uid).seen = seen; save(); },
  async move(folder, uid, target) { find(folder, uid).folder = target; save(); },
  async remove(folder, uid) {
    const m = find(folder, uid);
    if (folder === "trash") state.mails = state.mails.filter((x) => x !== m);
    else m.folder = "trash";
    save();
  },
  async send(out, draftUid) {
    await wait(500);
    if (!out.to.length) throw new Error("No recipients");
    if (draftUid != null) state.mails = state.mails.filter((x) => !(x.folder === "drafts" && x.uid === draftUid));
    state.mails.push(fromOutgoing(out, "sent")); save();
  },
  async saveDraft(out, draftUid) {
    if (draftUid != null) state.mails = state.mails.filter((x) => !(x.folder === "drafts" && x.uid === draftUid));
    const d = fromOutgoing(out, "drafts");
    state.mails.push(d); save();
    return d.uid;
  },
  async openUrl(url) { window.open(url, "_blank", "noopener"); },
};
