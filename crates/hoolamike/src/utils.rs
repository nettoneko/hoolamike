use {
    anyhow::Context,
    itertools::Itertools,
    serde::{Deserialize, Serialize},
    std::{path::PathBuf, sync::Arc},
    tap::prelude::*,
};

#[extension_traits::extension(pub trait ReadableCatchUnwindExt)]
impl<T> std::result::Result<T, Box<dyn std::any::Any + Send>> {
    fn for_anyhow(self) -> anyhow::Result<T> {
        self.map_err(ReadableCatchUnwindErrorExt::to_readable_error)
    }
}

#[extension_traits::extension(pub trait ReadableCatchUnwindErrorExt)]
impl Box<dyn std::any::Any + Send> {
    fn to_readable_error(self) -> anyhow::Error {
        if let Some(message) = self.downcast_ref::<&str>() {
            format!("Caught panic with message: {}", message)
        } else if let Some(message) = self.downcast_ref::<String>() {
            format!("Caught panic with message: {}", message)
        } else {
            "Caught panic with an unknown type.".to_string()
        }
        .pipe(|e| anyhow::anyhow!("{e}"))
    }
}

#[extension_traits::extension(pub trait ResultZipExt)]
impl<T, E> std::result::Result<T, E> {
    fn zip<O>(self, other: std::result::Result<O, E>) -> std::result::Result<(T, O), E> {
        self.and_then(|one| other.map(|other| (one, other)))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Hash, derive_more::Display, Clone, Ord)]
pub struct MaybeWindowsPath(pub String);

impl MaybeWindowsPath {
    pub fn into_path(self) -> PathBuf {
        let s = self.0;
        let s = match s.contains("\\\\") {
            true => s.split("\\\\").join("/"),
            false => s,
        };
        let s = match s.contains("\\") {
            true => s.split("\\").join("/"),
            false => s,
        };
        PathBuf::from(s)
    }
}

pub fn boxed_iter<'a, T: 'a>(iter: impl Iterator<Item = T> + 'a) -> Box<dyn Iterator<Item = T> + 'a> {
    Box::new(iter)
}

#[macro_export]
macro_rules! cloned {
    ($($es:ident),+) => {$(
        #[allow(unused_mut)]
        let mut $es = $es.clone();
    )*}
}

#[extension_traits::extension(pub(crate) trait PathReadWrite)]
impl<T: AsRef<std::path::Path>> T {
    fn open_file_read(&self) -> anyhow::Result<(PathBuf, std::fs::File)> {
        std::fs::OpenOptions::new()
            .read(true)
            .open(self)
            .with_context(|| format!("opening file for reading at [{}]", self.as_ref().display()))
            .map(|file| (self.as_ref().to_owned(), file))
    }
    fn open_file_write(&self) -> anyhow::Result<(PathBuf, std::fs::File)> {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self)
            .with_context(|| format!("opening file for writing at [{}]", self.as_ref().display()))
            .map(|file| (self.as_ref().to_owned(), file))
    }
}

#[derive(derive_more::Display, Debug, Clone)]
pub(crate) struct ArcError(Arc<anyhow::Error>);

pub(crate) type ArcResult<T> = std::result::Result<T, ArcError>;

#[extension_traits::extension(pub(crate) trait AnyhowArcResultExt)]
impl<T> anyhow::Result<T> {
    fn arced(self) -> ArcResult<T> {
        self.map_err(Arc::new).map_err(ArcError)
    }
}

#[extension_traits::extension(pub(crate) trait ArcResultExt)]
impl<T> ArcResult<T> {
    fn into_inner_err(self) -> anyhow::Result<T> {
        self.map_err(|e| Arc::try_unwrap(e.0.clone()).unwrap_or_else(|_| anyhow::anyhow!("{e:#?}")))
    }
}

impl std::error::Error for ArcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}
