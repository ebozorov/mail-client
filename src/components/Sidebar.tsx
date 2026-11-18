import { FOLDERS, type Folder } from "../types";

interface Props {
  active: Folder | null;
  unread: number;
  email: string;
  demo: boolean;
  onSelect: (f: Folder) => void;
  onCompose: () => void;
  onSignOut: () => void;
}

export default function Sidebar({ active, unread, email, demo, onSelect, onCompose, onSignOut }: Props) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <img src="/icon.svg" alt="" />
        <span>Mail</span>
        {demo && <span className="tag">demo</span>}
      </div>

      <nav>
        {FOLDERS.map((f) => (
          <button
            key={f.id}
            className={"nav-item" + (active === f.id ? " is-active" : "")}
            onClick={() => onSelect(f.id)}
          >
            {f.label}
            {f.id === "inbox" && unread > 0 && <span className="badge">{unread}</span>}
          </button>
        ))}
      </nav>

      <div className="sidebar__footer">
        <button className="btn btn--primary compose-btn" onClick={onCompose}>+ New</button>
        <div className="account" title={email}>
          <span className="account__email">{email}</span>
          <button className="link" onClick={onSignOut}>Sign out</button>
        </div>
      </div>
    </aside>
  );
}
