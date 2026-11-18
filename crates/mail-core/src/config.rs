use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// Boshidan TLS (993 / 465)
    Tls,
    /// Oddiy ulanib, keyin STARTTLS (143 / 587)
    StartTls,
    /// Shifrlanmagan — faqat lokal test serverlar uchun
    Plain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
}

impl Server {
    pub fn new(host: impl Into<String>, port: u16, security: Security) -> Self {
        Server { host: host.into(), port, security }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub email: String,
    #[serde(default)]
    pub name: String,
    /// Ko'pincha email bilan bir xil
    pub username: String,
    pub imap: Server,
    pub smtp: Server,
    /// Gmail/Outlook yuborilgan xatni Sent'ga o'zi saqlaydi; boshqalarda biz APPEND qilamiz
    #[serde(default = "yes")]
    pub save_sent_copy: bool,
}

fn yes() -> bool {
    true
}

/// Ilovadagi papka rollari. Serverdagi haqiqiy nomlar har xil bo'ladi
/// ("Sent", "Sent Items", "[Gmail]/Sent Mail"...), ular IMAP LIST orqali aniqlanadi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Folder {
    Inbox,
    Drafts,
    Sent,
    Archive,
    Trash,
}

impl Folder {
    /// Papka serverda bo'lmasa, shu nom bilan yaratiladi
    pub fn default_name(self) -> &'static str {
        match self {
            Folder::Inbox => "INBOX",
            Folder::Drafts => "Drafts",
            Folder::Sent => "Sent",
            Folder::Archive => "Archive",
            Folder::Trash => "Trash",
        }
    }
}
