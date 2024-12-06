use {
    crate::{
        downloaders::WithArchiveDescriptor,
        error::{MultiErrorCollectExt, TotalResult},
    },
    anyhow::{Context, Result},
    futures::{FutureExt, StreamExt, TryStreamExt},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
};

pub(crate) fn create_file_all(path: &Path) -> Result<std::fs::File> {
    path.parent()
        .map(|parent| std::fs::create_dir_all(parent).with_context(|| format!("creating directory for [{}]", parent.display())))
        .unwrap_or_else(|| Ok(()))
        .and_then(|_| {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
                .with_context(|| format!("creating file [{}]", path.display()))
        })
}

pub mod create_bsa {
    use {super::*, crate::modlist_json::directive::CreateBSADirective};

    #[derive(Clone, Debug)]
    pub struct CreateBSAHandler {}

    impl CreateBSAHandler {
        pub fn handle(self, directive: CreateBSADirective) -> Result<()> {
            anyhow::bail!("[CreateBSADirective] {directive:#?} is not implemented")
        }
    }
}

pub type DownloadSummary = Arc<BTreeMap<String, WithArchiveDescriptor<PathBuf>>>;

pub mod from_archive {
    use {
        super::*,
        crate::{
            compression::ProcessArchive,
            install_modlist::download_cache::validate_hash,
            modlist_json::directive::FromArchiveDirective,
            progress_bars::{print_error, vertical_progress_bar, ProgressKind, PROGRESS_BAR},
        },
        std::{convert::identity, path::Path},
    };

    #[derive(Clone, Debug)]
    pub struct FromArchiveHandler {
        pub download_summary: DownloadSummary,
        pub output_directory: PathBuf,
    }

    impl FromArchiveHandler {
        pub async fn handle(
            self,
            FromArchiveDirective {
                hash,
                size,
                to,
                archive_hash_path,
            }: FromArchiveDirective,
        ) -> Result<()> {
            let (source_hash, source_path) = match archive_hash_path {
                crate::modlist_json::directive::ArchiveHashPath::ArchiveHashAndPath((source_hash, source_path)) => (source_hash, source_path),
                other => anyhow::bail!("not implemented: {other:#?}"),
            };
            let source_path = source_path.into_path();
            let source = self
                .download_summary
                .get(&source_hash)
                .with_context(|| format!("directive expected hash [{source_hash}], but no such item was produced"))?;
            let source_file = source.inner.clone();
            let output_path = self.output_directory.join(to.into_path());

            if let Err(message) = validate_hash(output_path.clone(), hash).await {
                print_error(output_path.display().to_string(), &message);
                tokio::task::spawn_blocking(move || -> Result<_> {
                    let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndClear)
                        .attach_to(&PROGRESS_BAR)
                        .tap_mut(|pb| {
                            pb.set_message(output_path.display().to_string());
                        });

                    let output_file = create_file_all(&output_path)?;
                    let mut archive = std::fs::OpenOptions::new()
                        .read(true)
                        .open(&source_file)
                        .with_context(|| format!("opening [{}]", source_file.display()))
                        .and_then(|file| {
                            crate::compression::ArchiveHandle::guess(file)
                                .map_err(|_file| anyhow::anyhow!("no compression algorithm matched file [{}]", source_file.display()))
                        })?;
                    archive
                        .get_handle(Path::new(&source_path))
                        .and_then(|file| std::io::copy(&mut pb.wrap_read(file), &mut std::io::BufWriter::new(output_file)).context("copying file from archive"))
                        .map(|_| ())
                })
                .await
                .context("thread crashed")
                .and_then(identity)?;
            }
            Ok(())
        }
    }
}

pub mod inline_file {
    use {
        super::*,
        crate::{
            compression::ProcessArchive,
            install_modlist::download_cache::validate_hash,
            modlist_json::directive::InlineFileDirective,
            progress_bars::{print_error, vertical_progress_bar, ProgressKind, PROGRESS_BAR},
        },
        std::{convert::identity, path::Path},
    };

    #[derive(Clone, Debug)]
    pub struct InlineFileHandler {
        pub wabbajack_file: WabbajackFileHandle,
        pub output_directory: PathBuf,
    }

    impl InlineFileHandler {
        pub async fn handle(
            self,
            InlineFileDirective {
                hash,
                size,
                source_data_id,
                to,
            }: InlineFileDirective,
        ) -> Result<()> {
            let output_path = self.output_directory.join(to.into_path());
            if let Err(message) = validate_hash(output_path.clone(), hash).await {
                print_error(source_data_id.hyphenated().to_string(), &message);
                let wabbajack_file = self.wabbajack_file.clone();
                tokio::task::spawn_blocking(move || -> Result<_> {
                    let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndLeave)
                        .attach_to(&PROGRESS_BAR)
                        .tap_mut(|pb| pb.set_message(output_path.display().to_string()));

                    let output_file = create_file_all(&output_path)?;

                    let mut archive = wabbajack_file.blocking_lock();
                    archive
                        .get_handle(Path::new(&source_data_id.as_hyphenated().to_string()))
                        .and_then(|file| std::io::copy(&mut pb.wrap_read(file), &mut std::io::BufWriter::new(output_file)).context("copying file from archive"))
                        .map(|_| ())
                })
                .await
                .context("thread crashed")
                .and_then(identity)?;
            }
            Ok(())
        }
    }
}

