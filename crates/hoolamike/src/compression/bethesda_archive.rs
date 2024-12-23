use {
    super::ProcessArchive,
    crate::{
        progress_bars::{vertical_progress_bar, PROGRESS_BAR},
        utils::{MaybeWindowsPath, PathReadWrite, ReadableCatchUnwindExt},
    },
    anyhow::{Context, Result},
    ba2::{
        fo4::{self, FileWriteOptionsBuilder},
        ByteSlice,
        Reader,
    },
    std::{
        convert::identity,
        io::{Read, Seek, Write},
        panic::catch_unwind,
        path::{Path, PathBuf},
    },
};

type Fallout4Archive<'a> = (ba2::fo4::Archive<'a>, ba2::fo4::ArchiveOptions);

fn bethesda_path_to_path(bethesda_path: &[u8]) -> Result<PathBuf> {
    bethesda_path
        .to_str()
        .with_context(|| format!("converting [{}] to utf8", String::from_utf8_lossy(bethesda_path)))
        .map(ToOwned::to_owned)
        .map(MaybeWindowsPath)
        .map(MaybeWindowsPath::into_path)
}

#[extension_traits::extension(pub trait BethesdaArchiveCompat)]
impl Fallout4Archive<'_> {
    fn list_paths_with_originals(&self) -> Result<Vec<(PathBuf, fo4::ArchiveKey<'_>)>> {
        self.0
            .iter()
            .map(|(key, _file)| {
                key.name()
                    .to_str()
                    .context("name is not a valid string")
                    .map(|s| s.as_bytes())
                    .and_then(bethesda_path_to_path)
                    .map(|path| (path, key.to_owned()))
            })
            .collect::<Result<Vec<_>>>()
            .context("listing paths for bethesda archive")
    }
}

#[cfg(test)]
mod integration_tests;

impl super::ProcessArchive for Fallout4Archive<'_> {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        self.list_paths_with_originals()
            .map(|paths| paths.into_iter().map(|(p, _)| p).collect())
    }
    #[tracing::instrument(skip(self))]
    fn get_handle(&mut self, path: &Path) -> Result<super::ArchiveFileHandle> {
        use tap::prelude::*;

        let mut output = tempfile::SpooledTempFile::new(256 * 1024 * 1024);
        let options = FileWriteOptionsBuilder::new()
            .compression_format(self.1.compression_format())
            .build();
        let pb = vertical_progress_bar(0, crate::progress_bars::ProgressKind::ExtractTemporaryFile, indicatif::ProgressFinish::AndClear)
            .attach_to(&PROGRESS_BAR)
            .tap_mut(|pb| pb.set_message(path.display().to_string()));
        self.list_paths_with_originals()
            .context("listing entries")
            .and_then(|paths| {
                paths
                    .iter()
                    .find_map(|(entry, repr)| entry.eq(path).then_some(repr))
                    .with_context(|| format!("[{}] not found in [{paths:?}]", path.display()))
                    .and_then(|bethesda_path| {
                        catch_unwind(|| self.0.get(bethesda_path).context("could not read file"))
                            .for_anyhow()
                            .and_then(identity)
                            .context("reading archive entry")
                    })
                    .and_then(|file| {
                        catch_unwind(|| {
                            pb.set_length(file.iter().map(|chunk| chunk.len() as u64).sum());
                            pb.tick();
                            let mut writer = pb.wrap_write(&mut output);
                            file.write(&mut writer, &options)
                                // file.write(&mut output, &options)
                                .context("writing fallout 4 bsa to output buffer")
                                .and_then(|_| {
                                    output.rewind().context("rewinding file").and_then(|_| {
                                        let wrote = writer.progress.length().unwrap_or(0);
                                        tracing::debug!(%wrote, "finished dumping bethesda archive");
                                        output.flush().context("flushing").map(|_| output)
                                    })
                                })
                        })
                        .for_anyhow()
                        .and_then(identity)
                        .context("extracting fallout 4 bsa")
                    })
                    .map(BethesdaArchiveFile::Fallout4)
                    .map(super::ArchiveFileHandle::Bethesda)
                    .with_context(|| format!("getting file handle for [{}] out of derived paths [{:#?}]", path.display(), paths))
            })
            .context("getting fallout4 archive handle")
    }
}

#[derive(Debug)]
pub enum BethesdaArchive<'a> {
    Fallout4(Fallout4Archive<'a>),
}

impl ProcessArchive for BethesdaArchive<'_> {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        match self {
            BethesdaArchive::Fallout4(fo4) => fo4.list_paths(),
        }
    }

    fn get_handle(&mut self, path: &Path) -> Result<super::ArchiveFileHandle> {
        match self {
            BethesdaArchive::Fallout4(fo4) => fo4.get_handle(path),
        }
    }
}

impl BethesdaArchive<'_> {
    pub fn open(file: &Path) -> Result<Self> {
        file.open_file_read()
            .context("opening bethesda archive")
            .and_then(|(_path, mut archive)| {
                ba2::guess_format(&mut archive)
                    .context("unrecognized format")
                    .and_then(|format| match format {
                        ba2::FileFormat::FO4 => ba2::fo4::Archive::read(file)
                            .context("opening fo4")
                            .map(BethesdaArchive::Fallout4),
                        ba2::FileFormat::TES3 => anyhow::bail!("{format:?} is not supported"),
                        ba2::FileFormat::TES4 => anyhow::bail!("{format:?} is not supported"),
                    })
            })
    }
}

pub enum BethesdaArchiveFile {
    Fallout4(tempfile::SpooledTempFile),
}

impl Read for BethesdaArchiveFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BethesdaArchiveFile::Fallout4(file) => file.read(buf),
        }
    }
}
