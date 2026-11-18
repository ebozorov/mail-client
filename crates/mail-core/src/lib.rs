//! Pochta mantig'i: avtomatik sozlash, IMAP (o'qish) va SMTP (yuborish).
//! Tauri'ga bog'liq emas — mustaqil test qilinadi.

pub mod config;
pub mod discover;
pub mod error;
pub mod imap;
pub mod message;
pub mod smtp;
mod tls;

pub use config::{Account, Folder, Security, Server};
pub use error::{Error, Result};
