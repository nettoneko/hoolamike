use {
    super::{
        preheat_archive_hash_paths::PreheatedArchiveHashPaths,
        ArchivePathDirective,
        DirectivesHandler,
        DownloadSummary,
        FutureAnyhowExt,
        ResolvePathExt,
        StreamTryFlatMapExt,
    },
    anyhow::{Context, Result},
    futures::{FutureExt, Stream, StreamExt, TryFutureExt},
    std::{future::ready, sync::Arc},
    tap::prelude::*,
    tracing::{info_span, instrument, Instrument},
};

#[instrument(skip_all)]
pub(crate) fn handle_nested_archive_directives(
    manager: Arc<DirectivesHandler>,
    download_summary: DownloadSummary,
    directives: Vec<ArchivePathDirective>,
    concurrency: usize,
) -> impl Stream<Item = Result<u64>> {
    let preheat_task = {
        let preheat_directives = info_span!("preheat_directives");
        directives
            .iter()
            .map(|d| d.archive_path())
            .map(|path| download_summary.resolve_archive_path(path))
            .collect::<Result<Vec<_>>>()
            .pipe(ready)
            .and_then(|paths| {
                tokio::task::spawn_blocking(move || preheat_directives.in_scope(|| PreheatedArchiveHashPaths::preheat_archive_hash_paths(paths)))
                    .map_context("thread crashed")
                    .and_then(ready)
            })
    };
    let handle_directives = info_span!("handle_directives");
    preheat_task
        .map_ok(Arc::new)
        .into_stream()
        .try_flat_map(move |preheated| {
            directives
                .pipe(futures::stream::iter)
                .map(move |directive| match directive {
                    ArchivePathDirective::TransformedTexture(transformed_texture) => manager
                        .transformed_texture
                        .clone()
                        .handle(transformed_texture.clone(), preheated.clone())
                        .instrument(handle_directives.clone())
                        .map(move |res| res.with_context(|| format!("handling directive: {transformed_texture:#?}")))
                        .boxed(),
                    ArchivePathDirective::FromArchive(from_archive) => manager
                        .from_archive
                        .clone()
                        .handle(from_archive.clone(), preheated.clone())
                        .instrument(handle_directives.clone())
                        .map(move |res| res.with_context(|| format!("handling directive: {from_archive:#?}")))
                        .boxed(),
                    ArchivePathDirective::PatchedFromArchive(patched_from_archive_directive) => manager
                        .patched_from_archive
                        .clone()
                        .handle(patched_from_archive_directive.clone(), preheated.clone())
                        .instrument(handle_directives.clone())
                        .map(move |res| res.with_context(|| format!("handling directive: {patched_from_archive_directive:#?}")))
                        .boxed(),
                })
                .buffer_unordered(concurrency)
        })
}
