use super::*;
impl ProcessArchive for ::wrapped_7zip::ArchiveHandle {
    fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
        self.list_files()
            .map(|files| files.into_iter().map(|entry| entry.name).collect())
    }

    fn get_handle(&mut self, path: &Path) -> Result<self::ArchiveFileHandle<'_>> {
        self.get_file(path).map(ArchiveFileHandle::Wrapped7Zip)
    }
}
