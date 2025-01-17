use {
    super::kind_guard::WithKindGuard,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct StringVariable {
    pub name: String,
    pub value: Option<String>,
    pub exclude_delimiter: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct PersonalFolderVariable {
    pub name: String,
    pub exclude_delimiter: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct LocalAppDataVariable {
    pub name: String,
    pub exclude_delimiter: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct RegistryVariable {
    pub name: String,
    pub exclude_delimiter: bool,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Variable {
    String(WithKindGuard<0, StringVariable>),
    PersonalFolder(WithKindGuard<1, PersonalFolderVariable>),
    LocalAppData(WithKindGuard<2, LocalAppDataVariable>),
    Registry(WithKindGuard<4, RegistryVariable>),
}
