use {
    crate::utils::boxed_iter,
    ::wrapped_7zip::which,
    anyhow::{Context, Result},
    std::{
        fs::File,
        io::{self},
        path::{Path, PathBuf},
    },
    tap::prelude::*,
};

pub mod compress_tools;
pub mod sevenz;
pub mod zip;

pub mod forward_only_seek;

#[enum_dispatch::enum_dispatch(ArchiveHandle)]
pub trait ProcessArchive: Sized {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>>;
    fn get_handle(&mut self, path: &Path) -> Result<self::ArchiveFileHandle<'_>>;
}

impl ArchiveHandle {
    pub fn iter_mut(mut self) -> Result<FileHandleIterator<Self>> {
        self.list_paths().map(|paths| FileHandleIterator {
            paths: paths.into_iter().pipe(boxed_iter),
            archive: self,
        })
    }
}

pub struct FileHandleIterator<T> {
    paths: Box<dyn Iterator<Item = PathBuf>>,
    archive: T,
}

impl<T: ProcessArchive> FileHandleIterator<T> {
    pub fn try_map<U, F: FnMut(ArchiveFileHandle) -> Result<U>>(self, mut map: F) -> std::vec::IntoIter<Result<U>> {
        self.pipe(|Self { paths, mut archive }| {
            paths
                .into_iter()
                .map(|path| archive.get_handle(&path).and_then(&mut map))
                .collect::<Vec<_>>()
        })
        .into_iter()
    }
}

#[allow(clippy::large_enum_variant)]
#[enum_dispatch::enum_dispatch]
pub enum ArchiveFileHandle<'a> {
    Zip(zip::ZipFile<'a>),
    CompressTools(compress_tools::CompressToolsFile),
    Wrapped7Zip(::wrapped_7zip::ArchiveFileHandle),
}

impl ArchiveHandle {
    pub fn guess(file: std::fs::File, path: &Path) -> std::result::Result<Self, std::fs::File> {
        Err(file)
            .or_else(|file| {
                ["7z", "7z.exe"]
                    .into_iter()
                    .find_map(|bin| which::which(bin).ok())
                    .context("no 7z binary found")
                    .and_then(|path| ::wrapped_7zip::Wrapped7Zip::new(&path))
                    .and_then(|wrapped| wrapped.open_file(path).map(Self::Wrapped7Zip))
                    .tap_err(|message| tracing::warn!("could not open archive with 7z: {message:?}"))
                    .map_err(|_| file)
            })
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| compress_tools::CompressToolsArchive::new(file).context("reading zip"))
                    .map(Self::CompressTools)
                    .tap_err(|message| tracing::trace!("could not open archive with compress-tools: {message:?}"))
                    .map_err(|_| file)
            })
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| zip::ZipArchive::new(file).context("reading zip"))
                    .map(Self::Zip)
                    .tap_err(|message| tracing::trace!("could not open archive with zip: {message:?}"))
                    .map_err(|_| file)
            })
            .tap_ok(|a| tracing::trace!("succesfully opened an archive: {a:?}"))
    }
}

impl std::io::Read for ArchiveFileHandle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ArchiveFileHandle::Zip(zip_file_seek) => zip_file_seek.read(buf),
            ArchiveFileHandle::CompressTools(compress_tools_seek) => compress_tools_seek.read(buf),
            ArchiveFileHandle::Wrapped7Zip(wrapped_7zip) => wrapped_7zip.read(buf),
        }
    }
}

#[enum_dispatch::enum_dispatch(ArchiveHandle)]
pub trait ProcessArchiveFile {}

#[enum_dispatch::enum_dispatch]
#[derive(Debug)]
pub enum ArchiveHandle {
    Zip(zip::ZipArchive),
    CompressTools(compress_tools::CompressToolsArchive),
    Wrapped7Zip(::wrapped_7zip::ArchiveHandle),
}

pub mod wrapped_7zip;

#[extension_traits::extension(pub trait SeekWithTempFileExt)]
impl<T: std::io::Read> T {
    fn seek_with_temp_file(mut self) -> Result<tempfile::SpooledTempFile> {
        tempfile::SpooledTempFile::new(128 * 1024 * 1024).pipe(|mut temp_file| {
            std::io::copy(&mut self, &mut temp_file)
                .context("creating a seekable temp file")
                .map(|_| temp_file)
        })
    }
}
