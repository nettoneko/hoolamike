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

mod iterator_as_read {
    use std::io::{Cursor, Read};

    pub struct IteratorAsRead<I>
    where
        I: Iterator,
    {
        iter: I,
        cursor: Option<Cursor<I::Item>>,
    }

    impl<I> IteratorAsRead<I>
    where
        I: Iterator,
    {
        pub fn new<T>(iter: T) -> Self
        where
            T: IntoIterator<IntoIter = I, Item = I::Item>,
        {
            let mut iter = iter.into_iter();
            let cursor = iter.next().map(Cursor::new);
            IteratorAsRead { iter, cursor }
        }
    }

    impl<I> Read for IteratorAsRead<I>
    where
        I: Iterator,
        Cursor<I::Item>: Read,
    {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            while let Some(ref mut cursor) = self.cursor {
                let read = cursor.read(buf)?;
                if read > 0 {
                    return Ok(read);
                }
                self.cursor = self.iter.next().map(Cursor::new);
            }
            Ok(0)
        }
    }
}

pub mod sevenz;
pub mod zip;
pub mod compress_tools {
    use {
        super::{ProcessArchive, *},
        ::compress_tools::*,
        anyhow::{Context, Result},
        io::{Seek, SeekFrom},
        std::path::PathBuf,
    };

    #[allow(clippy::unnecessary_literal_unwrap)]
    impl std::io::Read for CompressToolsFile<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let name = self.path.display().to_string();
            ArchiveIterator::from_read(&mut self.file)
                .map_err(std::io::Error::other)?
                .skip_while(|e| match e {
                    ArchiveContents::StartOfEntry(entry_name, _stat) => &name != entry_name,
                    ArchiveContents::DataChunk(_vec) => true,
                    ArchiveContents::EndOfEntry => true,
                    ArchiveContents::Err(error) => panic!("{error:?}"),
                })
                .skip(1)
                .filter_map(|e| match e {
                    ArchiveContents::DataChunk(vec) => Some(vec),
                    _ => None,
                })
                .fuse()
                .pipe(super::iterator_as_read::IteratorAsRead::new)
                .read(buf)
        }
    }
    pub struct CompressToolsFile<'a> {
        pub position: usize,
        pub path: PathBuf,
        pub file: &'a mut File,
    }

    pub struct CompressToolsArchive(std::fs::File);

    impl CompressToolsArchive {
        pub fn new(mut file: std::fs::File) -> Result<Self> {
            list_archive_files(&mut file)
                .context("could not read with compress-tools (libarchive)")
                .map(|_| Self(file))
        }

        pub fn get_handle(&mut self, for_path: &Path) -> Result<CompressToolsFile> {
            self.list_paths()
                .and_then(|paths| {
                    paths
                        .contains(&for_path.to_owned())
                        .then_some(())
                        .with_context(|| format!("archive does not contain file [{}]", for_path.display()))
                })
                .and_then(|_| {
                    self.0
                        .seek(SeekFrom::Start(0))
                        .context("seeking file back to beginning")
                })
                .map(|_| CompressToolsFile {
                    position: 0,
                    path: for_path.to_owned(),
                    file: &mut self.0,
                })
        }
    }

    impl ProcessArchive for CompressToolsArchive {
        fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
            ::compress_tools::list_archive_files(&mut self.0)
                .context("listing archive files")
                .map(|e| e.into_iter().map(PathBuf::from).collect())
        }

        fn get_handle<'this>(&'this mut self, path: &Path) -> Result<super::ArchiveFileHandle<'this>> {
            self.get_handle(path)
                .map(super::ArchiveFileHandle::CompressTools)
        }
    }

    impl super::ProcessArchiveFile for CompressToolsFile<'_> {}
}

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
    CompressTools(compress_tools::CompressToolsFile<'a>),
}

impl ArchiveHandle {
    pub fn guess(file: std::fs::File) -> std::result::Result<Self, std::fs::File> {
        Err(file)
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| zip::ZipArchive::new(file).context("reading zip"))
                    .map(Self::Zip)
                    .map_err(|_| file)
            })
            .or_else(|file| {
                file.try_clone()
                    .context("cloning file")
                    .and_then(|file| compress_tools::CompressToolsArchive::new(file).context("reading zip"))
                    .map(Self::CompressTools)
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
