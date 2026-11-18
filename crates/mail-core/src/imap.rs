//! IMAP klient: bitta doimiy ulanish, uzilsa avtomatik qayta ulanadi.

use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use async_imap::types::{Fetch, Flag, NameAttribute};
use futures::future::BoxFuture;
use futures::TryStreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::config::{Account, Folder, Security, Server};
use crate::message::{self, FullMessage, Summary};
use crate::tls;
use crate::{Error, Result};

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> Io for T {}

type Stream = Box<dyn Io>;
type Session = async_imap::Session<Stream>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const OP_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ImapClient {
    account: Account,
    password: String,
    conn: Mutex<Option<Conn>>,
}

pub struct Conn {
    session: Session,
    folders: HashMap<Folder, String>,
    can_move: bool,
}

impl ImapClient {
    pub fn new(account: Account, password: String) -> Self {
        ImapClient { account, password, conn: Mutex::new(None) }
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    /// Ulanib, login qilib, papkalarni aniqlaydi. Akkaunt qo'shishda tekshiruv uchun.
    pub async fn check(&self) -> Result<()> {
        self.with(|_| Box::pin(async { Ok(()) })).await
    }

    /// Ulanishni olib, amalni bajaradi. Ulanish uzilgan bo'lsa — bir marta qayta ulanadi.
    async fn with<T, F>(&self, op: F) -> Result<T>
    where
        F: for<'a> Fn(&'a mut Conn) -> BoxFuture<'a, Result<T>>,
    {
        let mut guard = self.conn.lock().await;
        for attempt in 0..2 {
            if guard.is_none() {
                *guard = Some(Conn::open(&self.account, &self.password).await?);
            }
            let conn = guard.as_mut().expect("connected above");
            let res = match timeout(OP_TIMEOUT, op(conn)).await {
                Ok(r) => r,
                Err(_) => Err(Error::Connect("operation timed out".into())),
            };
            match res {
                Err(e) if e.is_connection_lost() && attempt == 0 => {
                    *guard = None; // eskirgan ulanishni tashlab, qayta urinamiz
                }
                Err(e) if e.is_connection_lost() => {
                    *guard = None;
                    return Err(e);
                }
                other => return other,
            }
        }
        unreachable!()
    }

    /// Papkadagi oxirgi `limit` ta xat (yangisi birinchi)
    pub async fn list(&self, folder: Folder, limit: u32) -> Result<Vec<Summary>> {
        self.with(move |c| Box::pin(c.list(folder, limit))).await
    }

    /// To'liq xat; o'qilgan deb belgilaydi
    pub async fn get(&self, folder: Folder, uid: u32) -> Result<FullMessage> {
        self.with(move |c| Box::pin(c.get(folder, uid))).await
    }

    pub async fn set_seen(&self, folder: Folder, uid: u32, seen: bool) -> Result<()> {
        self.with(move |c| Box::pin(c.set_seen(folder, uid, seen))).await
    }

    pub async fn move_to(&self, folder: Folder, uid: u32, target: Folder) -> Result<()> {
        self.with(move |c| Box::pin(c.move_to(folder, uid, target))).await
    }

    /// Trash'ga ko'chiradi; Trash'da bo'lsa butunlay o'chiradi
    pub async fn delete(&self, folder: Folder, uid: u32) -> Result<()> {
        self.with(move |c| {
            Box::pin(async move {
                if folder == Folder::Trash {
                    c.expunge_uid(folder, uid).await
                } else {
                    c.move_to(folder, uid, Folder::Trash).await
                }
            })
        })
        .await
    }

    /// Xatni papkaga qo'shadi (Sent nusxasi yoki qoralama). Yangi UID'ni qaytaradi.
    pub async fn append(&self, folder: Folder, raw: &[u8], flags: &str, message_id: &str) -> Result<Option<u32>> {
        let (raw, flags, message_id) = (raw.to_vec(), flags.to_string(), message_id.to_string());
        self.with(move |c| {
            let (raw, flags, message_id) = (raw.clone(), flags.clone(), message_id.clone());
            Box::pin(async move { c.append(folder, &raw, &flags, &message_id).await })
        })
        .await
    }

