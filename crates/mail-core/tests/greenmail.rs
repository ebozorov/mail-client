//! Haqiqiy IMAP/SMTP serverga qarshi to'liq oqim testi.
//!
//! Ishga tushirish:
//!   docker run -d --rm --name mc-greenmail -p 3025:3025 -p 3143:3143 \
//!     -e GREENMAIL_OPTS='-Dgreenmail.setup.test.all -Dgreenmail.hostname=0.0.0.0 \
//!       -Dgreenmail.users=me:secret@example.org,bob:secret@example.org -Dgreenmail.users.login=email' \
//!     greenmail/standalone:2.1.2
//!   GREENMAIL=1 cargo test --test greenmail -- --nocapture

use std::time::Duration;

use mail_core::imap::ImapClient;
use mail_core::smtp::{self, Outgoing};
use mail_core::{Account, Error, Folder, Security, Server};

fn account(user: &str) -> Account {
    Account {
        email: format!("{user}@example.org"),
        name: user.to_string(),
        username: format!("{user}@example.org"),
        imap: Server::new("127.0.0.1", 3143, Security::Plain),
        smtp: Server::new("127.0.0.1", 3025, Security::Plain),
        save_sent_copy: true,
    }
}

fn enabled() -> bool {
    if std::env::var("GREENMAIL").is_err() {
        eprintln!("GREENMAIL o'rnatilmagan — test o'tkazib yuborildi");
        return false;
    }
    true
}

async fn wait_for(imap: &ImapClient, folder: Folder, subject: &str) -> mail_core::message::Summary {
    for _ in 0..50 {
        let list = imap.list(folder, 50).await.unwrap();
        if let Some(m) = list.into_iter().find(|m| m.subject == subject) {
            return m;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{subject:?} {folder:?} papkasida paydo bo'lmadi");
}

#[tokio::test]
async fn wrong_password_is_auth_error() {
    if !enabled() {
        return;
    }
    let imap = ImapClient::new(account("me"), "wrong".into());
    let err = imap.check().await.unwrap_err();
    assert!(matches!(err, Error::Auth(_)), "got {err:?}");
}

#[tokio::test]
async fn full_flow() {
    if !enabled() {
        return;
    }
    let me = account("me");
    let bob = account("bob");
    let imap = ImapClient::new(me.clone(), "secret".into());
    let bob_imap = ImapClient::new(bob.clone(), "secret".into());

    imap.check().await.expect("login");
    smtp::test(&me, "secret").await.expect("smtp login");

    // 1. Yuborish: me → bob, nusxasi me'ning Sent papkasiga
    let subject = format!("Salom dünyo {}", std::process::id());
    let out = Outgoing {
        to: vec!["Bob <bob@example.org>".into()],
        subject: subject.clone(),
        body: "Birinchi qator\nIkkinchi qator".into(),
        ..Default::default()
    };
    let built = smtp::build(&me, &out, false).unwrap();
    smtp::send(&me, "secret", &built).await.expect("send");
    let sent_uid = imap.append(Folder::Sent, &built.raw, "(\\Seen)", &built.message_id).await.unwrap();
    assert!(sent_uid.is_some(), "Sent nusxasining UID'si topilishi kerak");

    // 2. Bob qabul qiladi: o'qilmagan, sarlavhalar to'g'ri
    let s = wait_for(&bob_imap, Folder::Inbox, &subject).await;
    assert!(!s.seen);
    assert_eq!(s.from.as_ref().unwrap().email, "me@example.org");
    assert_eq!(s.to[0].email, "bob@example.org");

    // 3. Ochish: matn keladi va o'qilgan bo'lib qoladi
    let full = bob_imap.get(Folder::Inbox, s.uid).await.unwrap();
    assert_eq!(full.text.trim(), "Birinchi qator\nIkkinchi qator");
    assert!(full.html.is_none());
    let again = wait_for(&bob_imap, Folder::Inbox, &subject).await;
    assert!(again.seen);
    bob_imap.set_seen(Folder::Inbox, s.uid, false).await.unwrap();
    assert!(!wait_for(&bob_imap, Folder::Inbox, &subject).await.seen);

    // 4. Javob: In-Reply-To zanjiri saqlanadi
    let reply = Outgoing {
        to: vec!["me@example.org".into()],
        subject: format!("Re: {subject}"),
        body: "Javob".into(),
        in_reply_to: full.summary.message_id.clone(),
        ..Default::default()
    };
    let built = smtp::build(&bob, &reply, false).unwrap();
    smtp::send(&bob, "secret", &built).await.unwrap();
    let r = wait_for(&imap, Folder::Inbox, &format!("Re: {subject}")).await;

    // 5. Arxivlash (Archive papkasi yo'q — avtomatik yaratiladi)
    imap.move_to(Folder::Inbox, r.uid, Folder::Archive).await.unwrap();
    assert!(imap.list(Folder::Inbox, 50).await.unwrap().iter().all(|m| m.uid != r.uid));
    let archived = wait_for(&imap, Folder::Archive, &format!("Re: {subject}")).await;

    // 6. O'chirish: avval Trash'ga, Trash'dan butunlay
    imap.delete(Folder::Archive, archived.uid).await.unwrap();
    let trashed = wait_for(&imap, Folder::Trash, &format!("Re: {subject}")).await;
    imap.delete(Folder::Trash, trashed.uid).await.unwrap();
    assert!(imap.list(Folder::Trash, 50).await.unwrap().iter().all(|m| m.subject != format!("Re: {subject}")));

    // 7. Qoralama: qabul qiluvchisiz saqlanadi, keyin almashtiriladi
    let draft = Outgoing { subject: "Qoralama".into(), body: "yarim".into(), ..Default::default() };
    let d = smtp::build(&me, &draft, true).unwrap();
    let uid = imap.append(Folder::Drafts, &d.raw, "(\\Draft \\Seen)", &d.message_id).await.unwrap().unwrap();
    let loaded = imap.get(Folder::Drafts, uid).await.unwrap();
    assert_eq!(loaded.text.trim(), "yarim");
    imap.expunge_uid(Folder::Drafts, uid).await.unwrap();
    assert!(imap.list(Folder::Drafts, 50).await.unwrap().iter().all(|m| m.uid != uid));

    imap.logout().await;
    bob_imap.logout().await;
}
