use std::path::PathBuf;

pub type Extracted = tempfile::TempPath;

#[derive(Debug)]
pub enum SourceKind {
    JustPath(PathBuf),
    CachedPath(Extracted),
}

impl AsRef<std::path::Path> for SourceKind {
    fn as_ref(&self) -> &std::path::Path {
        match self {
            SourceKind::JustPath(path_buf) => path_buf,
            SourceKind::CachedPath(cached) => cached,
        }
    }
}
