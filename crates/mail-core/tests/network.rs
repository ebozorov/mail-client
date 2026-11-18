//! Internetdagi haqiqiy serverlar: TLS va avtomatik sozlash.
//! Ishga tushirish: NETWORK=1 cargo test --test network -- --nocapture

use mail_core::discover::{discover, Source};
use mail_core::imap::ImapClient;
use mail_core::{Account, Error, Security, Server};

fn enabled() -> bool {
    std::env::var("NETWORK").is_ok()
}

/// Mozilla ISPDB'da bor, lekin bizning jadvalda yo'q provayder
#[tokio::test]
async fn discovers_via_ispdb() {
    if !enabled() {
        return;
    }
    let d = discover("someone@gmx.com").await.unwrap();
    assert_eq!(d.source, Source::Ispdb);
    assert_eq!(d.imap.security, Security::Tls);
    assert!(d.imap.host.contains("gmx"), "{d:?}");
}

/// ISPDB'da yo'q — domenning o'z autoconfig.<domen> fayli ishlatiladi
#[tokio::test]
async fn discovers_via_domain_autoconfig() {
    if !enabled() {
        return;
    }
    let d = discover("someone@fastmail.com").await.unwrap();
    assert_eq!(d.source, Source::Autoconfig);
    assert_eq!(d.imap.host, "imap.fastmail.com");
}

/// TLS (993) + OS sertifikatlari ishlaydi; noto'g'ri parol "auth" xatosi bo'lib qaytadi
#[tokio::test]
async fn tls_login_rejected_as_auth() {
    if !enabled() {
        return;
    }
    let acct = Account {
        email: "nobody.mail.client.test@gmail.com".into(),
        name: String::new(),
        username: "nobody.mail.client.test@gmail.com".into(),
        imap: Server::new("imap.gmail.com", 993, Security::Tls),
        smtp: Server::new("smtp.gmail.com", 465, Security::Tls),
        save_sent_copy: false,
    };
    let err = ImapClient::new(acct, "wrong-password".into()).check().await.unwrap_err();
    eprintln!("{err:?}");
    assert!(matches!(err, Error::Auth(_)), "got {err:?}");
}
