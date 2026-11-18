//! Ilova servisi: akkauntni saqlash/yuklash va UI amallari.
//! Tauri qatlami faqat shu metodlarni chaqiradi.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::RwLock;

use crate::config::{Account, Folder, Server};
use crate::discover::{self, Hint};
use crate::imap::ImapClient;
use crate::message::{FullMessage, Summary};
use crate::secrets::SecretStore;
use crate::smtp::{self, Outgoing};
use crate::{Error, Result};

const ACCOUNT_FILE: &str = "account.json";
pub const PAGE_SIZE: u32 = 50;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub email: String,
    #[serde(default)]
    pub name: String,
    pub password: String,
    /// Berilmasa — avtomatik aniqlanadi
    #[serde(default)]
    pub manual: Option<Manual>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manual {
    pub username: String,
    pub imap: Server,
    pub smtp: Server,
    #[serde(default = "yes")]
    pub save_sent_copy: bool,
}

fn yes() -> bool {
    true
}

struct Active {
    client: Arc<ImapClient>,
    password: String,
}

pub struct Service {
    dir: PathBuf,
    secrets: Arc<dyn SecretStore>,
    active: RwLock<Option<Arc<Active>>>,
}

impl Service {
    /// `dir` — sozlamalar papkasi (masalan ~/.config/dev.laboratory.mailclient)
    pub fn new(dir: impl Into<PathBuf>, secrets: Arc<dyn SecretStore>) -> Self {
        Service { dir: dir.into(), secrets, active: RwLock::new(None) }
    }

    fn account_path(&self) -> PathBuf {
        self.dir.join(ACCOUNT_FILE)
    }

