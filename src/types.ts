// Rust tarafidagi (crates/mail-core) serde tuzilmalari bilan mos

export type Folder = "inbox" | "drafts" | "sent" | "archive" | "trash";
export type Security = "tls" | "starttls" | "plain";

export interface Server {
  host: string;
  port: number;
  security: Security;
}

export interface Account {
  email: string;
  name: string;
  username: string;
  imap: Server;
  smtp: Server;
  saveSentCopy: boolean;
}

export type Hint =
  | { kind: "appPassword"; provider: string; url: string }
  | { kind: "oAuthOnly"; provider: string };

export interface Discovery {
  imap: Server;
  smtp: Server;
  username: string;
  saveSentCopy: boolean;
  source: "known" | "ispdb" | "autoconfig" | "guess";
  hint: Hint | null;
}

export interface NewAccount {
  email: string;
  name: string;
  password: string;
  manual?: { username: string; imap: Server; smtp: Server; saveSentCopy: boolean };
}

export interface Addr {
  name: string | null;
  email: string;
}

export interface Summary {
  uid: number;
  folder: Folder;
  from: Addr | null;
  to: Addr[];
  subject: string;
  date: string;
  seen: boolean;
  messageId: string | null;
}

export interface FullMessage extends Summary {
  cc: Addr[];
  text: string;
  html: string | null;
  attachments: { name: string; size: number }[];
}

export interface Outgoing {
  to: string[];
  cc?: string[];
  subject: string;
  body: string;
  inReplyTo?: string | null;
}

export interface ComposeInit {
  to: string[];
  subject: string;
  body: string;
  inReplyTo?: string | null;
  draftUid?: number | null;
}

export type View =
  | { kind: "list" }
  | { kind: "read"; folder: Folder; uid: number }
  | { kind: "compose"; init: ComposeInit };

export const FOLDERS: { id: Folder; label: string }[] = [
  { id: "inbox", label: "Inbox" },
  { id: "drafts", label: "Drafts" },
  { id: "sent", label: "Sent" },
  { id: "archive", label: "Archive" },
];

export const addrLabel = (a: Addr | null | undefined) => (a ? a.name || a.email : "");
export const addrFull = (a: Addr) => (a.name ? `${a.name} <${a.email}>` : a.email);
