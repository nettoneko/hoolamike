use {
    anyhow::{anyhow, Context, Result},
    std::{
        iter::once,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        sync::Arc,
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

// async fn check_exists_async(file: &Path) -> Result<&Path> {
//     try_exists(file)
//         .await
//         .context("checking file existance")
//         .and_then(|exists| exists.then_some(file).context("file does not exists"))
//         .with_context(|| format!("checking if file [{}] exists", file.display()))
// }

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

    fn read_stdout_success(mut self) -> Result<String> {
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
    pub fn query_file_info(&self, path: &Path) -> Result<String> {
        check_exists(path)
            .map(|path| {
                self.command(|c| {
                    c
                        // actual command
                        .arg("l")
                        .arg(path)
                })
            })
            .and_then(|command| command.read_stdout_success())
            .with_context(|| format!("statting file [{}]", path.display()))
    }
    pub fn open(&self, file: &Path) -> Result<ArchiveHandle> {
        check_exists(file)
            .context("checking if opened file exists")
            .and_then(|file| todo!())
    }
}

#[cfg(test)]
mod tests;
