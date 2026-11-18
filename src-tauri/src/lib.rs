//! Tauri qatlami: UI'dan keladigan chaqiruvlarni mail-core servisiga uzatadi.
//! Barcha mantiq crates/mail-core ichida (u yerda testlar ham bor).

use std::sync::Arc;

use mail_core::app::{NewAccount, Service};
use mail_core::discover::Discovery;
use mail_core::error::ErrorPayload;
use mail_core::message::{FullMessage, Summary};
use mail_core::secrets::Keyring;
use mail_core::smtp::Outgoing;
use mail_core::{Account, Folder};
use tauri::{Manager, State};

type Res<T> = Result<T, ErrorPayload>;

fn err(e: mail_core::Error) -> ErrorPayload {
    ErrorPayload::from(&e)
}

#[tauri::command]
async fn get_account(svc: State<'_, Service>) -> Res<Option<Account>> {
    svc.account().await.map_err(err)
}

#[tauri::command]
async fn discover(svc: State<'_, Service>, email: String) -> Res<Discovery> {
    svc.discover(&email).await.map_err(err)
}

#[tauri::command]
async fn add_account(svc: State<'_, Service>, input: NewAccount) -> Res<Account> {
    svc.add_account(input).await.map_err(err)
}

#[tauri::command]
async fn remove_account(svc: State<'_, Service>) -> Res<()> {
    svc.remove_account().await.map_err(err)
}

#[tauri::command]
async fn list_messages(svc: State<'_, Service>, folder: Folder) -> Res<Vec<Summary>> {
    svc.list(folder).await.map_err(err)
}

#[tauri::command]
async fn get_message(svc: State<'_, Service>, folder: Folder, uid: u32) -> Res<FullMessage> {
    svc.get(folder, uid).await.map_err(err)
}

#[tauri::command]
async fn set_seen(svc: State<'_, Service>, folder: Folder, uid: u32, seen: bool) -> Res<()> {
    svc.set_seen(folder, uid, seen).await.map_err(err)
}

#[tauri::command]
async fn move_message(svc: State<'_, Service>, folder: Folder, uid: u32, target: Folder) -> Res<()> {
    svc.move_to(folder, uid, target).await.map_err(err)
}

#[tauri::command]
async fn delete_message(svc: State<'_, Service>, folder: Folder, uid: u32) -> Res<()> {
    svc.delete(folder, uid).await.map_err(err)
}

#[tauri::command]
async fn send_message(svc: State<'_, Service>, mail: Outgoing, draft_uid: Option<u32>) -> Res<()> {
    svc.send(mail, draft_uid).await.map_err(err)
}

#[tauri::command]
async fn save_draft(svc: State<'_, Service>, mail: Outgoing, draft_uid: Option<u32>) -> Res<Option<u32>> {
    svc.save_draft(mail, draft_uid).await.map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let keyring = Keyring::new(app.config().identifier.clone());
            app.manage(Service::new(dir, Arc::new(keyring)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_account,
            discover,
            add_account,
            remove_account,
            list_messages,
            get_message,
            set_seen,
            move_message,
            delete_message,
            send_message,
            save_draft,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
