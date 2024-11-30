#[macro_export]
macro_rules! serde_type_guard {
    ($name:ident, $identifier:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name;

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_str($identifier)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                let value: String = Deserialize::deserialize(deserializer)?;
                if value == $identifier {
                    Ok($name)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "Expected \"{}\", but found \"{}\"",
                        $identifier, value
                    )))
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $identifier)
            }
        }
    };
}