pub mod patched_from_archive {
    use {super::*, crate::modlist_json::directive::PatchedFromArchiveDirective};

    #[derive(Clone, Debug)]
    pub struct PatchedFromArchiveHandler {}

    impl PatchedFromArchiveHandler {
        pub fn handle(self, directive: PatchedFromArchiveDirective) -> Result<()> {
            anyhow::bail!("[PatchedFromArchiveDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod remapped_inline_file {
    use {super::*, crate::modlist_json::directive::RemappedInlineFileDirective};

    #[derive(Clone, Debug)]
    pub struct RemappedInlineFileHandler {}

    impl RemappedInlineFileHandler {
        pub fn handle(self, directive: RemappedInlineFileDirective) -> Result<()> {
            anyhow::bail!("[RemappedInlineFileDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod transformed_texture {
    use {super::*, crate::modlist_json::directive::TransformedTextureDirective};

    #[derive(Clone, Debug)]
    pub struct TransformedTextureHandler {}

    impl TransformedTextureHandler {
        pub fn handle(self, directive: TransformedTextureDirective) -> Result<()> {
            anyhow::bail!("[TransformedTextureDirective ] {directive:#?} is not implemented")
        }
    }
}

use crate::modlist_json::Directive;

pub type WabbajackFileHandle = Arc<tokio::sync::Mutex<crate::compression::zip::ZipArchive>>;

#[extension_traits::extension(pub trait WabbajackFileHandleExt)]
impl WabbajackFileHandle {
    fn from_archive(archive: crate::compression::zip::ZipArchive) -> Self {
        Arc::new(tokio::sync::Mutex::new(archive))
    }
}

pub struct DirectivesHandler {
    pub create_bsa: create_bsa::CreateBSAHandler,
    pub from_archive: from_archive::FromArchiveHandler,
    pub inline_file: inline_file::InlineFileHandler,
    pub patched_from_archive: patched_from_archive::PatchedFromArchiveHandler,
    pub remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler,
    pub transformed_texture: transformed_texture::TransformedTextureHandler,
}

impl DirectivesHandler {
    #[allow(clippy::new_without_default)]
    pub fn new(wabbajack_file: WabbajackFileHandle, output_directory: PathBuf, sync_summary: Vec<WithArchiveDescriptor<PathBuf>>) -> Self {
        let download_summary = sync_summary
            .into_iter()
            .map(|s| (s.descriptor.hash.clone(), s))
            .collect::<BTreeMap<_, _>>()
            .pipe(Arc::new);
        Self {
            create_bsa: create_bsa::CreateBSAHandler {},
            from_archive: from_archive::FromArchiveHandler {
                output_directory: output_directory.clone(),
                download_summary,
            },
            inline_file: inline_file::InlineFileHandler {
                wabbajack_file,
                output_directory,
            },
            patched_from_archive: patched_from_archive::PatchedFromArchiveHandler {},
            remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler {},
            transformed_texture: transformed_texture::TransformedTextureHandler {},
        }
    }
    pub async fn handle(self: Arc<Self>, directive: Directive) -> Result<()> {
        match directive {
            Directive::CreateBSA(directive) => self.create_bsa.clone().handle(directive),
            Directive::FromArchive(directive) => self.from_archive.clone().handle(directive).await,
            Directive::InlineFile(directive) => self.inline_file.clone().handle(directive).await,
            Directive::PatchedFromArchive(directive) => self.patched_from_archive.clone().handle(directive),
            Directive::RemappedInlineFile(directive) => self.remapped_inline_file.clone().handle(directive),
            Directive::TransformedTexture(directive) => self.transformed_texture.clone().handle(directive),
        }
    }
    #[allow(clippy::unnecessary_literal_unwrap)]
    pub async fn handle_directives(self: Arc<Self>, directives: Vec<Directive>) -> TotalResult<()> {
        directives
            .pipe(futures::stream::iter)
            .then(|directive| {
                let directive_debug = format!("{directive:#?}");
                self.clone()
                    .handle(directive)
                    .map(move |r| r.with_context(|| format!("when handling directive: {directive_debug}")))
            })
            .map_err(|e| Err(e).expect("all directives must be handled"))
            .multi_error_collect()
            .await
    }
}