    /// Eski qoralama versiyasini Trash'ga tashlamasdan o'chiradi
    pub async fn expunge_uid(&self, folder: Folder, uid: u32) -> Result<()> {
        self.with(move |c| Box::pin(c.expunge_uid(folder, uid))).await
    }

    pub async fn logout(&self) {
        if let Some(mut c) = self.conn.lock().await.take() {
            let _ = timeout(Duration::from_secs(5), c.session.logout()).await;
        }
    }
}

async fn open_stream(server: &Server) -> Result<async_imap::Client<Stream>> {
    let addr = (server.host.as_str(), server.port);
    let tcp = match timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(Error::Connect(format!("{}:{} ({e})", server.host, server.port))),
        Err(_) => return Err(Error::Connect(format!("{}:{} (timed out)", server.host, server.port))),
    };

    let wrap_tls = |stream: Stream| async move {
        let tls = tls::connector()?
            .connect(tls::server_name(&server.host)?, stream)
            .await
            .map_err(|e| Error::Tls(e.to_string()))?;
        Ok::<Stream, Error>(Box::new(tls))
    };

    let mut client = match server.security {
        Security::Tls => async_imap::Client::new(wrap_tls(Box::new(tcp)).await?),
        Security::Plain | Security::StartTls => async_imap::Client::new(Box::new(tcp) as Stream),
    };
    read_greeting(&mut client).await?;

    if server.security == Security::StartTls {
        client.run_command_and_check_ok("STARTTLS", None).await?;
        let stream = wrap_tls(client.into_inner()).await?;
        // STARTTLS'dan keyin server qayta salomlashmaydi
        client = async_imap::Client::new(stream);
    }
    Ok(client)
}

async fn read_greeting(client: &mut async_imap::Client<Stream>) -> Result<()> {
    match timeout(CONNECT_TIMEOUT, client.read_response()).await {
        Ok(Ok(Some(_))) => Ok(()),
        Ok(Ok(None)) => Err(Error::Connect("server closed the connection".into())),
        Ok(Err(e)) => Err(Error::Connect(e.to_string())),
        Err(_) => Err(Error::Connect("no greeting from server".into())),
    }
}

fn seq_range(exists: u32, limit: u32) -> Option<String> {
    if exists == 0 || limit == 0 {
        return None;
    }
    let start = exists.saturating_sub(limit - 1).max(1);
    Some(format!("{start}:{exists}"))
}

fn is_seen(f: &Fetch) -> bool {
    f.flags().any(|fl| matches!(fl, Flag::Seen))
}

fn internal_date(f: &Fetch) -> Option<String> {
    f.internal_date().map(|d| d.to_rfc3339())
}

