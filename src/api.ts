import { invoke, isTauri } from "@tauri-apps/api/core";
import { demoBackend } from "./demo";
import type {
  Account, Discovery, Folder, FullMessage, NewAccount, Outgoing, Summary,
} from "./types";

export class ApiError extends Error {
  constructor(public kind: string, message: string) {
    super(message);
  }
}

export interface Backend {
  demo: boolean;
  getAccount(): Promise<Account | null>;
  discover(email: string): Promise<Discovery>;
  addAccount(input: NewAccount): Promise<Account>;
  removeAccount(): Promise<void>;
  list(folder: Folder): Promise<Summary[]>;
  get(folder: Folder, uid: number): Promise<FullMessage>;
  setSeen(folder: Folder, uid: number, seen: boolean): Promise<void>;
  move(folder: Folder, uid: number, target: Folder): Promise<void>;
  remove(folder: Folder, uid: number): Promise<void>;
  send(mail: Outgoing, draftUid?: number | null): Promise<void>;
  saveDraft(mail: Outgoing, draftUid?: number | null): Promise<number | null>;
  openUrl(url: string): Promise<void>;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    const p = e as { kind?: string; message?: string };
    throw new ApiError(p?.kind ?? "other", p?.message ?? String(e));
  }
}

const tauriBackend: Backend = {
  demo: false,
  getAccount: () => call("get_account"),
  discover: (email) => call("discover", { email }),
  addAccount: (input) => call("add_account", { input }),
  removeAccount: () => call("remove_account"),
  list: (folder) => call("list_messages", { folder }),
  get: (folder, uid) => call("get_message", { folder, uid }),
  setSeen: (folder, uid, seen) => call("set_seen", { folder, uid, seen }),
  move: (folder, uid, target) => call("move_message", { folder, uid, target }),
  remove: (folder, uid) => call("delete_message", { folder, uid }),
  send: (mail, draftUid) => call("send_message", { mail, draftUid: draftUid ?? null }),
  saveDraft: (mail, draftUid) => call("save_draft", { mail, draftUid: draftUid ?? null }),
  openUrl: async (url) => {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  },
};

export const api: Backend = isTauri() ? tauriBackend : demoBackend;
