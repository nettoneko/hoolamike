#![allow(clippy::option_map_unit_fn)]

pub use which;
use {
    anyhow::{anyhow, Context, Result},
    list_output::{ListOutput, ListOutputEntry},
    std::{
        collections::BTreeMap,
        iter::once,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        str::FromStr,
        sync::Arc,
    },
    tap::prelude::*,
    tempfile::TempPath,
    tracing::instrument,
};

#[derive(Clone, Debug)]
pub struct Wrapped7Zip {
    bin: Arc<Path>,
    temp_files_dir: Arc<Path>,
    thread_count: Option<usize>,
}

fn check_exists(file: &Path) -> Result<&Path> {
    file.try_exists()
        .context("checking for existance of the provided binary")
        .and_then(|exists| exists.then_some(file).context("checking if file exists"))
        .with_context(|| format!("checking if file exists: [{}]", file.display()))
}

impl Wrapped7Zip {
    pub fn new(bin: &Path, temp_files_dir: &Path) -> Result<Self> {
        Self::with_thread_count(bin, temp_files_dir, None)
    }

    pub fn with_thread_count(bin: &Path, temp_files_dir: &Path, thread_count: Option<usize>) -> Result<Self> {
        check_exists(bin)
            .context("checking if binary exists")
            .map(Arc::from)
            .map(|bin| Self {
                bin,
                temp_files_dir: Arc::from(temp_files_dir),
                thread_count,
            })
            .with_context(|| format!("instantiating wrapper at [{}]", bin.display()))
    }
}

#[derive(Debug)]
pub struct ArchiveHandle {
    binary: Wrapped7Zip,
    archive: PathBuf,
}

#[extension_traits::extension(pub trait CommandExt)]
impl Command {
    fn command_debug(&self) -> String {
        let command = self.get_program().to_string_lossy().to_string();
        self.get_args()
            .map(|a| a.to_string_lossy().to_string())
            .pipe(|args| once(command).chain(args).collect::<Vec<_>>())
            .join(" ")
    }
    fn read_stdout_ok(mut self) -> Result<String> {
        let dbg = self.command_debug();
        self.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("spawning command")
            .and_then(|Output { status, stdout, stderr }| {
                status
                    .success()
                    .then_some(())
                    .ok_or_else(|| status.code().unwrap_or(-1))
                    .map_err(|code| anyhow!("command failed with status [{code}]"))
                    .with_context(|| String::from_utf8_lossy(&stderr).to_string())
                    .and_then(|_| {
                        stdout
                            .pipe(String::from_utf8)
                            .context("output is not a string")
                    })
            })
            .with_context(|| format!("when executing [{dbg}]"))
    }
}