    async fn load_account(&self) -> Result<Option<Account>> {
        match tokio::fs::read(self.account_path()).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::Other(format!("Corrupted account file: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Other(e.to_string())),
        }
    }

    /// Keyring D-Bus/OS chaqiruvlari bloklovchi — alohida thread'da
    async fn secret<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn SecretStore) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let store = self.secrets.clone();
        tokio::task::spawn_blocking(move || f(store.as_ref())).await.map_err(|e| Error::Other(e.to_string()))?
    }

    async fn active(&self) -> Result<Arc<Active>> {
        if let Some(a) = self.active.read().await.clone() {
            return Ok(a);
        }
        let mut slot = self.active.write().await;
        if let Some(a) = slot.clone() {
            return Ok(a);
        }
        let account = self.load_account().await?.ok_or_else(|| Error::Other("No account configured".into()))?;
        let email = account.email.clone();
        let password = self
            .secret(move |s| s.get(&email))
            .await?
            .ok_or_else(|| Error::Auth("password is missing from the system keyring".into()))?;
        let a = Arc::new(Active { client: Arc::new(ImapClient::new(account, password.clone())), password });
        *slot = Some(a.clone());
        Ok(a)
    }

    pub async fn account(&self) -> Result<Option<Account>> {
        self.load_account().await
    }

    pub async fn discover(&self, email: &str) -> Result<discover::Discovery> {
        discover::discover(email).await
    }

    /// Sozlamalarni aniqlaydi, IMAP va SMTP login'ni tekshiradi, keyin saqlaydi
    pub async fn add_account(&self, input: NewAccount) -> Result<Account> {
        let email = input.email.trim().to_string();
        discover::split_email(&email)?;

        let account = match input.manual {
            Some(m) => Account {
                email: email.clone(),
                name: input.name.trim().to_string(),
                username: if m.username.trim().is_empty() { email.clone() } else { m.username.trim().to_string() },
                imap: m.imap,
                smtp: m.smtp,
                save_sent_copy: m.save_sent_copy,
            },
            None => {
                let d = discover::discover(&email).await?;
                if let Some(Hint::OAuthOnly { provider }) = &d.hint {
                    return Err(Error::Other(format!(
                        "{provider} accounts require OAuth sign-in, which is not supported yet"
                    )));
                }
                Account {
                    email: email.clone(),
                    name: input.name.trim().to_string(),
                    username: d.username,
                    imap: d.imap,
                    smtp: d.smtp,
                    save_sent_copy: d.save_sent_copy,
                }
            }
        };

        let client = ImapClient::new(account.clone(), input.password.clone());
        let (imap_res, smtp_res) = tokio::join!(client.check(), smtp::test(&account, &input.password));
        imap_res?;
        smtp_res?;

        tokio::fs::create_dir_all(&self.dir).await.map_err(|e| Error::Other(e.to_string()))?;
        let json = serde_json::to_vec_pretty(&account).map_err(|e| Error::Other(e.to_string()))?;
        tokio::fs::write(self.account_path(), json).await.map_err(|e| Error::Other(e.to_string()))?;
        let (e, p) = (email.clone(), input.password.clone());
        self.secret(move |s| s.set(&e, &p)).await?;

        let old =
            self.active.write().await.replace(Arc::new(Active { client: Arc::new(client), password: input.password }));
        if let Some(old) = old {
            old.client.logout().await;
        }
        Ok(account)
    }

    pub async fn remove_account(&self) -> Result<()> {
        if let Some(a) = self.active.write().await.take() {
            a.client.logout().await;
        }
        if let Some(acct) = self.load_account().await? {
            let email = acct.email;
            self.secret(move |s| s.delete(&email)).await?;
        }
        match tokio::fs::remove_file(self.account_path()).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Other(e.to_string())),
        }
    }

    pub async fn list(&self, folder: Folder) -> Result<Vec<Summary>> {
        self.active().await?.client.list(folder, PAGE_SIZE).await
    }

    pub async fn get(&self, folder: Folder, uid: u32) -> Result<FullMessage> {
        self.active().await?.client.get(folder, uid).await
    }

    pub async fn set_seen(&self, folder: Folder, uid: u32, seen: bool) -> Result<()> {
        self.active().await?.client.set_seen(folder, uid, seen).await
    }

    pub async fn move_to(&self, folder: Folder, uid: u32, target: Folder) -> Result<()> {
        self.active().await?.client.move_to(folder, uid, target).await
    }

    pub async fn delete(&self, folder: Folder, uid: u32) -> Result<()> {
        self.active().await?.client.delete(folder, uid).await
    }

    /// Yuboradi; kerak bo'lsa Sent'ga nusxa qo'yadi va eski qoralamani o'chiradi
    pub async fn send(&self, out: Outgoing, draft_uid: Option<u32>) -> Result<()> {
        let a = self.active().await?;
        let account = a.client.account();
        let built = smtp::build(account, &out, false)?;
        smtp::send(account, &a.password, &built).await?;

        // Xat allaqachon ketgan — quyidagi xatolar yuborishni muvaffaqiyatsiz qilmasligi kerak
        if account.save_sent_copy {
            if let Err(e) = a.client.append(Folder::Sent, &built.raw, "(\\Seen)", &built.message_id).await {
                eprintln!("could not save a copy to Sent: {e}");
            }
        }
        if let Some(uid) = draft_uid {
            if let Err(e) = a.client.expunge_uid(Folder::Drafts, uid).await {
                eprintln!("could not remove draft {uid}: {e}");
            }
        }
        Ok(())
    }

    /// Qoralamani serverdagi Drafts'ga saqlaydi (eski versiyani almashtiradi). Yangi UID.
    pub async fn save_draft(&self, out: Outgoing, draft_uid: Option<u32>) -> Result<Option<u32>> {
        let a = self.active().await?;
        let built = smtp::build(a.client.account(), &out, true)?;
        let uid = a.client.append(Folder::Drafts, &built.raw, "(\\Draft \\Seen)", &built.message_id).await?;
        if let Some(old) = draft_uid {
            a.client.expunge_uid(Folder::Drafts, old).await?;
        }
        Ok(uid)
    }
}
