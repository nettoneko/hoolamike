use {
    super::{ProcessArchive, *},
    ::compress_tools::*,
    anyhow::{Context, Result},
    std::{
        io::{BufWriter, Seek},
        path::PathBuf,
    },
    tracing::trace,
};

pub type CompressToolsFile = tempfile::SpooledTempFile;

#[derive(Debug)]
pub struct CompressToolsArchive(std::fs::File);

impl CompressToolsArchive {
    pub fn new(mut file: std::fs::File) -> Result<Self> {
        list_archive_files(&mut file)
            .context("listing files")
            .and_then(|_| file.rewind().context("rewinding the stream"))
            .context("could not read with compress-tools (libarchive)")
            .map(|_| Self(file))
    }

    #[tracing::instrument(skip(self))]
    pub fn get_handle(&mut self, for_path: &Path) -> Result<CompressToolsFile> {
        self.0.rewind().context("rewinding file")?;
        let lookup = for_path.display().to_string();
        list_archive_files(&mut self.0)
            .context("listing archive")
            .and_then(|files| {
                files
                    .contains(&lookup)
                    .then_some(&lookup)
                    .with_context(|| format!("no [{lookup}] in {files:?}"))
                    .tap_ok(|lookup| trace!("[{lookup}] found in [{files:?}]"))
            })
            .and_then(|lookup| {
                self.0.rewind().context("rewinding file")?;
                tempfile::SpooledTempFile::new(1024 * 1024).pipe(|mut temp_file| {
                    {
                        let mut writer = BufWriter::new(&mut temp_file);
                        uncompress_archive_file(&mut tracing::Span::current().wrap_read(0, &mut self.0), &mut writer, lookup)
                    }
                    .context("extracting archive")
                    .tap_ok(|bytes| trace!(%bytes, "extracted from CompressTools archive"))
                    .and_then(|_| {
                        temp_file
                            .flush()
                            .and_then(|_| temp_file.rewind())
                            .context("rewinding to beginning of file")
                            .map(|_| temp_file)
                    })
                })
            })
    }
}

impl ProcessArchive for CompressToolsArchive {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        ::compress_tools::list_archive_files(&mut self.0)
            .context("listing archive files")
            .map(|e| e.into_iter().map(PathBuf::from).collect())
            .and_then(|out| self.0.rewind().context("rewinding file").map(|_| out))
    }

    fn get_handle<'this>(&mut self, path: &Path) -> Result<super::ArchiveFileHandle> {
        self.get_handle(path)
            .map(super::ArchiveFileHandle::CompressTools)
    }
}

impl super::ProcessArchiveFile for CompressToolsFile {}