impl Wrapped7Zip {
    fn command<F: FnMut(&mut Command) -> &mut Command>(&self, mut build_command: F) -> Command {
        let mut command = Command::new(self.bin.as_ref());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        // Add thread count option if specified
        if let Some(count) = self.thread_count {
            command.arg(format!("-mmt{}", count));
        }
        // command.kill_on_drop(true);
        build_command(&mut command);
        command
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn query_file_info(&self, path: &Path) -> Result<String> {
        path.try_exists()
            .context("checking for file existence")
            .and_then(|exists| exists.then_some(path).context("path does not exist"))
            .map(|path| self.command(|c| c.arg("l").arg(path)))
            .and_then(|command| command.read_stdout_ok())
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn open_file(&self, archive: &Path) -> Result<ArchiveHandle> {
        self.query_file_info(archive)
            .map(|_| archive)
            .map(|archive| ArchiveHandle {
                binary: self.clone(),
                archive: archive.into(),
            })
    }

    pub fn find_bin(temp_files_dir: &Path, thread_count: Option<usize>) -> Result<Self> {
        ["7z", "7z.exe"]
            .into_iter()
            .find_map(|bin| which::which(bin).ok())
            .context("no 7z binary")
            .and_then(|bin| Self::with_thread_count(&bin, temp_files_dir, thread_count))
    }
}

// thread_local! {
//     pub static WRAPPED_7ZIP: Arc<Wrapped7Zip> = Arc::new(Wrapped7Zip::find_bin().expect("no 7z found, fix your dependencies"));
// }

pub struct ArchiveFileHandle {
    pub path: TempPath,
    pub file: std::fs::File,
}

pub mod list_output;

#[derive(Debug, PartialEq, PartialOrd, Hash)]
pub(crate) struct MaybeWindowsPath(pub String);

impl MaybeWindowsPath {
    pub fn into_path(self) -> PathBuf {
        let s = self.0;
        let s = match s.contains("\\\\") {
            true => s.split("\\\\").collect::<Vec<_>>().join("/"),
            false => s,
        };
        let s = match s.contains("\\") {
            true => s.split("\\").collect::<Vec<_>>().join("/"),
            false => s,
        };
        PathBuf::from(s)
    }
}

impl ArchiveHandle {
    #[instrument]
    pub fn list_files(&self) -> Result<Vec<ListOutputEntry>> {
        self.binary
            .command(|c| {
                c.arg("l")
                    // more parsing-friendly output
                    .arg("-slt")
                    .arg(&self.archive)
            })
            .read_stdout_ok()
            .and_then(|o| list_output::ListOutput::from_str(&o).with_context(|| format!("unexpected output from list command:\n{o}")))
            .map(|ListOutput { entries }| entries)
    }

    #[instrument]
    pub fn get_many_handles(&self, paths: &[&Path]) -> Result<Vec<(ListOutputEntry, ArchiveFileHandle)>> {
        let mut lookup = paths
            .iter()
            .copied()
            .map(|p| (p.display().to_string().to_lowercase(), p))
            .collect::<BTreeMap<_, _>>();
        tempfile::tempdir_in(&self.binary.temp_files_dir)
            .context("creating temporary directory")
            .map(|temp_dir| temp_dir.into_path())
            .and_then(|temp_dir| {
                self.list_files()
                    .map(|files| {
                        files
                            .into_iter()
                            .filter_map(|entry| {
                                lookup
                                    .remove(&entry.path.display().to_string().to_lowercase())
                                    .map(|_| entry)
                            })
                            .collect::<Vec<_>>()
                    })
                    .and_then(|entries| {
                        lookup
                            .is_empty()
                            .then_some(entries)
                            .with_context(|| format!("some paths were not found: {lookup:#?}"))
                    })
                    .and_then(|entries| {
                        self.binary
                            .command(|c| c.arg("x").arg(&self.archive))
                            .pipe(|c| {
                                let mut c = entries.iter().fold(c, |c, entry| {
                                    c.tap_mut(|c| {
                                        c.arg(&entry.original_path);
                                    })
                                });
                                c.arg(format!("-o{}", temp_dir.display()));
                                c.arg(&temp_dir);
                                c
                            })
                            .read_stdout_ok()
                            .tap_ok(|res| tracing::debug!(%res))
                            .and_then(|_| {
                                entries
                                    .into_iter()
                                    .map(|e| {
                                        let path = temp_dir.join(&e.original_path).pipe(TempPath::from_path);
                                        let file = std::fs::File::open(&path).with_context(|| {
                                            format!(
                                                "no file was created for entry [{path:?}]\n(found paths: [{:#?}])",
                                                std::fs::read_dir(&temp_dir).unwrap().collect::<Vec<_>>()
                                            )
                                        });
                                        file.map(|file| (e, ArchiveFileHandle { path, file }))
                                    })
                                    .collect::<Result<Vec<_>>>()
                                    .context("some files were not created")
                            })
                    })
            })
    }
    #[instrument]
    pub fn get_file(&self, file: &Path) -> Result<(ListOutputEntry, ArchiveFileHandle)> {
        self.get_many_handles(&[file])
            .and_then(|file| file.into_iter().next().context("empty output"))
    }
}

#[cfg(test)]
mod tests;
