// use {
//     super::{ProcessArchive, *},
//     itertools::Itertools,
//     std::{convert::identity, fs::File, path::PathBuf},
// };

// pub type ZipFile<'a> = ::zip::read::ZipFile<'a>;
// pub type ZipArchive = ::zip::read::ZipArchive<File>;

// impl ProcessArchive for ZipArchive {
//     fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
//         (0..self.len())
//             .map(|idx| {
//                 self.by_index(idx)
//                     .with_context(|| format!("reading file idx [{idx}]"))
//                     .map(|file| {
//                         file.is_file()
//                             .then_some(())
//                             .and_then(|_| file.enclosed_name())
//                     })
//             })
//             .filter_map_ok(identity)
//             .collect::<Result<_>>()
//             .context("reading archive contents")
//     }

//     fn get_handle<'this>(&'this mut self, path: &Path) -> Result<super::ArchiveFileHandle> {
//         self.index_for_path(path)
//             .with_context(|| format!("no entry for [{}]", path.display()))
//             .and_then(|index| self.by_index(index).context("index read"))
//             .map(super::ArchiveFileHandle::Zip)
//     }
// }

// impl super::ProcessArchiveFile for ZipFile<'_> {}
