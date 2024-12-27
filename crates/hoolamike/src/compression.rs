use {
    crate::{
        install_modlist::directives::nested_archive_manager::{max_open_files, WithPermit, OPEN_FILE_PERMITS},
        progress_bars_v2::IndicatifWrapIoExt,
        utils::{boxed_iter, ReadableCatchUnwindExt},
    },
    ::wrapped_7zip::WRAPPED_7ZIP,
    anyhow::{Context, Result},
    std::{
        convert::identity,
        io::{Seek, Write},
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tracing::Instrument,
};

pub mod bethesda_archive;
pub mod compress_tools;
pub mod sevenz;
pub mod zip;

pub mod forward_only_seek;

#[enum_dispatch::enum_dispatch(ArchiveHandle)]
pub trait ProcessArchive: Sized {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>>;
    fn get_handle(&mut self, path: &Path) -> Result<self::ArchiveFileHandle>;
}

impl ArchiveHandle<'_> {
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
pub enum ArchiveFileHandle {
    // CompressTools(compress_tools::CompressToolsFile),
    Wrapped7Zip((::wrapped_7zip::list_output::ListOutputEntry, ::wrapped_7zip::ArchiveFileHandle)),
    Bethesda(self::bethesda_archive::BethesdaArchiveFile),
}

impl ArchiveFileHandle {
    pub fn size(&mut self) -> Result<u64> {
        match self {
            ArchiveFileHandle::Wrapped7Zip((entry, _)) => Ok(entry.size),
            ArchiveFileHandle::Bethesda(bethesda_archive_file) => bethesda_archive_file
                .stream_len()
                .context("could not seek to find the stream length"),
        }
    }
}

// static_assertions::assert_impl_all!(zip::ZipFile<'static>: Send, Sync);
// static_assertions::assert_impl_all!(compress_tools::CompressToolsFile: Send, Sync);
static_assertions::assert_impl_all!(::wrapped_7zip::ArchiveFileHandle: Send, Sync);
static_assertions::assert_impl_all!(self::bethesda_archive::BethesdaArchiveFile: Send, Sync);
static_assertions::assert_impl_all!(ArchiveFileHandle: Send , Sync);

impl ArchiveHandle<'_> {
    pub fn guess(path: &Path) -> anyhow::Result<Self> {
        std::panic::catch_unwind(|| {
            Err(())
                .or_else(|_| {
                    bethesda_archive::BethesdaArchive::open(path)
                        .context("reading zip")
                        .map(Self::Bethesda)
                        .tap_err(|message| tracing::trace!("could not open archive with compress-tools: {message:?}"))
                })
                .or_else(|_| {
                    WRAPPED_7ZIP
                        .with(|wrapped| wrapped.open_file(path).map(Self::Wrapped7Zip))
                        .tap_err(|message| tracing::warn!("could not open archive with 7z: {message:?}"))
                })
                .tap_ok(|a| tracing::trace!("succesfully opened an archive: {a:?}"))
                .map_err(|_| anyhow::anyhow!("no defined archive handler could handle this file"))
        })
        .for_anyhow()
        .context("unexpected panic")
        .and_then(identity)
    }
}

impl std::io::Read for ArchiveFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            // ArchiveFileHandle::CompressTools(compress_tools_seek) => compress_tools_seek.read(buf),
            ArchiveFileHandle::Wrapped7Zip(wrapped_7zip) => wrapped_7zip.1.read(buf),
            ArchiveFileHandle::Bethesda(bethesda_archive_file) => bethesda_archive_file.read(buf),
        }
    }
}

#[enum_dispatch::enum_dispatch(ArchiveHandle)]
pub trait ProcessArchiveFile {}

#[enum_dispatch::enum_dispatch]
#[derive(Debug)]
pub enum ArchiveHandle<'a> {
    // CompressTools(compress_tools::CompressToolsArchive),
    Wrapped7Zip(::wrapped_7zip::ArchiveHandle),
    Bethesda(bethesda_archive::BethesdaArchive<'a>),
}

pub mod wrapped_7zip;

#[extension_traits::extension(pub(crate) trait SeekWithTempFileExt)]
impl<T: std::io::Read + Sync + 'static> T
where
    Self: Sized + Sync + Send + 'static,
{
    fn seek_with_temp_file_blocking(mut self, expected_size: u64, permit: tokio::sync::OwnedSemaphorePermit) -> Result<WithPermit<tempfile::TempPath>> {
        let span = tracing::info_span!(
            "seek_with_temp_file",
            acquired_file_permits=%(max_open_files() - OPEN_FILE_PERMITS.available_permits()),
            max_open_files=%max_open_files(),
        );
        tempfile::NamedTempFile::new()
            .context("creating a tempfile")
            .and_then(|mut temp_file| {
                {
                    let writer = &mut span.clone().wrap_write(expected_size, &mut temp_file);
                    std::io::copy(&mut self, writer)
                }
                .context("creating a seekable temp file")
                .and_then(|wrote_size| {
                    wrote_size
                        .eq(&expected_size)
                        .then_some(wrote_size)
                        .with_context(|| format!("error when writing temp file: expected [{expected_size}], found [{wrote_size}]"))
                })
                .map(|_| temp_file)
                .and_then(|mut file| {
                    file.flush()
                        .context("flushing file")
                        .map(|_| file.into_temp_path())
                })
            })
            .map(|file| WithPermit { permit, inner: file })
    }
    async fn seek_with_temp_file(self, expected_size: u64) -> Result<WithPermit<tempfile::TempPath>> {
        let span = tracing::info_span!(
            "seek_with_temp_file",
            acquired_file_permits=%(max_open_files() - OPEN_FILE_PERMITS.available_permits()),
            max_open_files=%max_open_files(),
        );
        let reader = Arc::new(std::sync::Mutex::new(self));
        WithPermit::new_blocking(&OPEN_FILE_PERMITS, {
            cloned![span];
            move || {
                let span = span.entered();
                tempfile::NamedTempFile::new()
                    .context("creating a tempfile")
                    .and_then(|mut temp_file| {
                        {
                            let mut reader = reader.lock().unwrap();
                            let writer = &mut span.clone().wrap_write(expected_size, &mut temp_file);
                            std::io::copy(&mut *reader, writer)
                        }
                        .context("creating a seekable temp file")
                        .and_then(|wrote_size| {
                            wrote_size
                                .eq(&expected_size)
                                .then_some(wrote_size)
                                .with_context(|| format!("error when writing temp file: expected [{expected_size}], found [{wrote_size}]"))
                        })
                        .map(|_| temp_file)
                        .and_then(|mut file| {
                            file.flush()
                                .context("flushing file")
                                .map(|_| file.into_temp_path())
                        })
                    })
            }
        })
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        futures::{StreamExt, TryFutureExt, TryStreamExt},
        std::io::BufReader,
    };
    #[test_log::test(tokio::test)]
    async fn test_seek_with_tempfile() -> Result<()> {
        [
            //
            [1u8; 8].as_slice(),
        ]
        .pipe(futures::stream::iter)
        .map(|slice| (slice, slice.pipe(std::io::Cursor::new).pipe(BufReader::new)))
        .map(Ok)
        .try_for_each(|(slice, reader)| {
            reader
                .seek_with_temp_file(slice.len() as _)
                .and_then(move |temp| async move {
                    let buffer = temp
                        .inner
                        .pipe(|f| std::fs::read(&f))
                        .context("reading failed")?;
                    assert_eq!(slice, &buffer, "buffer mismatch");
                    Ok(())
                })
        })
        .await
    }
}
