//! Email manzilidan IMAP/SMTP sozlamalarini topish.
//! Tartib: ma'lum provayderlar → Mozilla ISPDB → domenning o'z autoconfig'i → taxmin.

use std::time::Duration;

use serde::Serialize;

use crate::config::{Security, Server};
use crate::{Error, Result};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub imap: Server,
    pub smtp: Server,
    pub username: String,
    pub save_sent_copy: bool,
    pub source: Source,
    pub hint: Option<Hint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Known,
    Ispdb,
    Autoconfig,
    Guess,
}

/// Foydalanuvchiga ko'rsatiladigan maslahat
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Hint {
    /// Oddiy parol ishlamaydi — provayder sozlamalarida "App Password" yaratish kerak
    AppPassword { provider: String, url: String },
    /// Faqat OAuth2 bilan kiriladi (2-bosqichda qo'shiladi)
    OAuthOnly { provider: String },
}

pub fn split_email(email: &str) -> Result<(String, String)> {
    let email = email.trim();
    match email.rsplit_once('@') {
        Some((local, domain)) if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') => {
            Ok((local.to_string(), domain.to_ascii_lowercase()))
        }
        _ => Err(Error::InvalidAddress(email.to_string())),
    }
}

pub async fn discover(email: &str) -> Result<Discovery> {
    let (local, domain) = split_email(email)?;
    let email = email.trim();

    if let Some(d) = known(&domain, email) {
        return Ok(d);
    }

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("mail-client/0.1")
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;

    let sources = [
        (Source::Ispdb, format!("https://autoconfig.thunderbird.net/v1.1/{domain}")),
        (Source::Autoconfig, format!("https://autoconfig.{domain}/mail/config-v1.1.xml")),
    ];
    for (source, url) in sources {
        if let Some(xml) = fetch_text(&http, &url).await {
            if let Some((imap, smtp, username)) = parse_autoconfig(&xml, email, &local, &domain) {
                return Ok(Discovery { imap, smtp, username, save_sent_copy: true, source, hint: None });
            }
        }
    }

    Ok(Discovery {
        imap: Server::new(format!("imap.{domain}"), 993, Security::Tls),
        smtp: Server::new(format!("smtp.{domain}"), 465, Security::Tls),
        username: email.to_string(),
        save_sent_copy: true,
        source: Source::Guess,
        hint: None,
    })
}

