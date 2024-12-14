pub use ::wrapped_7zip::{ArchiveFileHandle, ArchiveHandle, WRAPPED_7ZIP};

use super::*;
impl ProcessArchive for ::wrapped_7zip::ArchiveHandle {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        self.list_files()
            .map(|files| files.into_iter().map(|entry| entry.path).collect())
    }

    fn get_handle(&mut self, path: &Path) -> Result<super::ArchiveFileHandle> {
        self.get_file(path)
            .map(super::ArchiveFileHandle::Wrapped7Zip)
    }
}
