// use {
//     super::{ProcessArchive, *},
//     itertools::Itertools,
//     std::{borrow::{Borrow, BorrowMut}, convert::identity, path::PathBuf},
// };

// pub type SevenZipFile<'a> = ::sevenz_rust::SevenZReader<File>;
// pub type SevenZipArchive = ::sevenz_rust::SevenZReader<File>;

// impl ProcessArchive for SevenZipArchive {
//     fn list_paths(&mut self) -> Result<Vec<PathBuf>> {
//         (0..self.borrow_mut())
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

//     fn get_handle<'this>(&'this mut self, path: &Path) -> Result<super::ArchiveFileHandle<'this>> {
//         self.index_for_path(path)
//             .with_context(|| format!("no entry for [{}]", path.display()))
//             .and_then(|index| self.by_index(index).context("index read"))
//             .map(super::ArchiveFileHandle::SevenZip)
//     }
// }

// impl super::ProcessArchiveFile for SevenZipFile<'_> {}
