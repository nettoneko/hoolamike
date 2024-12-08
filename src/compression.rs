use {
    crate::utils::boxed_iter,
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
}

impl ArchiveHandle {
    pub fn guess(file: std::fs::File) -> std::result::Result<Self, std::fs::File> {
        Err(file)
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| compress_tools::CompressToolsArchive::new(file).context("reading zip"))
                    .map(Self::CompressTools)
                    .map_err(|_| file)
            })
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| zip::ZipArchive::new(file).context("reading zip"))
                    .map(Self::Zip)
                    .map_err(|_| file)
            })
    }
}

impl std::io::Read for ArchiveFileHandle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ArchiveFileHandle::Zip(zip_file_seek) => zip_file_seek.read(buf),
            ArchiveFileHandle::CompressTools(compress_tools_seek) => compress_tools_seek.read(buf),
        }
    }
}

#[enum_dispatch::enum_dispatch(ArchiveHandle)]
pub trait ProcessArchiveFile {}

#[enum_dispatch::enum_dispatch]
pub enum ArchiveHandle {
    Zip(zip::ZipArchive),
    CompressTools(compress_tools::CompressToolsArchive),
}
