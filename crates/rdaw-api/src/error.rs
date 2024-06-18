use std::{fmt, io};

use rdaw_rpc::ProtocolError;
use serde::{Deserialize, Serialize};
use tracing_error::SpanTrace;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    Other,

    Deserialization,
    Disconnected,
    IndexOutOfBounds,
    InvalidId,
    InvalidType,
    InvalidUtf8,
    InvalidUuid,
    Io,
    NotFound,
    NotSupported,
    OutOfMemory,
    PermissionDenied,
    Serialization,
    Sql,
    UnknownVersion,
}

impl From<io::ErrorKind> for ErrorKind {
    fn from(value: io::ErrorKind) -> Self {
        match value {
            io::ErrorKind::NotFound => ErrorKind::NotFound,
            io::ErrorKind::Other => ErrorKind::Other,
            io::ErrorKind::OutOfMemory => ErrorKind::OutOfMemory,
            io::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
            _ => ErrorKind::Io,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Error {
    repr: Box<Repr>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Repr {
    cause: ErrorEntry,
    backtrace: Vec<Location>,
}

impl Error {
    #[track_caller]
    pub fn new<T: fmt::Display>(kind: ErrorKind, message: T) -> Error {
        Error {
            repr: Box::new(Repr {
                cause: ErrorEntry {
                    kind,
                    message: message.to_string(),
                    cause: None,
                },
                backtrace: capture_backtrace(),
            }),
        }
    }

    #[track_caller]
    pub fn other<T: fmt::Display>(message: T) -> Error {
        Error::new(ErrorKind::Other, message)
    }

    pub fn wrap<T: fmt::Display>(self, kind: ErrorKind, message: T) -> Error {
        Error {
            repr: Box::new(Repr {
                cause: ErrorEntry {
                    kind,
                    message: message.to_string(),
                    cause: Some(Box::new(self.repr.cause)),
                },
                backtrace: self.repr.backtrace,
            }),
        }
    }

    pub fn context<T: fmt::Display>(self, message: T) -> Error {
        let kind = self.kind();
        self.wrap(kind, message)
    }

    pub fn cause(&self) -> &ErrorEntry {
        &self.repr.cause
    }

    pub fn kind(&self) -> ErrorKind {
        self.repr.cause.kind
    }

    pub fn message(&self) -> &str {
        &self.repr.cause.message
    }

    pub fn chain(&self) -> impl Iterator<Item = &ErrorEntry> + '_ {
        let mut cur = Some(&self.repr.cause);
        std::iter::from_fn(move || {
            let ret = cur.take()?;
            cur = ret.cause.as_deref();
            Some(ret)
        })
    }

    pub fn backtrace(&self) -> impl Iterator<Item = &Location> + '_ {
        self.repr.backtrace.iter()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Error:")?;

        let indent = (self.chain().count().ilog10() as usize) + 3;

        for (i, entry) in self.chain().enumerate() {
            write!(f, "{i: >indent$}: {:?}: {}", entry.kind, entry.message)?;
        }

        writeln!(f)?;
        writeln!(f, "Backtrace:")?;

        let indent = (self.backtrace().count().ilog10() as usize) + 3;

        for (i, location) in self.backtrace().enumerate() {
            write!(f, "{i: >indent$}: ")?;

            if let Some(path) = &location.path {
                write!(f, "{path}")?;
            } else if let Some(file) = &location.file {
                write!(f, "{file}")?;
                if let Some(line) = location.line {
                    write!(f, ":{line}")?;
                }
            }

            if let Some(fields) = &location.fields {
                if !fields.is_empty() {
                    write!(f, " with {fields}")?;
                }
            }

            writeln!(f)?;

            if let Some(file) = location.file.as_ref().filter(|_| location.path.is_some()) {
                write!(f, "{:indent$}  at {file}", "")?;
                if let Some(line) = location.line {
                    write!(f, ":{line}")?;
                }
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    #[track_caller]
    fn from(value: io::Error) -> Self {
        Error::new(value.kind().into(), value)
    }
}

impl ProtocolError for Error {
    fn disconnected() -> Self {
        Error::new(ErrorKind::Disconnected, "disconnected")
    }

    fn invalid_type() -> Self {
        Error::new(ErrorKind::InvalidType, "invalid type")
    }

    fn is_disconnected(&self) -> bool {
        self.kind() == ErrorKind::Disconnected
    }

    fn is_invalid_type(&self) -> bool {
        self.kind() == ErrorKind::InvalidType
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub kind: ErrorKind,
    pub message: String,
    pub cause: Option<Box<ErrorEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub fields: Option<String>,
}

#[cold]
#[track_caller]
fn capture_backtrace() -> Vec<Location> {
    let mut backtrace = Vec::new();

    let caller = std::panic::Location::caller();

    backtrace.push(Location {
        path: None,
        file: Some(caller.file().into()),
        line: Some(caller.line()),
        fields: None,
    });

    let spantrace = SpanTrace::capture();
    spantrace.with_spans(|metadata, fields| {
        backtrace.push(Location {
            path: Some(format!(
                "{} in {}",
                metadata.name(),
                metadata.module_path().unwrap_or_else(|| metadata.target())
            )),
            file: metadata.file().map(String::from),
            line: metadata.line(),
            fields: Some(fields.into()),
        });
        true
    });

    backtrace
}

pub trait ResultExt<T, E> {
    fn convert_err(self, kind: ErrorKind) -> Result<T, Error>;

    fn convert_err_with<F: FnOnce(&E) -> ErrorKind>(self, get_kind: F) -> Result<T, Error>;
}

impl<T, E: std::error::Error> ResultExt<T, E> for Result<T, E> {
    #[track_caller]
    fn convert_err(self, kind: ErrorKind) -> Result<T, Error> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::new(kind, e)),
        }
    }

    #[track_caller]
    fn convert_err_with<F: FnOnce(&E) -> ErrorKind>(self, get_kind: F) -> Result<T, Error> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                let kind = get_kind(&e);
                Err(Error::new(kind, e))
            }
        }
    }
}

#[macro_export]
macro_rules! assert_err {
    ($res:expr, $kind:expr $(,)?) => {
        match $res {
            Ok(_) => panic!("expected error {:?}", $kind),
            Err(e) => assert_eq!(e.kind(), $kind),
        }
    };
}

#[macro_export]
macro_rules! format_err {
    ($kind:expr, $($arg:tt)*) => {
        $crate::Error::new($kind, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! bail {
    ($kind:expr, $($arg:tt)*) => {
        return Err($crate::format_err!($kind, $($arg)*).into());
    };
}
