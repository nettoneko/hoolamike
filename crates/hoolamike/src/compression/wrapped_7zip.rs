use ::wrapped_7zip::Wrapped7Zip;
pub use ::wrapped_7zip::{ArchiveFileHandle, ArchiveHandle};

thread_local! {
    pub static WRAPPED_7ZIP: Arc<Wrapped7Zip> = Arc::new(Wrapped7Zip::find_bin(*crate::consts::TEMP_FILE_DIR).expect("no 7z found, fix your dependencies"));
}

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
