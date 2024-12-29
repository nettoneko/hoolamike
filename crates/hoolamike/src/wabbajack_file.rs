use {
    crate::{
        compression::ProcessArchive,
        install_modlist::directives::{WabbajackFileHandle, WabbajackFileHandleExt},
        progress_bars_v2::IndicatifWrapIoExt,
        utils::PathReadWrite,
    },
    anyhow::{Context, Result},
    std::path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct WabbajackFile {
    pub wabbajack_file_path: PathBuf,
    pub wabbajack_entries: Vec<PathBuf>,
    pub modlist: super::modlist_json::Modlist,
}

const MODLIST_JSON_FILENAME: &str = "modlist";

impl WabbajackFile {
    #[tracing::instrument]
    pub fn load_wabbajack_file(at_path: PathBuf) -> Result<(WabbajackFileHandle, Self)> {
        at_path
            .open_file_read()
            .and_then(|(_, file)| crate::compression::compress_tools::ArchiveHandle::new(file))
            .context("reading archive")
            .and_then(|mut archive| {
                archive.list_paths().and_then(|entries| {
                    archive
                        .get_handle(Path::new(MODLIST_JSON_FILENAME))
                        .context("looking up file by name")
                        .and_then(|handle| {
                            serde_json::from_reader::<_, crate::modlist_json::Modlist>(&mut tracing::Span::current().wrap_read(0, handle))
                                .context("reading archive contents")
                        })
                        .with_context(|| format!("reading [{MODLIST_JSON_FILENAME}]"))
                        .map(|modlist| Self {
                            wabbajack_file_path: at_path,
                            wabbajack_entries: entries,
                            modlist,
                        })
                        .map(|data| (WabbajackFileHandle::from_archive(archive), data))
                })
            })
    }
}
