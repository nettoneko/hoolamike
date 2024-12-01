use std::path::PathBuf;

use url::Url;

use crate::modlist_json::ArchiveDescriptor;

pub mod gamefile_source_downloader;
pub mod google_drive;
pub mod http {
    pub struct HttpDownloader {}
}
pub mod manual {
    pub struct ManualDownloader {}
}
pub mod nexus;
pub mod wabbajack_cdn {
    pub struct WabbajackCDNDownloader {}
}

pub mod helpers;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, transpare::Transpare)]
pub struct WithArchiveDescriptor<T> {
    pub inner: T,
    pub descriptor: ArchiveDescriptor,
}

pub type DownloadTask = WithArchiveDescriptor<(url::Url, PathBuf)>;
pub type CopyFileTask = WithArchiveDescriptor<(PathBuf, PathBuf)>;

#[derive(Debug, Clone, derive_more::From)]
pub enum SyncTask {
    Download(DownloadTask),
    Copy(CopyFileTask),
}
