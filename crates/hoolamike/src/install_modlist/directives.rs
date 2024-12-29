use {
    crate::{
        downloaders::WithArchiveDescriptor,
        install_modlist::{download_cache::validate_hash, io_progress_style},
        modlist_json::{
            directive::{
                create_bsa_directive::{CreateBSADirective, CreateBSADirectiveKind},
                ArchiveHashPath,
                FromArchiveDirective,
                InlineFileDirective,
                PatchedFromArchiveDirective,
                RemappedInlineFileDirective,
                TransformedTextureDirective,
            },
            DirectiveKind,
        },
        utils::{MaybeWindowsPath, PathReadWrite},
    },
    anyhow::{Context, Result},
    futures::{FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt},
    itertools::Itertools,
    nonempty::NonEmpty,
    num::ToPrimitive,
    parking_lot::Mutex,
    queued_archive_task::QueuedArchiveService,
    remapped_inline_file::RemappingContext,
    std::{
        collections::BTreeMap,
        future::ready,
        ops::Mul,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tracing::{info_span, instrument, Instrument},
    tracing_indicatif::span_ext::IndicatifSpanExt,
};

pub(crate) fn create_file_all(path: &Path) -> Result<std::fs::File> {
    path.parent()
        .map(|parent| std::fs::create_dir_all(parent).with_context(|| format!("creating directory for [{}]", parent.display())))
        .unwrap_or_else(|| Ok(()))
        .and_then(|_| path.open_file_write())
        .map(|(_, f)| f)
}

pub type DownloadSummary = Arc<BTreeMap<String, WithArchiveDescriptor<PathBuf>>>;

pub mod create_bsa;
pub mod from_archive;
pub mod inline_file;
pub mod patched_from_archive;
pub mod remapped_inline_file;
pub mod transformed_texture;

use crate::modlist_json::Directive;

pub type WabbajackFileHandle = Arc<Mutex<crate::compression::compress_tools::ArchiveHandle>>;

#[extension_traits::extension(pub trait WabbajackFileHandleExt)]
impl WabbajackFileHandle {
    fn from_archive(archive: crate::compression::compress_tools::ArchiveHandle) -> Self {
        Arc::new(Mutex::new(archive))
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
    pub download_summary: DownloadSummary,
    pub archive_extraction_queue: Arc<QueuedArchiveService>,
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

#[extension_traits::extension(pub trait StreamTryFlatMapLocalExt)]
impl<'iter, T, E, I> I
where
    E: 'iter,
    T: 'iter,
    I: Stream<Item = Result<T, E>> + 'iter,
{
    fn try_flat_map_local<U, NewStream, F>(self, try_flat_map: F) -> impl Stream<Item = Result<U, E>> + 'iter
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
#[extension_traits::extension(pub trait StreamTryFlatMapExt)]
impl<'iter, T, E, I> I
where
    E: 'static + Send + Sync,
    T: 'static + Send + Sync,
    I: Stream<Item = Result<T, E>> + 'static + Unpin,
{
    fn try_flat_map<U, NewStream, F>(self, try_flat_map: F) -> impl Stream<Item = Result<U, E>> + Unpin
    where
        U: 'static + Send + Sync,
        NewStream: Stream<Item = Result<U, E>> + 'iter,
        F: FnOnce(T) -> NewStream + 'iter + Clone,
    {
        self.flat_map(move |e| match e {
            Ok(value) => value.pipe(try_flat_map.clone()).boxed_local(),
            Err(e) => e.pipe(Err).pipe(ready).pipe(futures::stream::once).boxed(),
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

#[derive(derive_more::From, Clone, Debug)]
enum ArchivePathDirective {
    FromArchive(FromArchiveDirective),
    PatchedFromArchive(PatchedFromArchiveDirective),
    TransformedTexture(TransformedTextureDirective),
}

impl ArchivePathDirective {
    fn directive_size(&self) -> u64 {
        match self {
            ArchivePathDirective::FromArchive(d) => d.size,
            ArchivePathDirective::PatchedFromArchive(d) => d.size,
            ArchivePathDirective::TransformedTexture(d) => d.size,
        }
    }
    fn archive_path(&self) -> &ArchiveHashPath {
        match self {
            ArchivePathDirective::FromArchive(f) => &f.archive_hash_path,
            ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => &patched_from_archive_directive.archive_hash_path,
            ArchivePathDirective::TransformedTexture(transformed_texture_directive) => &transformed_texture_directive.archive_hash_path,
        }
    }
}

pub mod queued_archive_task;

fn handle_nested_archive_directives(
    manager: Arc<DirectivesHandler>,
    archive_extraction_queue: Arc<QueuedArchiveService>,
    directives: Vec<ArchivePathDirective>,
    concurrency: usize,
) -> impl Stream<Item = Result<u64>> {
    let handle_directives = tracing::Span::current();
    let preheat = {
        cloned![archive_extraction_queue];
        move |paths: Vec<NonEmpty<PathBuf>>| {
            cloned![archive_extraction_queue];
            async move {
                paths
                    .into_iter()
                    .collect_vec()
                    .pipe(|paths| {
                        tracing::info!("preheating [{}] archives", paths.len());
                        paths
                            .into_iter()
                            .map(|path| archive_extraction_queue.clone().get_archive_spawn(path))
                            .collect_vec()
                            .pipe(|tasks| {
                                futures::stream::iter(tasks)
                                    .buffered(2137)
                                    .for_each(|_| ready(()))
                            })
                    })
                    .await
            }
        }
    };

    directives
        .tap_mut(|directives| {
            // sorting
            directives.sort_by_cached_key(|archive| {
                // this is so that progress is easier on the eye
                archive.archive_path().clone()
            });
        })
        .tap({
            cloned![preheat, manager];
            move |directives| {
                cloned![preheat, manager];
                let preheat_subpaths_of_len = move |directives: &[ArchivePathDirective], len: usize, slice: usize| {
                    // preheat
                    directives
                        .iter()
                        .map(|directive| {
                            directive
                                .archive_path()
                                .clone()
                                .pipe(|path| manager.download_summary.resolve_archive_path(path.clone()))
                        })
                        .filter_map(|res| {
                            res.tap_err(|reason| {
                                tracing::error!(?reason, "the the program will continue, but it will likely not finish");
                            })
                            .ok()
                        })
                        .filter_map(|path| {
                            path.len()
                                .eq(&len)
                                .then(|| path.tap_mut(|path| path.truncate(slice)))
                        })
                        .collect_vec()
                        .pipe(|paths| {
                            cloned![preheat];
                            preheat(paths)
                        })
                };
                [(3, 2), (4, 2), (4, 3), (2, 2)]
                    .into_iter()
                    .map({
                        cloned![preheat_subpaths_of_len];
                        let directives = directives.clone();
                        move |(len, slice)| preheat_subpaths_of_len(&directives, len, slice)
                    })
                    .pipe(|tasks| {
                        tokio::task::spawn(async move {
                            for task in tasks {
                                task.await;
                            }
                        })
                    });
            }
        })
        .pipe(futures::stream::iter)
        .map(move |directive| match directive {
            ArchivePathDirective::TransformedTexture(transformed_texture) => manager
                .transformed_texture
                .clone()
                .handle(transformed_texture.clone())
                .instrument(handle_directives.clone())
                .map(move |res| res.with_context(|| format!("handling directive: {transformed_texture:#?}")))
                .boxed(),
            ArchivePathDirective::FromArchive(from_archive) => manager
                .from_archive
                .clone()
                .handle(from_archive.clone())
                .instrument(handle_directives.clone())
                .map(move |res| res.with_context(|| format!("handling directive: {from_archive:#?}")))
                .boxed(),
            ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => manager
                .patched_from_archive
                .clone()
                .handle(patched_from_archive_directive.clone())
                .instrument(handle_directives.clone())
                .map(move |res| res.with_context(|| format!("handling directive: {patched_from_archive_directive:#?}")))
                .boxed(),
        })
        .buffer_unordered(concurrency)
}

// /// it's dirty as hell but saves disk space
// fn handle_nested_archive_directives(
//     manager: Arc<DirectivesHandler>,
//     nested_archive_manager: Arc<NestedArchivesService>,
//     directives: Vec<ArchivePathDirective>,
//     concurrency: usize,
// ) -> impl Stream<Item = ArcResult<u64>> {
//     pub const CHUNK_SIZE_BYTES: u64 = 64 * 1024 * 1024;
//     let handle_directives: &'static _ = tracing::Span::current().pipe(Box::new).pipe(Box::leak);
//     directives
//         .into_iter()
//         .sorted_unstable_by_key(|a| a.archive_path().clone())
//         .chunk_by(|a| a.archive_path().clone().parent().map(|(path, _)| path))
//         .into_iter()
//         .map(|(parent_archive, chunk)| (parent_archive, chunk.into_iter().collect_vec()))
//         .collect_vec()
//         .into_iter()
//         .fold(vec![vec![]], |acc: Vec<Vec<(_, Vec<ArchivePathDirective>)>>, next| {
//             acc.tap_mut(|acc| {
//                 if acc
//                     .last()
//                     .unwrap()
//                     .iter()
//                     .map(|(_, d)| d.iter().map(|d| d.directive_size()).sum::<u64>())
//                     .sum::<u64>()
//                     > CHUNK_SIZE_BYTES
//                 {
//                     acc.push(vec![]);
//                 }
//                 acc.last_mut().unwrap().push(next);
//             })
//         })
//         .into_iter()
//         .collect_vec()
//         .pipe(futures::stream::iter)
//         .pipe(Box::pin)
//         .flat_map_unordered(concurrency.div(4).max(1), {
//             cloned![nested_archive_manager];
//             cloned![manager];
//             move |chunk| {
//                 let preheat = {
//                     cloned![nested_archive_manager];
//                     cloned![handle_directives];
//                     move |parent_archive: ArchiveHashPath| {
//                         cloned![nested_archive_manager];
//                         {
//                             cloned![parent_archive];
//                             cloned![handle_directives];
//                             async move {
//                                 nested_archive_manager
//                                     .clone()
//                                     .preheat(parent_archive.clone())
//                                     .boxed()
//                                     .instrument(handle_directives.clone())
//                                     .await
//                             }
//                             .boxed()
//                         }
//                         .instrument(info_span!("preheating_archive", ?parent_archive))
//                     }
//                 };
//                 let cleanup = {
//                     cloned![nested_archive_manager];
//                     move |parent_archive: ArchiveHashPath| {
//                         cloned![nested_archive_manager];
//                         {
//                             cloned![parent_archive];

//                             async move {
//                                 nested_archive_manager
//                                     .clone()
//                                     .cleanup(parent_archive.clone())
//                                     .instrument(handle_directives.clone())
//                                     .await
//                             }
//                         }
//                         .instrument(info_span!("cleaning_up", ?parent_archive))
//                     }
//                 };

//                 let parent_chunk = chunk
//                     .iter()
//                     .filter_map(|(parent, _)| parent.clone())
//                     .collect_vec();
//                 let preheat_all = {
//                     cloned![parent_chunk];
//                     cloned![preheat];
//                     move || async move {
//                         parent_chunk
//                             .pipe(futures::stream::iter)
//                             .map(preheat.clone())
//                             .buffer_unordered(concurrency.div(4).max(1))
//                             .try_collect::<()>()
//                             .await
//                             .context("preheating chunk")
//                             .arced()
//                     }
//                 };
//                 let cleanup_all = {
//                     cloned![parent_chunk];
//                     cloned![cleanup];
//                     move || async move {
//                         parent_chunk
//                             .pipe(futures::stream::iter)
//                             .map(cleanup.clone())
//                             .buffer_unordered(concurrency.div(4).max(1))
//                             .collect::<()>()
//                             .map(anyhow::Ok)
//                             .await
//                             .context("preheating chunk")
//                             .arced()
//                     }
//                 };
//                 preheat_all()
//                     .boxed()
//                     .into_stream()
//                     .try_flat_map({
//                         cloned![manager, chunk];
//                         move |_| {
//                             chunk
//                                 .pipe(futures::stream::iter)
//                                 .flat_map(|(_, chunk)| futures::stream::iter(chunk))
//                                 .map(move |directive| match directive {
//                                     ArchivePathDirective::TransformedTexture(transformed_texture) => manager
//                                         .transformed_texture
//                                         .clone()
//                                         .handle(transformed_texture.clone())
//                                         .instrument(handle_directives.clone())
//                                         .map(move |res| {
//                                             res.with_context(|| format!("handling directive: {transformed_texture:#?}"))
//                                                 .arced()
//                                         })
//                                         .boxed(),
//                                     ArchivePathDirective::FromArchive(from_archive) => manager
//                                         .from_archive
//                                         .clone()
//                                         .handle(from_archive.clone())
//                                         .instrument(handle_directives.clone())
//                                         .map(move |res| {
//                                             res.with_context(|| format!("handling directive: {from_archive:#?}"))
//                                                 .arced()
//                                         })
//                                         .boxed(),
//                                     ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => manager
//                                         .patched_from_archive
//                                         .clone()
//                                         .handle(patched_from_archive_directive.clone())
//                                         .instrument(handle_directives.clone())
//                                         .map(move |res| {
//                                             res.with_context(|| format!("handling directive: {patched_from_archive_directive:#?}"))
//                                                 .arced()
//                                         })
//                                         .boxed(),
//                                 })
//                                 .buffered(concurrency.div(2).max(1))
//                         }
//                     })
//                     .chain(cleanup_all().boxed().map_ok(|_| 0).into_stream())
//             }
//         })
// }

#[extension_traits::extension(pub (crate) trait ResolvePathExt)]
impl DownloadSummary {
    fn resolve_archive_path(&self, ArchiveHashPath { source_hash, path }: ArchiveHashPath) -> Result<NonEmpty<PathBuf>> {
        self.get(&source_hash)
            .with_context(|| format!("no [{source_hash}] in downloads"))
            .map(|parent| NonEmpty::new(parent.inner.clone()).tap_mut(|resolved| resolved.extend(path.into_iter().map(MaybeWindowsPath::into_path))))
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
        let archive_extraction_queue = queued_archive_task::QueuedArchiveService::new(num_cpus::get());

        Self {
            config,
            create_bsa: create_bsa::CreateBSAHandler {
                output_directory: output_directory.clone(),
            },
            from_archive: from_archive::FromArchiveHandler {
                output_directory: output_directory.clone(),
                download_summary: download_summary.clone(),
                archive_extraction_queue: archive_extraction_queue.clone(),
            },
            inline_file: inline_file::InlineFileHandler {
                wabbajack_file: wabbajack_file.clone(),
                output_directory: output_directory.clone(),
            },
            patched_from_archive: patched_from_archive::PatchedFromArchiveHandler {
                output_directory: output_directory.clone(),
                wabbajack_file: wabbajack_file.clone(),
                download_summary: download_summary.clone(),
                archive_extraction_queue: archive_extraction_queue.clone(),
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
                download_summary: download_summary.clone(),
                archive_extraction_queue: archive_extraction_queue.clone(),
            },
            download_summary,
            archive_extraction_queue,
        }
    }

    #[allow(clippy::unnecessary_literal_unwrap)]
    #[instrument(skip_all, fields(directives=%directives.len()))]
    pub fn handle_directives(self: Arc<Self>, directives: Vec<Directive>) -> impl Stream<Item = Result<u64>> {
        let handle_directives: &'static _ = tracing::Span::current()
            .tap(|pb| {
                pb.pb_set_length(directives.iter().map(directive_size).sum());
                pb.pb_set_style(&io_progress_style());
            })
            .pipe(Box::new)
            .pipe(Box::leak);

        fn directive_size(d: &Directive) -> u64 {
            match d {
                Directive::CreateBSA(directive) => directive.size(),
                Directive::FromArchive(directive) => directive.size,
                Directive::InlineFile(directive) => directive.size,
                Directive::PatchedFromArchive(directive) => directive.size,
                Directive::RemappedInlineFile(directive) => directive.size,
                Directive::TransformedTexture(directive) => directive.size,
            }
        }
        let manager = self.clone();

        enum DirectiveStatus {
            Completed(u64),
            NeedsRebuild { reason: anyhow::Error, directive: Directive },
        }

        let check_completed = {
            let output_directory = self.from_archive.output_directory.clone();
            move |directive: Directive| {
                let _kind = DirectiveKind::from(&directive);
                match &directive {
                    Directive::CreateBSA(create_bsa) => match create_bsa {
                        CreateBSADirective::Bsa(CreateBSADirectiveKind { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                        CreateBSADirective::Ba2(CreateBSADirectiveKind { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                    },
                    Directive::FromArchive(FromArchiveDirective { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                    Directive::InlineFile(InlineFileDirective { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                    Directive::PatchedFromArchive(PatchedFromArchiveDirective { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                    Directive::RemappedInlineFile(RemappedInlineFileDirective { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                    Directive::TransformedTexture(TransformedTextureDirective { hash, size, to, .. }) => (hash.clone(), *size, to.clone()),
                }
                .pipe(|(hash, size, to)| (hash, size, output_directory.join(to.into_path())))
                .pipe(move |(hash, size, to)| {
                    validate_hash_with_overrides(to.clone(), hash, size)
                        .map(move |res| match res {
                            Ok(_) => DirectiveStatus::Completed(size),
                            Err(reason) => DirectiveStatus::NeedsRebuild { reason, directive },
                        })
                        .instrument(handle_directives.clone())
                })
            }
        };
        directives
            .pipe(futures::stream::iter)
            .map(check_completed)
            .buffer_unordered(num_cpus::get())
            .collect::<Vec<_>>()
            .then(|directives| {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
                    .pipe(
                        |(
                            mut create_bsa,
                            mut from_archive,
                            mut inline_file,
                            mut patched_from_archive,
                            mut remapped_inline_file,
                            mut transformed_texture,
                            mut completed,
                        )| {
                            directives
                                .into_iter()
                                .for_each(|directive| match directive {
                                    DirectiveStatus::Completed(size) => completed.push(size),
                                    DirectiveStatus::NeedsRebuild { reason, directive } => {
                                        tracing::trace!("recomputing directive:\n{reason:?}");
                                        match directive {
                                            Directive::CreateBSA(create_bsadirective) => create_bsa.push(create_bsadirective),
                                            Directive::FromArchive(from_archive_directive) => from_archive.push(from_archive_directive),
                                            Directive::InlineFile(inline_file_directive) => inline_file.push(inline_file_directive),
                                            Directive::PatchedFromArchive(patched_from_archive_directive) => {
                                                patched_from_archive.push(patched_from_archive_directive)
                                            }
                                            Directive::RemappedInlineFile(remapped_inline_file_directive) => {
                                                remapped_inline_file.push(remapped_inline_file_directive)
                                            }
                                            Directive::TransformedTexture(transformed_texture_directive) => {
                                                transformed_texture.push(transformed_texture_directive)
                                            }
                                        }
                                    }
                                })
                                .pipe(|_| {
                                    (
                                        create_bsa,
                                        from_archive,
                                        inline_file,
                                        patched_from_archive,
                                        remapped_inline_file,
                                        transformed_texture,
                                        completed,
                                    )
                                })
                        },
                    )
                    .pipe(ready)
            })
            .into_stream()
            .flat_map(
                move |(create_bsa, from_archive, inline_file, patched_from_archive, remapped_inline_file, transformed_texture, completed)| {
                    futures::stream::empty()
                        .chain(completed.pipe(futures::stream::iter).map(Ok))
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
                                            .instrument(handle_directives.clone())
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
                                .collect_vec()
                                .pipe(|directives| {
                                    handle_directives.in_scope(|| {
                                        handle_nested_archive_directives(manager.clone(), self.archive_extraction_queue.clone(), directives, concurrency() * 10)
                                    })
                                }),
                        )
                        .chain(remapped_inline_file.pipe(futures::stream::iter).then({
                            cloned![manager];
                            move |remapped_inline_file| {
                                manager
                                    .remapped_inline_file
                                    .clone()
                                    .handle(remapped_inline_file.clone())
                                    .instrument(handle_directives.clone())
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
                                    .instrument(handle_directives.clone())
                                    .map(move |res| res.with_context(|| format!("handling directive: [{debug}]")))
                            }
                        }))
                        .inspect_ok({
                            move |size| {
                                handle_directives.pb_inc(*size);
                            }
                        })
                },
            )
    }
}
