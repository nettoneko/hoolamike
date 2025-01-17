use {
    anyhow::{Context, Result},
    serde::{Deserialize, Serialize},
    tap::prelude::*,
};

#[derive(Debug, serde_repr::Serialize_repr, serde_repr::Deserialize_repr, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
enum AssetRawKind {
    Copy = 0,
    New = 1,
    Patch = 2,
    XwmaFuz = 3,
    OggEnc2 = 4,
    AudioEnc = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Params(String);

impl Params {
    pub const fn empty() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Status(u8);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tags(u16);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocationIndex(u8);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, Default)]
struct FileName(String);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
enum AssetRaw {
    A(Tags, AssetRawKind, Params, Status, LocationIndex, LocationIndex, FileName),
    B(Tags, AssetRawKind, Params, Status, LocationIndex, LocationIndex, FileName, FileName),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct FullLocation {
    pub location: LocationIndex,
    pub path: FileName,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CopyAsset {
    pub tags: Tags,
    pub status: Status,
    pub source: FullLocation,
    pub target: FullLocation,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct NewAsset {
    tags: u16,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PatchAsset {
    tags: u16,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct XwmaFuzAsset {
    tags: u16,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct OggEnc2Asset {
    tags: u16,
}
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct AudioEncAsset {
    tags: u16,
}

#[derive(derive_more::From, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Asset {
    Copy(CopyAsset),
    New(NewAsset),
    Patch(PatchAsset),
    XwmaFuz(XwmaFuzAsset),
    OggEnc2(OggEnc2Asset),
    AudioEnc(AudioEncAsset),
}

impl From<&Asset> for AssetRawKind {
    fn from(value: &Asset) -> Self {
        match value {
            Asset::Copy(_) => Self::Copy,
            Asset::New(_) => Self::New,
            Asset::Patch(_) => Self::Patch,
            Asset::XwmaFuz(_) => Self::XwmaFuz,
            Asset::OggEnc2(_) => Self::OggEnc2,
            Asset::AudioEnc(_) => Self::AudioEnc,
        }
    }
}

impl Into<AssetRaw> for Asset {
    fn into(self) -> AssetRaw {
        let kind = AssetRawKind::from(&self);
        match self {
            Asset::Copy(CopyAsset { tags, status, source, target }) => {
                AssetRaw::B(tags, kind, Params::empty(), status, source.location, target.location, source.path, target.path)
            }
            Asset::New(new_asset) => todo!(),
            Asset::Patch(patch_asset) => todo!(),
            Asset::XwmaFuz(xwma_fuz_asset) => todo!(),
            Asset::OggEnc2(ogg_enc2_asset) => todo!(),
            Asset::AudioEnc(audio_enc_asset) => todo!(),
        }
    }
}

impl TryFrom<AssetRaw> for Asset {
    type Error = anyhow::Error;

    fn try_from(value: AssetRaw) -> Result<Self, Self::Error> {
        let tags;
        let operation;
        let parameters;
        let status;
        let location_location_index;
        let dest_location_location_index;
        let name;
        let dest_name;
        match value {
            AssetRaw::A(
                //
                f_flags,
                f_asset_raw_kind,
                f_params,
                f_status,
                f_location_index,
                f_location_index1,
                f_file_name,
            ) => {
                tags = f_flags;
                operation = f_asset_raw_kind;
                parameters = f_params;
                status = f_status;
                location_location_index = f_location_index;
                dest_location_location_index = f_location_index1;
                name = f_file_name;
                dest_name = None;
            }
            AssetRaw::B(
                //
                f_flags,
                f_asset_raw_kind,
                f_params,
                f_status,
                f_location_index,
                f_location_index1,
                f_file_name,
                f_dest_name,
            ) => {
                tags = f_flags;
                operation = f_asset_raw_kind;
                parameters = f_params;
                status = f_status;
                location_location_index = f_location_index;
                dest_location_location_index = f_location_index1;
                name = f_file_name;
                dest_name = Some(f_dest_name);
            }
        }

        match operation {
            AssetRawKind::Copy => CopyAsset {
                tags,
                status,
                source: FullLocation {
                    location: location_location_index,
                    path: name,
                },
                target: FullLocation {
                    location: dest_location_location_index,
                    path: dest_name.context("no dest name for copy")?,
                },
            }
            .pipe(Self::from),
            AssetRawKind::New => anyhow::bail!("AssetRawKind::New"),
            AssetRawKind::Patch => anyhow::bail!("AssetRawKind::Patch"),
            AssetRawKind::XwmaFuz => anyhow::bail!("AssetRawKind::XwmaFuz"),
            AssetRawKind::OggEnc2 => anyhow::bail!("AssetRawKind::OggEnc2"),
            AssetRawKind::AudioEnc => anyhow::bail!("AssetRawKind::AudioEnc"),
        }
        .pipe(anyhow::Ok)
    }
}

mod serde_compat;
