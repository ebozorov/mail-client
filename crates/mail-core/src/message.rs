//! Xom RFC 5322 xatni UI uchun tuzilmaga aylantirish (mail-parser yordamida).

use mail_parser::{Address, MessageParser, MimeHeaders, PartType};
use serde::Serialize;

use crate::config::Folder;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Addr {
    pub name: Option<String>,
    pub email: String,
}

/// Ro'yxatda ko'rinadigan qisqa ma'lumot (faqat sarlavhalardan)
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub uid: u32,
    pub folder: Folder,
    pub from: Option<Addr>,
    pub to: Vec<Addr>,
    pub subject: String,
    /// RFC 3339
    pub date: String,
    pub seen: bool,
    pub message_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub size: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullMessage {
    #[serde(flatten)]
    pub summary: Summary,
    pub cc: Vec<Addr>,
    /// Oddiy matn (HTML'dan ham yasaladi)
    pub text: String,
    /// Xatda haqiqiy HTML qism bo'lsa
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
}

fn addrs(a: Option<&Address>) -> Vec<Addr> {
    a.map(|a| {
        a.iter()
            .filter_map(|x| {
                Some(Addr {
                    name: x.name().map(str::to_string).filter(|n| !n.is_empty()),
                    email: x.address()?.to_string(),
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Sarlavhalardan Summary yasaydi. `fallback_date` — serverdagi INTERNALDATE.
pub fn summary(raw: &[u8], uid: u32, folder: Folder, seen: bool, fallback_date: Option<String>) -> Summary {
    let parsed = MessageParser::default().parse_headers(raw);
    let Some(m) = parsed else {
        return Summary {
            uid,
            folder,
            from: None,
            to: vec![],
            subject: String::new(),
            date: fallback_date.unwrap_or_default(),
            seen,
            message_id: None,
        };
    };
    Summary {
        uid,
        folder,
        from: addrs(m.from()).into_iter().next(),
        to: addrs(m.to()),
        subject: m.subject().unwrap_or_default().to_string(),
        date: m.date().map(|d| d.to_rfc3339()).or(fallback_date).unwrap_or_default(),
        seen,
        message_id: m.message_id().map(str::to_string),
    }
}

pub fn full(raw: &[u8], uid: u32, folder: Folder, seen: bool, fallback_date: Option<String>) -> FullMessage {
    let summary_part = summary(raw, uid, folder, seen, fallback_date);
    let Some(m) = MessageParser::default().parse(raw) else {
        return FullMessage {
            summary: summary_part,
            cc: vec![],
            text: String::from_utf8_lossy(raw).replace("\r\n", "\n"),
            html: None,
            attachments: vec![],
        };
    };

    let has_html = m.parts.iter().any(|p| matches!(p.body, PartType::Html(_)));
    FullMessage {
        cc: addrs(m.cc()),
        text: m.body_text(0).map(|t| t.replace("\r\n", "\n")).unwrap_or_default(),
        html: if has_html { m.body_html(0).map(|h| h.into_owned()) } else { None },
        attachments: m
            .attachments()
            .map(|p| Attachment { name: p.attachment_name().unwrap_or("attachment").to_string(), size: p.len() })
            .collect(),
        summary: summary_part,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &[u8] = b"From: =?UTF-8?B?QWxpIFZhbGl5ZXY=?= <ali@example.org>\r\n\
To: Bob <bob@example.org>, carol@example.org\r\n\
Subject: =?UTF-8?Q?Salom_d=C3=BCnyo?=\r\n\
Date: Tue, 17 Nov 2026 10:00:00 +0500\r\n\
Message-ID: <abc@example.org>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"b\"\r\n\
\r\n\
--b\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hello plain\r\n\
--b\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p>Hello <b>html</b></p>\r\n\
--b--\r\n";

    #[test]
    fn decodes_headers() {
        let s = summary(RAW, 7, Folder::Inbox, false, None);
        assert_eq!(s.from, Some(Addr { name: Some("Ali Valiyev".into()), email: "ali@example.org".into() }));
        assert_eq!(s.to.len(), 2);
        assert_eq!(s.to[1].email, "carol@example.org");
        assert_eq!(s.subject, "Salom dünyo");
        assert_eq!(s.date, "2026-11-17T10:00:00+05:00");
        assert_eq!(s.message_id.as_deref(), Some("abc@example.org"));
    }

    #[test]
    fn extracts_bodies() {
        let f = full(RAW, 7, Folder::Inbox, true, None);
        assert_eq!(f.text.trim(), "Hello plain");
        assert!(f.html.unwrap().contains("<b>html</b>"));
    }

    #[test]
    fn plain_only_has_no_html() {
        let raw = b"From: a@b.c\r\nSubject: x\r\n\r\njust text\r\n";
        let f = full(raw, 1, Folder::Inbox, true, Some("2026-01-01T00:00:00+00:00".into()));
        assert!(f.html.is_none());
        assert_eq!(f.text.trim(), "just text");
        assert_eq!(f.summary.date, "2026-01-01T00:00:00+00:00");
    }
}
