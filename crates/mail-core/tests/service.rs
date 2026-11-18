//! Service: akkaunt hayot sikli va qoralama → yuborish oqimi (GreenMail kerak, greenmail.rs ga qarang).

use std::sync::Arc;
use std::time::Duration;

use mail_core::app::{Manual, NewAccount, Service};
use mail_core::secrets::{Keyring, MemoryStore, SecretStore};
use mail_core::smtp::Outgoing;
use mail_core::{Error, Folder, Security, Server};

fn manual() -> Manual {
    Manual {
        username: String::new(),
        imap: Server::new("127.0.0.1", 3143, Security::Plain),
        smtp: Server::new("127.0.0.1", 3025, Security::Plain),
        save_sent_copy: true,
    }
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mail-core-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[tokio::test]
async fn account_lifecycle_and_draft_send() {
    if std::env::var("GREENMAIL").is_err() {
        return;
    }
    let dir = tmpdir("svc");
    let store: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
    let svc = Service::new(&dir, store.clone());
    assert!(svc.account().await.unwrap().is_none());
    assert!(svc.list(Folder::Inbox).await.is_err());

    // Noto'g'ri parol — hech narsa saqlanmaydi
    let bad = NewAccount {
        email: "me@example.org".into(),
        name: "Me".into(),
        password: "nope".into(),
        manual: Some(manual()),
    };
    assert!(matches!(svc.add_account(bad).await, Err(Error::Auth(_))));
    assert!(svc.account().await.unwrap().is_none());

    let ok = NewAccount {
        email: "me@example.org".into(),
        name: "Me".into(),
        password: "secret".into(),
        manual: Some(manual()),
    };
    let acct = svc.add_account(ok).await.unwrap();
    assert_eq!(acct.username, "me@example.org");
    assert_eq!(store.get("me@example.org").unwrap().as_deref(), Some("secret"));
    let saved = std::fs::read_to_string(dir.join("account.json")).unwrap();
    assert!(!saved.contains("secret"), "parol faylga yozilmasligi kerak");

    // Ilova qayta ishga tushdi: fayl + keyring'dan tiklanadi
    let svc = Service::new(&dir, store.clone());
    let subject = format!("Draft flow {}", std::process::id());
    let mut out = Outgoing { subject: subject.clone(), body: "v1".into(), ..Default::default() };
    let uid1 = svc.save_draft(out.clone(), None).await.unwrap();
    out.body = "v2".into();
    out.to = vec!["bob@example.org".into()];
    let uid2 = svc.save_draft(out.clone(), uid1).await.unwrap();
    let drafts = svc.list(Folder::Drafts).await.unwrap();
    let ours: Vec<_> = drafts.iter().filter(|m| m.subject == subject).collect();
    assert_eq!(ours.len(), 1, "eski versiya almashtirilishi kerak");
    assert_eq!(Some(ours[0].uid), uid2);

    svc.send(out, uid2).await.unwrap();
    assert!(svc.list(Folder::Drafts).await.unwrap().iter().all(|m| m.subject != subject));
    let mut in_sent = false;
    for _ in 0..30 {
        if svc.list(Folder::Sent).await.unwrap().iter().any(|m| m.subject == subject) {
            in_sent = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(in_sent, "Sent'da nusxa bo'lishi kerak");

    svc.remove_account().await.unwrap();
    assert!(svc.account().await.unwrap().is_none());
    assert!(store.get("me@example.org").unwrap().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Haqiqiy OS keyring (GNOME Keyring). Ishga tushirish: KEYRING=1 cargo test --test service
#[tokio::test]
async fn system_keyring_roundtrip() {
    if std::env::var("KEYRING").is_err() {
        return;
    }
    // Ilovadagi kabi: keyring chaqiruvlari spawn_blocking ichida (zbus o'z runtime'ini ishlatadi)
    tokio::task::spawn_blocking(|| {
        let k = Keyring::new("dev.laboratory.mailclient.test");
        let user = "roundtrip@example.org";
        k.set(user, "p@ss w0rd ✓").unwrap();
        assert_eq!(k.get(user).unwrap().as_deref(), Some("p@ss w0rd ✓"));
        k.delete(user).unwrap();
        assert!(k.get(user).unwrap().is_none());
    })
    .await
    .unwrap();
}
