use {
    super::ProcessArchive,
    crate::{
        progress_bars::{vertical_progress_bar, PROGRESS_BAR},
        utils::boxed_iter,
    },
    anyhow::{Context, Result},
    ba2::{
        fo4::{self, ChunkCompressionOptionsBuilder, FileWriteOptionsBuilder},
        prelude::*,
        BString,
        ByteSlice,
        Reader,
    },
    itertools::Itertools,
    std::{
        borrow::Cow,
        io::Read,
        path::{Path, PathBuf},
    },
    tap::prelude::*,
    wrapped_7zip::MaybeWindowsPath,
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
    fn list_paths_with_originals(&mut self) -> Result<Vec<(PathBuf, BString)>> {
        self.0
            .iter()
            .map(|(key, _file)| {
                key.name()
                    .to_str()
                    .context("name is not a valid string")
                    .map(|s| s.as_bytes())
                    .and_then(bethesda_path_to_path)
                    .map(|path| (path, key.name().to_owned()))
            })
            .collect::<Result<Vec<_>>>()
            .context("listing paths for bethesda archive")
    }
}

impl super::ProcessArchive for Fallout4Archive<'_> {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        self.list_paths_with_originals()
            .map(|paths| paths.into_iter().map(|(p, _)| p).collect())
    }

    fn get_handle(&mut self, path: &Path) -> Result<super::ArchiveFileHandle<'_>> {
        let mut output = tempfile::SpooledTempFile::new(256 * 1024 * 1024);
        let options = FileWriteOptionsBuilder::new()
            .compression_format(self.1.compression_format())
            .build();
        let pb = vertical_progress_bar(0, crate::progress_bars::ProgressKind::ExtractTemporaryFile, indicatif::ProgressFinish::AndLeave)
            .attach_to(&PROGRESS_BAR)
            .tap_mut(|pb| pb.set_message(path.display().to_string()));
        self.list_paths_with_originals()?
            .pipe(|paths| {
                paths
                    .iter()
                    .find_map(|(entry, repr)| entry.eq(path).then_some(repr))
                    .with_context(|| format!("[{}] not found in [{paths:?}]", path.display()))
                    .and_then(|bethesda_path| {
                        self.0
                            .get(&fo4::ArchiveKey::from(bethesda_path.clone()))
                            .context("could not read file")
                    })
                    .and_then(|file| {
                        file.write(&mut pb.wrap_write(&mut output), &options)
                            .context("extracting fallout 4 bsa")
                            .map(|_| output)
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

    fn get_handle(&mut self, path: &Path) -> Result<super::ArchiveFileHandle<'_>> {
        match self {
            BethesdaArchive::Fallout4(fo4) => fo4.get_handle(path),
        }
    }
}

impl BethesdaArchive<'_> {
    pub fn open(file: &Path) -> Result<Self> {
        let format = std::fs::OpenOptions::new()
            .read(true)
            .open(file)
            .context("opening bethesda archive")
            .and_then(|mut archive| ba2::guess_format(&mut archive).context("unrecognized format"))
            .with_context(|| format!("guessing format of [{}]", file.display()))?;
        match format {
            ba2::FileFormat::FO4 => ba2::fo4::Archive::read(file)
                .context("opening fo4")
                .map(BethesdaArchive::Fallout4),
            ba2::FileFormat::TES3 => todo!(),
            ba2::FileFormat::TES4 => todo!(),
        }
    }
}

type BytesIterator<'a> = iter_read::IterRead<std::io::Result<Vec<u8>>, Box<dyn Iterator<Item = std::io::Result<Vec<u8>>> + 'a>>;

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
