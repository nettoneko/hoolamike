use {super::*, serde::Serializer};

// Implement custom `Deserialize` for `Element` by first reading into `ElementRaw`.
impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D>(deserializer: D) -> Result<Asset, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // First, deserialize JSON -> ElementRaw
        let raw = AssetRaw::deserialize(deserializer)?;

        // Then validate or transform ElementRaw -> Element
        Asset::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Asset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert "valid" Asset into the raw struct we know how to serialize

        self.clone().conv::<AssetRaw>().serialize(serializer)
    }
}
