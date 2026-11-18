//! Xat yasash va SMTP orqali yuborish (lettre).

use std::time::Duration;

use lettre::address::Envelope;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::Deserialize;

use crate::config::{Account, Security};
use crate::{Error, Result};

/// UI'dan keladigan yangi xat
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    /// Javob berilayotgan xatning Message-ID'si (zanjir uchun)
    #[serde(default)]
    pub in_reply_to: Option<String>,
}

/// Tayyor xat: SMTP uchun obyekt va Sent/Drafts'ga APPEND uchun xom baytlar
pub struct Built {
    pub message: Message,
    pub raw: Vec<u8>,
    pub message_id: String,
}

fn mailbox(s: &str) -> Result<Mailbox> {
    s.trim().parse::<Mailbox>().map_err(|_| Error::InvalidAddress(s.trim().to_string()))
}

fn new_message_id(account: &Account) -> String {
    let domain = account.email.rsplit_once('@').map(|(_, d)| d).unwrap_or("localhost");
    let rand: u64 = rand_u64();
    format!("{}.{:x}@{}", chrono::Utc::now().timestamp_millis(), rand, domain)
}

fn rand_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    h.finish()
}

/// `draft = true` bo'lsa qabul qiluvchisiz ham yasaladi (qoralama uchun).
pub fn build(account: &Account, out: &Outgoing, draft: bool) -> Result<Built> {
    let from = Mailbox::new(
        Some(account.name.clone()).filter(|n| !n.trim().is_empty()),
        account.email.parse().map_err(|_| Error::InvalidAddress(account.email.clone()))?,
    );
    let to: Vec<Mailbox> = out.to.iter().filter(|s| !s.trim().is_empty()).map(|s| mailbox(s)).collect::<Result<_>>()?;
    let cc: Vec<Mailbox> = out.cc.iter().filter(|s| !s.trim().is_empty()).map(|s| mailbox(s)).collect::<Result<_>>()?;
    if !draft && to.is_empty() && cc.is_empty() {
        return Err(Error::InvalidAddress("no recipients".into()));
    }

    let message_id = new_message_id(account);
    let mut b = Message::builder()
        .from(from.clone())
        .subject(out.subject.clone())
        .message_id(Some(format!("<{message_id}>")))
        .date_now();
    for m in &to {
        b = b.to(m.clone());
    }
    for m in &cc {
        b = b.cc(m.clone());
    }
    if let Some(id) = out.in_reply_to.as_deref().filter(|s| !s.is_empty()) {
        let id = format!("<{}>", id.trim_matches(['<', '>']));
        b = b.in_reply_to(id.clone()).references(id);
    }
    if draft && to.is_empty() && cc.is_empty() {
        // Qabul qiluvchi yo'q qoralama: lettre envelope talab qiladi, shuning uchun o'zimizni qo'yamiz
        let me = from.email.clone();
        b = b.envelope(Envelope::new(Some(me.clone()), vec![me]).map_err(|e| Error::Other(e.to_string()))?);
    }

    let message = b.header(ContentType::TEXT_PLAIN).body(out.body.clone()).map_err(|e| Error::Other(e.to_string()))?;
    let raw = message.formatted();
    Ok(Built { message, raw, message_id })
}

fn transport(account: &Account, password: &str) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let s = &account.smtp;
    let builder = match s.security {
        Security::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host),
        Security::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host),
        Security::Plain => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&s.host)),
    }
    .map_err(|e| Error::Tls(e.to_string()))?;
    Ok(builder
        .port(s.port)
        .credentials(Credentials::new(account.username.clone(), password.to_string()))
        .timeout(Some(Duration::from_secs(30)))
        .build())
}

fn map_err(e: lettre::transport::smtp::Error, host: &str) -> Error {
    if let Some(code) = e.status() {
        // 535: autentifikatsiya rad etildi, 534: qo'shimcha xavfsizlik talab qilinadi (App Password)
        if code.to_string().starts_with("535") || code.to_string().starts_with("534") {
            return Error::Auth(e.to_string());
        }
        return Error::Smtp(e.to_string());
    }
    if e.is_transient() || e.is_permanent() {
        return Error::Smtp(e.to_string());
    }
    let msg = e.to_string();
    if msg.to_ascii_lowercase().contains("tls") || msg.to_ascii_lowercase().contains("certificate") {
        Error::Tls(msg)
    } else {
        Error::Connect(format!("{host}: {msg}"))
    }
}

/// Ulanish va login'ni tekshiradi (akkaunt qo'shishda)
pub async fn test(account: &Account, password: &str) -> Result<()> {
    let t = transport(account, password)?;
    match t.test_connection().await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Connect(account.smtp.host.clone())),
        Err(e) => Err(map_err(e, &account.smtp.host)),
    }
}

pub async fn send(account: &Account, password: &str, built: &Built) -> Result<()> {
    let t = transport(account, password)?;
    t.send(built.message.clone()).await.map(|_| ()).map_err(|e| map_err(e, &account.smtp.host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Server;

    fn acct() -> Account {
        Account {
            email: "me@example.org".into(),
            name: "Me".into(),
            username: "me@example.org".into(),
            imap: Server::new("imap.example.org", 993, Security::Tls),
            smtp: Server::new("smtp.example.org", 465, Security::Tls),
            save_sent_copy: true,
        }
    }

    #[test]
    fn builds_reply() {
        let out = Outgoing {
            to: vec!["Bob <bob@example.org>".into()],
            subject: "Re: Salom".into(),
            body: "Javob matni".into(),
            in_reply_to: Some("abc@example.org".into()),
            ..Default::default()
        };
        let b = build(&acct(), &out, false).unwrap();
        let raw = String::from_utf8(b.raw).unwrap();
        assert!(raw.contains("To: Bob <bob@example.org>"));
        assert!(raw.contains("In-Reply-To: <abc@example.org>"));
        assert!(raw.contains(&format!("Message-ID: <{}>", b.message_id)));
    }

    #[test]
    fn requires_recipient_unless_draft() {
        let out = Outgoing { subject: "x".into(), ..Default::default() };
        assert!(build(&acct(), &out, false).is_err());
        assert!(build(&acct(), &out, true).is_ok());
    }

    #[test]
    fn rejects_bad_address() {
        let out = Outgoing { to: vec!["not an address".into()], ..Default::default() };
        assert!(matches!(build(&acct(), &out, false), Err(Error::InvalidAddress(_))));
    }
}
