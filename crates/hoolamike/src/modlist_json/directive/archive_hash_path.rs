use {
    super::MaybeWindowsPath,
    nonempty::NonEmpty,
    serde::{ser::Error as _, Deserialize, Serialize},
    std::iter::{empty, once},
    tap::prelude::*,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchiveHashPath {
    pub source_hash: String,
    pub path: Vec<MaybeWindowsPath>,
}

impl Serialize for ArchiveHashPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.pipe(|Self { source_hash: root_hash, path }| {
            empty()
                .chain(once(root_hash.clone().pipe(Ok)))
                .chain(
                    path.iter()
                        .map(|p| serde_json::to_string(p).map_err(S::Error::custom)),
                )
                .collect::<Result<Vec<_>, _>>()
                .and_then(|output| output.serialize(serializer))
        })
    }
}

impl<'de> Deserialize<'de> for ArchiveHashPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        NonEmpty::<String>::deserialize(deserializer).map(|NonEmpty { head, tail }| ArchiveHashPath {
            source_hash: head,
            path: tail.into_iter().map(MaybeWindowsPath).collect(),
        })
    }
}