async fn fetch_text(http: &reqwest::Client, url: &str) -> Option<String> {
    let res = http.get(url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    res.text().await.ok()
}

/// Eng ko'p ishlatiladigan provayderlar — tarmoqqa chiqmasdan aniqlanadi.
pub fn known(domain: &str, email: &str) -> Option<Discovery> {
    let app_pw = |provider: &str, url: &str| Some(Hint::AppPassword { provider: provider.into(), url: url.into() });
    let (imap, smtp, save_sent_copy, hint) = match domain {
        "gmail.com" | "googlemail.com" => (
            ("imap.gmail.com", 993, Security::Tls),
            ("smtp.gmail.com", 465, Security::Tls),
            false,
            app_pw("Google", "https://myaccount.google.com/apppasswords"),
        ),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => (
            ("outlook.office365.com", 993, Security::Tls),
            ("smtp-mail.outlook.com", 587, Security::StartTls),
            false,
            Some(Hint::OAuthOnly { provider: "Microsoft".into() }),
        ),
        "yahoo.com" | "ymail.com" => (
            ("imap.mail.yahoo.com", 993, Security::Tls),
            ("smtp.mail.yahoo.com", 465, Security::Tls),
            true,
            app_pw("Yahoo", "https://login.yahoo.com/account/security"),
        ),
        "icloud.com" | "me.com" | "mac.com" => (
            ("imap.mail.me.com", 993, Security::Tls),
            ("smtp.mail.me.com", 587, Security::StartTls),
            true,
            app_pw("Apple", "https://account.apple.com/account/manage"),
        ),
        "yandex.ru" | "yandex.com" | "ya.ru" | "yandex.uz" => (
            ("imap.yandex.com", 993, Security::Tls),
            ("smtp.yandex.com", 465, Security::Tls),
            false,
            app_pw("Yandex", "https://id.yandex.com/security/app-passwords"),
        ),
        "mail.ru" | "inbox.ru" | "list.ru" | "bk.ru" => (
            ("imap.mail.ru", 993, Security::Tls),
            ("smtp.mail.ru", 465, Security::Tls),
            true,
            app_pw("Mail.ru", "https://account.mail.ru/user/2-step-auth/passwords"),
        ),
        _ => return None,
    };
    Some(Discovery {
        imap: Server::new(imap.0, imap.1, imap.2),
        smtp: Server::new(smtp.0, smtp.1, smtp.2),
        username: email.to_string(),
        save_sent_copy,
        source: Source::Known,
        hint,
    })
}

/// Thunderbird autoconfig XML formatini o'qiydi.
/// https://wiki.mozilla.org/Thunderbird:Autoconfiguration:ConfigFileFormat
pub fn parse_autoconfig(xml: &str, email: &str, local: &str, domain: &str) -> Option<(Server, Server, String)> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let child = |n: roxmltree::Node, tag: &str| {
        n.children().find(|c| c.has_tag_name(tag)).and_then(|c| c.text()).map(|t| t.trim().to_string())
    };
    let subst = |s: String| {
        s.replace("%EMAILADDRESS%", email).replace("%EMAILLOCALPART%", local).replace("%EMAILDOMAIN%", domain)
    };

    // (server, username, xavfsizlik bo'yicha ustunlik)
    let pick = |tag: &str, kind: &str| {
        doc.descendants()
            .filter(|n| n.has_tag_name(tag) && n.attribute("type") == Some(kind))
            .filter_map(|n| {
                let host = subst(child(n, "hostname")?);
                let port = child(n, "port")?.parse::<u16>().ok()?;
                let (security, rank) = match child(n, "socketType")?.to_ascii_uppercase().as_str() {
                    "SSL" | "TLS" => (Security::Tls, 0),
                    "STARTTLS" => (Security::StartTls, 1),
                    _ => (Security::Plain, 2),
                };
                let user = child(n, "username").map(subst).unwrap_or_else(|| email.to_string());
                Some((Server::new(host, port, security), user, rank))
            })
            .min_by_key(|(_, _, rank)| *rank)
    };

    let (imap, username, _) = pick("incomingServer", "imap")?;
    let (smtp, _, _) = pick("outgoingServer", "smtp")?;
    Some((imap, smtp, username))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<clientConfig version="1.1">
  <emailProvider id="example.org">
    <domain>example.org</domain>
    <incomingServer type="pop3">
      <hostname>pop.example.org</hostname><port>995</port><socketType>SSL</socketType>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>imap.example.org</hostname><port>143</port><socketType>STARTTLS</socketType>
      <username>%EMAILLOCALPART%</username>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>mail.%EMAILDOMAIN%</hostname><port>993</port><socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.org</hostname><port>587</port><socketType>STARTTLS</socketType>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

    #[test]
    fn parses_autoconfig_preferring_tls() {
        let (imap, smtp, user) = parse_autoconfig(XML, "ali@example.org", "ali", "example.org").unwrap();
        assert_eq!(imap, Server::new("mail.example.org", 993, Security::Tls));
        assert_eq!(smtp, Server::new("smtp.example.org", 587, Security::StartTls));
        assert_eq!(user, "ali@example.org");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_autoconfig("<html>not found</html>", "a@b.c", "a", "b.c").is_none());
        assert!(parse_autoconfig("not xml", "a@b.c", "a", "b.c").is_none());
    }

    #[test]
    fn known_providers() {
        let g = known("gmail.com", "x@gmail.com").unwrap();
        assert_eq!(g.imap.host, "imap.gmail.com");
        assert!(!g.save_sent_copy);
        assert!(matches!(g.hint, Some(Hint::AppPassword { .. })));
        assert!(matches!(known("outlook.com", "x@outlook.com").unwrap().hint, Some(Hint::OAuthOnly { .. })));
        assert!(known("example.org", "x@example.org").is_none());
    }

    #[test]
    fn splits_email() {
        assert_eq!(split_email(" Ali@Example.ORG ").unwrap(), ("Ali".into(), "example.org".into()));
        assert!(split_email("no-at-sign").is_err());
        assert!(split_email("@example.org").is_err());
        assert!(split_email("a@localhost").is_err());
    }
}
