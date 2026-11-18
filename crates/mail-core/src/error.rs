use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

/// Foydalanuvchiga ko'rsatiladigan xatolar (UI inglizcha, shuning uchun matnlar ham inglizcha).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Wrong username or password")]
    Auth(String),
    #[error("Can't connect to {0}")]
    Connect(String),
    #[error("Secure connection failed: {0}")]
    Tls(String),
    #[error("Mail server error: {0}")]
    Imap(String),
    #[error("Sending failed: {0}")]
    Smtp(String),
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),
    #[error("Message not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// UI uchun qisqa kod
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Auth(_) => "auth",
            Error::Connect(_) => "connect",
            Error::Tls(_) => "tls",
            Error::Imap(_) => "imap",
            Error::Smtp(_) => "smtp",
            Error::InvalidAddress(_) => "address",
            Error::NotFound => "not_found",
            Error::Other(_) => "other",
        }
    }

    /// Ulanish uzilgan bo'lsa — qayta ulanib urinib ko'rish mumkin
    pub(crate) fn is_connection_lost(&self) -> bool {
        matches!(self, Error::Connect(_) | Error::Tls(_))
    }
}

impl From<async_imap::error::Error> for Error {
    fn from(e: async_imap::error::Error) -> Self {
        use async_imap::error::Error as E;
        match e {
            E::Io(io) => Error::Connect(io.to_string()),
            E::ConnectionLost => Error::Connect("connection lost".into()),
            other => Error::Imap(other.to_string()),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorPayload {
    pub kind: &'static str,
    pub message: String,
}

impl From<&Error> for ErrorPayload {
    fn from(e: &Error) -> Self {
        ErrorPayload { kind: e.kind(), message: e.to_string() }
    }
}
