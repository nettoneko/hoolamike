use {
    crate::{
        downloaders::WithArchiveDescriptor,
        install_modlist::download_cache::validate_hash,
        modlist_json::{
            directive::{
                ArchiveHashPath,
                CreateBSADirective,
                FromArchiveDirective,
                InlineFileDirective,
                PatchedFromArchiveDirective,
                RemappedInlineFileDirective,
                TransformedTextureDirective,
            },
            DirectiveKind,
        },
        progress_bars::{vertical_progress_bar, ProgressKind, PROGRESS_BAR},
        utils::PathReadWrite,
    },
    anyhow::{Context, Result},
    futures::{FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt},
    itertools::Itertools,
    nested_archive_manager::{max_open_files, NestedArchivesService},
    remapped_inline_file::RemappingContext,
    std::{
        collections::BTreeMap,
        future::ready,
        ops::Div,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
    tap::prelude::*,
    tokio::sync::Mutex,
    tracing::{info_span, Instrument},
};

pub(crate) fn create_file_all(path: &Path) -> Result<std::fs::File> {
    path.parent()
        .map(|parent| std::fs::create_dir_all(parent).with_context(|| format!("creating directory for [{}]", parent.display())))
        .unwrap_or_else(|| Ok(()))
        .and_then(|_| path.open_file_write())
        .map(|(_, f)| f)
}

pub mod create_bsa;

pub type DownloadSummary = Arc<BTreeMap<String, WithArchiveDescriptor<PathBuf>>>;

pub mod from_archive;

pub mod inline_file;

pub mod patched_from_archive;

pub mod remapped_inline_file;

pub mod transformed_texture;

use crate::modlist_json::Directive;

pub type WabbajackFileHandle = Arc<tokio::sync::Mutex<crate::compression::wrapped_7zip::ArchiveHandle>>;

#[extension_traits::extension(pub trait WabbajackFileHandleExt)]
impl WabbajackFileHandle {
    fn from_archive(archive: crate::compression::wrapped_7zip::ArchiveHandle) -> Self {
        Arc::new(tokio::sync::Mutex::new(archive))
    }
}

pub struct DirectivesHandler {
    pub config: DirectivesHandlerConfig,
    pub create_bsa: create_bsa::CreateBSAHandler,
    pub from_archive: from_archive::FromArchiveHandler,
    pub inline_file: inline_file::InlineFileHandler,
    pub patched_from_archive: patched_from_archive::PatchedFromArchiveHandler,
    pub remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler,
    pub transformed_texture: transformed_texture::TransformedTextureHandler,
    pub nested_archive_manager: Arc<NestedArchivesService>,
}

impl DirectiveKind {
    /// directives are not supposed to be executed in order, BSA directives expect stuff to be there up front no matter
    /// what their position in the list is
    pub fn priority(self) -> u8 {
        match self {
            DirectiveKind::InlineFile => 10,
            DirectiveKind::FromArchive => 11,
            DirectiveKind::PatchedFromArchive => 12,
            DirectiveKind::RemappedInlineFile => 13,
            DirectiveKind::TransformedTexture => 240,
            DirectiveKind::CreateBSA => 250,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirectivesHandlerConfig {
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
    pub game_directory: PathBuf,
    pub downloads_directory: PathBuf,
}

pub mod nested_archive_manager;

fn concurrency() -> usize {
    #[cfg(not(debug_assertions))]
    {
        use std::ops::{Div, Mul};

        num_cpus::get().div(2).saturating_sub(1).max(1)
    }
    #[cfg(debug_assertions)]
    {
        1
    }
}

#[extension_traits::extension(pub trait StreamTryFlatMapExt)]
impl<'iter, T, E, I> I
where
    E: 'iter,
    T: 'iter,
    I: Stream<Item = Result<T, E>> + 'iter,
{
    fn try_flat_map<U, NewStream, F>(self, try_flat_map: F) -> impl Stream<Item = Result<U, E>> + 'iter
    where
        U: 'iter,
        NewStream: Stream<Item = Result<U, E>> + 'iter,
        F: FnOnce(T) -> NewStream + 'iter + Clone,
    {
        self.flat_map(move |e| match e {
            Ok(value) => value.pipe(try_flat_map.clone()).boxed_local(),
            Err(e) => e
                .pipe(Err)
                .pipe(ready)
                .pipe(futures::stream::once)
                .boxed_local(),
        })
    }
}

const EXTENSION_HASH_WHITELIST: &[&str] = &[
    // hashes won't match because headers are also hashed in wabbajack
    "dds",
];

fn is_whitelisted_by_path(path: &Path) -> bool {
    matches!(
        path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref(),
        Some(ext) if EXTENSION_HASH_WHITELIST.contains(&ext)
    )
}

pub async fn validate_hash_with_overrides(path: PathBuf, hash: String, size: u64) -> Result<PathBuf> {
    match is_whitelisted_by_path(&path) {
        true => super::download_cache::validate_file_size(path, size).await,
        false => validate_hash(path, hash).await,
    }
}

impl DirectivesHandler {
    #[allow(clippy::new_without_default)]
    pub fn new(config: DirectivesHandlerConfig, sync_summary: Vec<WithArchiveDescriptor<PathBuf>>) -> Self {
        let DirectivesHandlerConfig {
            wabbajack_file,
            output_directory,
            game_directory,
            downloads_directory,
        } = config.clone();
        let download_summary: DownloadSummary = sync_summary
            .into_iter()
            .map(|s| (s.descriptor.hash.clone(), s))
            .collect::<BTreeMap<_, _>>()
            .pipe(Arc::new);

        let nested_archive_service = NestedArchivesService::new(download_summary.clone(), max_open_files()).pipe(Arc::new);
        Self {
            config,
            create_bsa: create_bsa::CreateBSAHandler {
                output_directory: output_directory.clone(),
            },
            from_archive: from_archive::FromArchiveHandler {
                output_directory: output_directory.clone(),
                nested_archive_service: nested_archive_service.clone(),
            },
            inline_file: inline_file::InlineFileHandler {
                wabbajack_file: wabbajack_file.clone(),
                output_directory: output_directory.clone(),
            },
            patched_from_archive: patched_from_archive::PatchedFromArchiveHandler {
                output_directory: output_directory.clone(),
                wabbajack_file: wabbajack_file.clone(),
                nested_archive_service: nested_archive_service.clone(),
            },
            remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler {
                remapping_context: Arc::new(RemappingContext {
                    game_folder: game_directory.clone(),
                    output_directory: output_directory.clone(),
                    downloads_directory,
                }),
                wabbajack_file: wabbajack_file.clone(),
            },
            transformed_texture: transformed_texture::TransformedTextureHandler {
                output_directory: output_directory.clone(),
                nested_archive_service: nested_archive_service.clone(),
            },
            nested_archive_manager: nested_archive_service,
        }
    }

    #[allow(clippy::unnecessary_literal_unwrap)]
    pub fn handle_directives(self: Arc<Self>, directives: Vec<Directive>) -> impl Stream<Item = Result<()>> {
        let pb = vertical_progress_bar(
            directives.iter().map(directive_size).sum(),
            ProgressKind::InstallDirectives,
            indicatif::ProgressFinish::AndClear,
        )
        .attach_to(&PROGRESS_BAR)
        .tap_mut(|pb| {
            pb.set_message("TOTAL");
            pb.enable_steady_tick(Duration::from_secs(2));
        });

        fn directive_size(d: &Directive) -> u64 {
            match d {
                Directive::CreateBSA(directive) => directive.size,
                Directive::FromArchive(directive) => directive.size,
                Directive::InlineFile(directive) => directive.size,
                Directive::PatchedFromArchive(directive) => directive.size,
                Directive::RemappedInlineFile(directive) => directive.size,
                Directive::TransformedTexture(directive) => directive.size,
            }
        }

        let manager = self.clone();
        let nested_archive_manager = self.nested_archive_manager.clone();
        let check_completed = {
            let output_directory = self.from_archive.output_directory.clone();
            move |directive: &Directive| {
                let kind = DirectiveKind::from(directive);
                match directive {
                    Directive::CreateBSA(CreateBSADirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                    Directive::FromArchive(FromArchiveDirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                    Directive::InlineFile(InlineFileDirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                    Directive::PatchedFromArchive(PatchedFromArchiveDirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                    Directive::RemappedInlineFile(RemappedInlineFileDirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                    Directive::TransformedTexture(TransformedTextureDirective { hash, size, to, .. }) => (hash.clone(), size, to.clone()),
                }
                .pipe(|(hash, size, to)| (hash, *size, output_directory.join(to.into_path())))
                .pipe(move |(hash, size, to)| {
                    validate_hash_with_overrides(to.clone(), hash, size).map(move |res| {
                        res.tap_err(|reason| tracing::warn!(%kind, ?size, ?reason, ?to, "directive will be recomputed"))
                            .tap_ok(|path| tracing::info!(%kind, ?size, ?path, ?to,  "directive is ok"))
                            .is_err()
                    })
                })
            }
        };
        directives
            .pipe(futures::stream::iter)
            .map(move |directive| check_completed(&directive).map(move |validated| validated.then_some(directive)))
            .buffer_unordered(concurrency())
            .filter_map(ready)
            .collect::<Vec<_>>()
            .then(|directives| {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
                    .pipe(
                        |(mut create_bsa, mut from_archive, mut inline_file, mut patched_from_archive, mut remapped_inline_file, mut transformed_texture)| {
                            directives
                                .into_iter()
                                .for_each(|directive| match directive {
                                    Directive::CreateBSA(create_bsadirective) => create_bsa.push(create_bsadirective),
                                    Directive::FromArchive(from_archive_directive) => from_archive.push(from_archive_directive),
                                    Directive::InlineFile(inline_file_directive) => inline_file.push(inline_file_directive),
                                    Directive::PatchedFromArchive(patched_from_archive_directive) => patched_from_archive.push(patched_from_archive_directive),
                                    Directive::RemappedInlineFile(remapped_inline_file_directive) => remapped_inline_file.push(remapped_inline_file_directive),
                                    Directive::TransformedTexture(transformed_texture_directive) => transformed_texture.push(transformed_texture_directive),
                                })
                                .pipe(|_| {
                                    (
                                        create_bsa,
                                        from_archive,
                                        inline_file,
                                        patched_from_archive,
                                        remapped_inline_file,
                                        transformed_texture,
                                    )
                                })
                        },
                    )
                    .pipe(ready)
            })
            .into_stream()
            .flat_map(
                move |(create_bsa, from_archive, inline_file, patched_from_archive, remapped_inline_file, transformed_texture)| {
                    #[derive(derive_more::From, Clone)]
                    enum ArchivePathDirective {
                        FromArchive(FromArchiveDirective),
                        PatchedFromArchive(PatchedFromArchiveDirective),
                        TransformedTexture(TransformedTextureDirective),
                    }

                    impl ArchivePathDirective {
                        fn size(&self) -> u64 {
                            match self {
                                ArchivePathDirective::FromArchive(directive) => directive.size,
                                ArchivePathDirective::PatchedFromArchive(directive) => directive.size,
                                ArchivePathDirective::TransformedTexture(directive) => directive.size,
                            }
                        }
                    }

                    impl ArchivePathDirective {
                        fn archive_path(&self) -> &ArchiveHashPath {
                            match self {
                                ArchivePathDirective::FromArchive(f) => &f.archive_hash_path,
                                ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => &patched_from_archive_directive.archive_hash_path,
                                ArchivePathDirective::TransformedTexture(transformed_texture_directive) => &transformed_texture_directive.archive_hash_path,
                            }
                        }
                    }

                    futures::stream::empty()
                        .chain(
                            inline_file
                                .pipe(futures::stream::iter)
                                .map({
                                    cloned![manager];
                                    move |directive| {
                                        manager
                                            .clone()
                                            .inline_file
                                            .clone()
                                            .handle(directive.clone())
                                            .map(move |res| res.with_context(|| format!("handling directive [{directive:#?}]")))
                                    }
                                })
                                .buffer_unordered(concurrency()),
                        )
                        .chain(
                            std::iter::empty()
                                .chain(
                                    patched_from_archive
                                        .into_iter()
                                        .map(ArchivePathDirective::from),
                                )
                                .chain(from_archive.into_iter().map(ArchivePathDirective::from))
                                .chain(
                                    transformed_texture
                                        .into_iter()
                                        .map(ArchivePathDirective::from),
                                )
                                .sorted_unstable_by_key(|a| a.archive_path().clone())
                                .chunk_by(|a| a.archive_path().clone().parent().map(|(path, _)| path))
                                .into_iter()
                                .map(|(parent_archive, chunk)| (parent_archive, chunk.into_iter().collect_vec()))
                                .collect_vec()
                                .chunks(concurrency())
                                .map(|chunk| chunk.to_vec())
                                .collect_vec()
                                .pipe(futures::stream::iter)
                                .map({
                                    cloned![nested_archive_manager];
                                    cloned![manager];
                                    move |chunk| {
                                        let preheat = {
                                            cloned![nested_archive_manager];
                                            move |parent_archive: ArchiveHashPath| {
                                                cloned![nested_archive_manager];
                                                {
                                                    cloned![parent_archive];
                                                    async move {
                                                        nested_archive_manager
                                                            .clone()
                                                            .preheat(parent_archive.clone())
                                                            .await
                                                    }
                                                }
                                                .instrument(info_span!("preheating_archive", ?parent_archive))
                                            }
                                        };
                                        let cleanup = {
                                            cloned![nested_archive_manager];
                                            move |parent_archive: ArchiveHashPath| {
                                                cloned![nested_archive_manager];
                                                {
                                                    cloned![parent_archive];

                                                    async move {
                                                        nested_archive_manager
                                                            .clone()
                                                            .cleanup(parent_archive.clone())
                                                            .await
                                                    }
                                                }
                                                .instrument(info_span!("cleaning_up", ?parent_archive))
                                            }
                                        };

                                        let parent_chunk = chunk
                                            .iter()
                                            .filter_map(|(parent, _)| parent.clone())
                                            .collect_vec();
                                        let preheat_all = {
                                            cloned![parent_chunk];
                                            move || async move {
                                                parent_chunk
                                                    .pipe(futures::stream::iter)
                                                    .map(&preheat)
                                                    .buffer_unordered(concurrency().div(4).max(1))
                                                    .try_collect::<()>()
                                                    .await
                                                    .context("preheating chunk")
                                            }
                                        };
                                        let cleanup_all = {
                                            cloned![parent_chunk];
                                            move || async move {
                                                parent_chunk
                                                    .pipe(futures::stream::iter)
                                                    .map(&cleanup)
                                                    .buffer_unordered(concurrency().div(4).max(1))
                                                    .collect::<()>()
                                                    .map(anyhow::Ok)
                                                    .await
                                                    .context("preheating chunk")
                                            }
                                        };
                                        preheat_all()
                                            .into_stream()
                                            .try_flat_map({
                                                cloned![manager];
                                                move |_| {
                                                    chunk
                                                        .to_vec()
                                                        .pipe(futures::stream::iter)
                                                        .flat_map(|(_, chunk)| futures::stream::iter(chunk))
                                                        .map(move |directive| match directive {
                                                            ArchivePathDirective::TransformedTexture(transformed_texture) => manager
                                                                .transformed_texture
                                                                .clone()
                                                                .handle(transformed_texture.clone())
                                                                .map(move |res| res.with_context(|| format!("handling directive: {transformed_texture:#?}")))
                                                                .boxed_local(),
                                                            ArchivePathDirective::FromArchive(from_archive) => manager
                                                                .from_archive
                                                                .clone()
                                                                .handle(from_archive.clone())
                                                                .map(move |res| res.with_context(|| format!("handling directive: {from_archive:#?}")))
                                                                .boxed_local(),
                                                            ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => manager
                                                                .patched_from_archive
                                                                .clone()
                                                                .handle(patched_from_archive_directive.clone())
                                                                .map(move |res| {
                                                                    res.with_context(|| format!("handling directive: {patched_from_archive_directive:#?}"))
                                                                })
                                                                .boxed_local(),
                                                        })
                                                        .buffer_unordered(concurrency().div(4).max(1))
                                                }
                                            })
                                            .chain(cleanup_all().map_ok(|_| 0).into_stream())
                                            .try_fold(0, |acc, next| ready(Ok(acc + next)))
                                    }
                                })
                                .buffer_unordered(4.min(concurrency())),
                        )
                        .chain(remapped_inline_file.pipe(futures::stream::iter).then({
                            cloned![manager];
                            move |remapped_inline_file| {
                                manager
                                    .remapped_inline_file
                                    .clone()
                                    .handle(remapped_inline_file.clone())
                                    .map(move |res| res.with_context(|| format!("handling {remapped_inline_file:#?}")))
                            }
                        }))
                        .chain(create_bsa.pipe(futures::stream::iter).then({
                            cloned![manager];
                            move |create_bsa| {
                                let debug = format!("{create_bsa:#?}")
                                    .chars()
                                    .take(256)
                                    .collect::<String>();
                                manager
                                    .create_bsa
                                    .clone()
                                    .handle(create_bsa)
                                    .map(move |res| res.with_context(|| format!("handling directive: [{debug}]")))
                            }
                        }))
                        .map_ok({
                            cloned![pb];
                            move |size| {
                                pb.inc(size);
                            }
                        })
                },
            )
    }
}
