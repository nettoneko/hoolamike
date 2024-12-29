#![allow(clippy::option_map_unit_fn)]

pub use which;
use {
    anyhow::{anyhow, Context, Result},
    list_output::{ListOutput, ListOutputEntry},
    parking_lot::Mutex,
    std::{
        collections::BTreeMap,
        io::{BufReader, Read},
        iter::once,
        path::{Path, PathBuf},
        process::{Child, ChildStdout, Command, Output, Stdio},
        str::FromStr,
        sync::{
            mpsc::{self, Receiver, Sender},
            Arc,
        },
    },
    tap::prelude::*,
};

#[derive(Clone, Debug)]
pub struct Wrapped7Zip(Arc<Path>);

fn check_exists(file: &Path) -> Result<&Path> {
    file.try_exists()
        .context("checking for existance of the provided binary")
        .and_then(|exists| exists.then_some(file).context("checking if file exists"))
        .with_context(|| format!("checking if file exists: [{}]", file.display()))
}

impl Wrapped7Zip {
    pub fn new(path: &Path) -> Result<Self> {
        check_exists(path)
            .context("checking if binary exists")
            .map(Arc::from)
            .map(Self)
            .with_context(|| format!("instantiating wrapper at [{}]", path.display()))
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
        self.output()
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
        let mut command = Command::new(self.0.as_ref());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
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
}

fn run_watcher(error_callback: Sender<anyhow::Error>, mut child: Child) {
    loop {
        let status = match child.try_wait() {
            Ok(status_code) => match status_code {
                Some(status_code) => match status_code.success() {
                    true => Some(Ok(())),
                    false => match child.stderr.take() {
                        Some(mut stderr) => Vec::new().pipe(|mut stderr_output| {
                            stderr
                                .read_to_end(&mut stderr_output)
                                .context("could not read stderr")
                                .map(|_| String::from_utf8_lossy(&stderr_output).to_string())
                                .pipe(|res| match res {
                                    Ok(error) => Err(anyhow!("source: {error}")),
                                    Err(message) => Err(message),
                                })
                        }),
                        None => Err(anyhow!("process exited without stderr")),
                    }
                    .pipe(Some),
                },
                None => None,
            },
            Err(e) => Some(Err(anyhow::Error::from(e).context("checking for status of process"))),
        };
        match status {
            Some(result) => match result {
                Ok(_) => break,
                Err(error) => {
                    error_callback.send(error).ok();
                    break;
                }
            },
            None => continue,
        }
    }
}
fn spawn_watcher(error_callback: Sender<anyhow::Error>, child: Child) {
    tokio::task::spawn_blocking(move || {
        run_watcher(error_callback, child);
    });
}

impl Wrapped7Zip {
    pub fn find_bin() -> Result<Self> {
        ["7z", "7z.exe"]
            .into_iter()
            .find_map(|bin| which::which(bin).ok())
            .context("no 7z binary")
            .and_then(|path| Self::new(&path))
    }
}

thread_local! {
    pub static WRAPPED_7ZIP: Arc<Wrapped7Zip> = Arc::new(Wrapped7Zip::find_bin().expect("no 7z found, fix your dependencies"));
}

pub struct ArchiveFileHandle {
    error_callback: Arc<Mutex<Receiver<anyhow::Error>>>,
    reader: BufReader<ChildStdout>,
    finished: bool,
}

impl Read for ArchiveFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        let n = self
            .reader
            .read(buf)
            .tap_err(|error| tracing::warn!(?error, "reading from 7zip stopped"))?;

        if n == 0 {
            // EOF reached. Check for errors
            self.finished = true;
            if let Ok(error) = self.error_callback.lock().try_recv() {
                return Err(std::io::Error::other(error));
            }
        }

        Ok(n)
    }
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
    // pub fn get_files(&self, files: &[&Path]) -> Result<Vec<(ListOutputEntry, tempfile::TempPath)>> {
    //     self.list_files().and_then(|listing| {
    //         listing
    //             .into_iter()
    //             .map(|file| (file.path.as_path(), file))
    //             .collect::<BTreeMap<_, _>>()
    //             .pipe(|lookup| {
    //                 files.iter().map(|file| {
    //                     lookup
    //                         .remove(file)
    //                         .with_context(|| format!("file [{file}] not in archive"))
    //                 })
    //             })
    //             .collect::<Result<Vec<_>>>()
    //             .context("not all files found")
    //     }).and_then(|files| {

    //         })
    // }
    pub fn get_file(&self, file: &Path) -> Result<(ListOutputEntry, ArchiveFileHandle)> {
        self.list_files()
            .and_then(|files| {
                files
                    .iter()
                    .find(
                        |ListOutputEntry {
                             modified: _,
                             original_path: _,
                             created: _,
                             size: _,
                             path,
                         }| { path.as_path().eq(file) },
                    )
                    .cloned()
                    .with_context(|| format!("file not found in {:#?}", files.into_iter().map(|file| file.path).collect::<Vec<_>>()))
            })
            .and_then(|file| {
                self.binary
                    .command(|c| {
                        c
                            // extract
                            .arg("x")
                            .arg(&self.archive)
                            .arg(&file.original_path)
                            // write data to stdout
                            .arg("-so")
                    })
                    .spawn()
                    .context("spawning extract command")
                    .and_then(|mut child| {
                        child
                            .stdout
                            .take()
                            .context("no stdout")
                            .map(|stdout| (stdout, child))
                    })
                    .map(|(stdout, child)| {
                        let (tx, rx) = mpsc::channel();
                        spawn_watcher(tx, child);
                        ArchiveFileHandle {
                            error_callback: rx.pipe(Mutex::new).pipe(Arc::new),
                            reader: stdout.pipe(BufReader::new),
                            finished: false,
                        }
                    })
                    .with_context(|| format!("when initializing read from archive file [{}]", file.path.display()))
                    .map(|handle| (file, handle))
            })
    }
}

#[cfg(test)]
mod tests;