/// LIST natijasidan papka rollarini aniqlaydi: avval RFC 6154 belgilari, keyin mashhur nomlar.
fn resolve_folders(names: &[(String, Vec<NameAttribute<'_>>)]) -> HashMap<Folder, String> {
    let mut map = HashMap::new();
    let selectable = names.iter().filter(|(_, attrs)| !attrs.iter().any(|a| matches!(a, NameAttribute::NoSelect)));

    for (name, attrs) in selectable.clone() {
        if name.eq_ignore_ascii_case("INBOX") {
            map.insert(Folder::Inbox, name.clone());
        }
        for a in attrs {
            let role = match a {
                NameAttribute::Sent => Folder::Sent,
                NameAttribute::Drafts => Folder::Drafts,
                NameAttribute::Archive => Folder::Archive,
                NameAttribute::Trash => Folder::Trash,
                _ => continue,
            };
            map.entry(role).or_insert_with(|| name.clone());
        }
    }

    const FALLBACK: &[(Folder, &[&str])] = &[
        (Folder::Sent, &["Sent", "Sent Items", "Sent Messages", "Sent Mail", "INBOX.Sent", "Отправленные"]),
        (Folder::Drafts, &["Drafts", "INBOX.Drafts", "Черновики"]),
        (Folder::Archive, &["Archive", "Archives", "INBOX.Archive", "Архив"]),
        (Folder::Trash, &["Trash", "Deleted Items", "Deleted Messages", "Bin", "INBOX.Trash", "Корзина"]),
    ];
    for (role, candidates) in FALLBACK {
        if map.contains_key(role) {
            continue;
        }
        if let Some((name, _)) = selectable.clone().find(|(n, _)| candidates.iter().any(|c| n.eq_ignore_ascii_case(c)))
        {
            map.insert(*role, name.clone());
        }
    }

    // Gmail'da "Archive" yo'q — arxivlash "All Mail"ga ko'chirish demak
    if let std::collections::hash_map::Entry::Vacant(slot) = map.entry(Folder::Archive) {
        if let Some((name, _)) = selectable.clone().find(|(_, a)| a.iter().any(|x| matches!(x, NameAttribute::All))) {
            slot.insert(name.clone());
        }
    }
    map
}

impl Conn {
    async fn open(account: &Account, password: &str) -> Result<Conn> {
        let client = open_stream(&account.imap).await?;
        let mut session = client.login(&account.username, password).await.map_err(|(e, _)| match e {
            async_imap::error::Error::No(m) | async_imap::error::Error::Bad(m) => Error::Auth(m),
            other => Error::from(other),
        })?;

        let caps = session.capabilities().await?;
        let can_move = caps.has_str("MOVE");

        let names: Vec<_> = session.list(Some(""), Some("*")).await?.try_collect().await?;
        let owned: Vec<(String, Vec<NameAttribute<'_>>)> =
            names.iter().map(|n| (n.name().to_string(), n.attributes().to_vec())).collect();
        let mut folders = resolve_folders(&owned);
        folders.entry(Folder::Inbox).or_insert_with(|| "INBOX".into());

        Ok(Conn { session, folders, can_move })
    }

    fn name(&self, folder: Folder) -> Option<String> {
        self.folders.get(&folder).cloned()
    }

    /// Papka bo'lmasa yaratadi (masalan, yangi serverda Archive yo'q)
    async fn ensure(&mut self, folder: Folder) -> Result<String> {
        if let Some(n) = self.name(folder) {
            return Ok(n);
        }
        let name = folder.default_name().to_string();
        // Boshqa klient yaratib qo'ygan bo'lishi mumkin — xatoni e'tiborsiz qoldiramiz
        let _ = self.session.create(&name).await;
        self.folders.insert(folder, name.clone());
        Ok(name)
    }

    async fn select(&mut self, folder: Folder) -> Result<Option<u32>> {
        let Some(name) = self.name(folder) else { return Ok(None) };
        let mb = self.session.select(&name).await?;
        Ok(Some(mb.exists))
    }

    async fn list(&mut self, folder: Folder, limit: u32) -> Result<Vec<Summary>> {
        let Some(exists) = self.select(folder).await? else { return Ok(vec![]) };
        let Some(range) = seq_range(exists, limit) else { return Ok(vec![]) };

        let fetches: Vec<Fetch> = self
            .session
            .fetch(&range, "(UID FLAGS INTERNALDATE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE MESSAGE-ID)])")
            .await?
            .try_collect()
            .await?;

        let mut out: Vec<Summary> = fetches
            .iter()
            .filter_map(|f| {
                let uid = f.uid?;
                Some(message::summary(f.header().unwrap_or_default(), uid, folder, is_seen(f), internal_date(f)))
            })
            .collect();
        out.sort_by_key(|m| std::cmp::Reverse(m.uid));
        Ok(out)
    }

    async fn get(&mut self, folder: Folder, uid: u32) -> Result<FullMessage> {
        if self.select(folder).await?.is_none() {
            return Err(Error::NotFound);
        }
        let fetches: Vec<Fetch> = self
            .session
            .uid_fetch(uid.to_string(), "(UID FLAGS INTERNALDATE BODY.PEEK[])")
            .await?
            .try_collect()
            .await?;
        let f = fetches.iter().find(|f| f.uid == Some(uid)).ok_or(Error::NotFound)?;
        let raw = f.body().ok_or(Error::NotFound)?;
        let msg = message::full(raw, uid, folder, true, internal_date(f));
        if !is_seen(f) {
            self.store(uid, "+FLAGS.SILENT (\\Seen)").await?;
        }
        Ok(msg)
    }

    async fn store(&mut self, uid: u32, query: &str) -> Result<()> {
        let _: Vec<Fetch> = self.session.uid_store(uid.to_string(), query).await?.try_collect().await?;
        Ok(())
    }

    async fn set_seen(&mut self, folder: Folder, uid: u32, seen: bool) -> Result<()> {
        if self.select(folder).await?.is_none() {
            return Err(Error::NotFound);
        }
        self.store(uid, if seen { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" }).await
    }

    async fn move_to(&mut self, folder: Folder, uid: u32, target: Folder) -> Result<()> {
        if folder == target {
            return Ok(());
        }
        let target_name = self.ensure(target).await?;
        if self.select(folder).await?.is_none() {
            return Err(Error::NotFound);
        }
        if self.can_move {
            self.session.uid_mv(uid.to_string(), &target_name).await?;
        } else {
            // MOVE yo'q eski serverlar: COPY + \Deleted + EXPUNGE
            self.session.uid_copy(uid.to_string(), &target_name).await?;
            self.store(uid, "+FLAGS.SILENT (\\Deleted)").await?;
            let _: Vec<_> = self.session.expunge().await?.try_collect().await?;
        }
        Ok(())
    }

    async fn expunge_uid(&mut self, folder: Folder, uid: u32) -> Result<()> {
        if self.select(folder).await?.is_none() {
            return Err(Error::NotFound);
        }
        self.store(uid, "+FLAGS.SILENT (\\Deleted)").await?;
        let _: Vec<_> = self.session.expunge().await?.try_collect().await?;
        Ok(())
    }

    async fn append(&mut self, folder: Folder, raw: &[u8], flags: &str, message_id: &str) -> Result<Option<u32>> {
        let name = self.ensure(folder).await?;
        self.session.append(&name, Some(flags), None, raw).await?;
        // APPENDUID hamma serverda yo'q — yangi UID'ni Message-ID bo'yicha qidiramiz
        self.session.select(&name).await?;
        let query = format!("HEADER Message-ID \"{}\"", message_id.replace(['"', '\\'], ""));
        let uids = self.session.uid_search(query).await?;
        Ok(uids.into_iter().max())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn range() {
        assert_eq!(seq_range(0, 50), None);
        assert_eq!(seq_range(10, 50).as_deref(), Some("1:10"));
        assert_eq!(seq_range(120, 50).as_deref(), Some("71:120"));
        assert_eq!(seq_range(50, 50).as_deref(), Some("1:50"));
    }

    #[test]
    fn resolves_special_use_and_fallbacks() {
        let names = vec![
            ("INBOX".to_string(), vec![]),
            ("[Gmail]".to_string(), vec![NameAttribute::NoSelect]),
            ("[Gmail]/Sent Mail".to_string(), vec![NameAttribute::Sent]),
            ("[Gmail]/All Mail".to_string(), vec![NameAttribute::All]),
            ("[Gmail]/Drafts".to_string(), vec![NameAttribute::Drafts]),
            ("Deleted Items".to_string(), vec![NameAttribute::Extension(Cow::Borrowed("\\HasNoChildren"))]),
        ];
        let m = resolve_folders(&names);
        assert_eq!(m[&Folder::Inbox], "INBOX");
        assert_eq!(m[&Folder::Sent], "[Gmail]/Sent Mail");
        assert_eq!(m[&Folder::Drafts], "[Gmail]/Drafts");
        assert_eq!(m[&Folder::Archive], "[Gmail]/All Mail");
        assert_eq!(m[&Folder::Trash], "Deleted Items");
    }
}
