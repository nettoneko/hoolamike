use {crate::modlist_json::ArchiveDescriptor, std::path::PathBuf, url::Url};

pub mod gamefile_source_downloader;
pub mod google_drive;
pub mod http {
    pub struct HttpDownloader {}
}
pub mod manual {
    pub struct ManualDownloader {}
}
pub mod nexus;
pub mod wabbajack_cdn;

pub mod helpers;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, transpare::Transpare)]
pub struct WithArchiveDescriptor<T> {
    pub inner: T,
    pub descriptor: ArchiveDescriptor,
}

pub type MergeDownloadTask = WithArchiveDescriptor<(Vec<url::Url>, PathBuf)>;
pub type DownloadTask = WithArchiveDescriptor<(url::Url, PathBuf)>;
pub type CopyFileTask = WithArchiveDescriptor<(PathBuf, PathBuf)>;

#[derive(Debug, Clone, derive_more::From)]
pub enum SyncTask {
    MergeDownload(MergeDownloadTask),
    Download(DownloadTask),
    Copy(CopyFileTask),
}
