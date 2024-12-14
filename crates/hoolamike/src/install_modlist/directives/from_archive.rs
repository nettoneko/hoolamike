use {
    super::*,
    crate::{
        install_modlist::download_cache::{to_u64_from_base_64, validate_file_size, validate_hash},
        modlist_json::directive::FromArchiveDirective,
        progress_bars::{vertical_progress_bar, ProgressKind, PROGRESS_BAR},
        read_wrappers::ReadExt,
    },
    nested_archive_manager::NestedArchivesService,
    std::{
        convert::identity,
        io::{Read, Seek, Write},
        path::Path,
    },
    tokio::sync::Mutex,
};

#[derive(Clone, Debug)]
pub struct FromArchiveHandler {
    pub nested_archive_service: Arc<Mutex<NestedArchivesService>>,
    pub output_directory: PathBuf,
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

async fn validate_hash_with_overrides(path: PathBuf, hash: String, size: u64) -> Result<PathBuf> {
    match is_whitelisted_by_path(&path) {
        true => validate_file_size(path, size).await,
        false => validate_hash(path, hash).await,
    }
}

impl FromArchiveHandler {
    #[tracing::instrument(skip(self))]
    pub async fn handle(
        self,
        FromArchiveDirective {
            hash,
            size,
            to,
            archive_hash_path,
        }: FromArchiveDirective,
    ) -> Result<()> {
        let output_path = self.output_directory.join(to.into_path());

        if let Err(message) = validate_hash_with_overrides(output_path.clone(), hash.clone(), size).await {
            let source_file = self
                .nested_archive_service
                .lock()
                .await
                .get(archive_hash_path.clone())
                .await
                .context("could not get a handle to archive")?;

            tokio::task::spawn_blocking(move || -> Result<_> {
                let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndClear)
                    .attach_to(&PROGRESS_BAR)
                    .tap_mut(|pb| {
                        pb.set_message(output_path.display().to_string());
                    });
                let perform_copy = move |from: &mut dyn Read, to: &mut dyn Write, target_path: PathBuf| {
                    let mut writer = to;
                    let mut reader: Box<dyn Read> = match is_whitelisted_by_path(&target_path) {
                        true => pb.wrap_read(from).and_validate_size(size).pipe(Box::new),
                        false => pb
                            .wrap_read(from)
                            .and_validate_size(size)
                            .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on"))
                            .pipe(Box::new),
                    };
                    std::io::copy(
                        &mut reader,
                        // WARN: hashes are not gonna match for bsa stuff because we write headers differentlys
                        // .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on")),
                        &mut writer,
                    )
                    .context("copying file from archive")
                    .and_then(|_| writer.flush().context("flushing write"))
                    .map(|_| ())
                };

                match source_file {
                    nested_archive_manager::HandleKind::Cached(file) => file
                        .inner
                        .1
                        .try_clone()
                        .context("cloning file")
                        .and_then(|mut f| f.rewind().context("rewinding").map(|_| f)),
                    nested_archive_manager::HandleKind::JustHashPath(source_file_path) => std::fs::OpenOptions::new()
                        .read(true)
                        .open(&source_file_path)
                        .with_context(|| format!("opening [{}]", source_file_path.display())),
                }
                .and_then(|mut final_source| {
                    create_file_all(&output_path).and_then(|mut output_file| {
                        perform_copy(&mut final_source, &mut output_file, output_path.clone())
                            .with_context(|| format!("when extracting from [{:?}] to [{}]", archive_hash_path, output_path.display()))
                    })
                })?;
                Ok(())
            })
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(())
    }
}
