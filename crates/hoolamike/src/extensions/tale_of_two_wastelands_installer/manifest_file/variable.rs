use {
    super::kind_guard::WithKindGuard,
    serde::{Deserialize, Serialize},
    tap::prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct StringVariable {
    pub name: String,
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl Variable {
    pub fn name(&self) -> &str {
        match self {
            Variable::String(v) => v.inner.name.as_str(),
            Variable::PersonalFolder(v) => v.inner.name.as_str(),
            Variable::LocalAppData(v) => v.inner.name.as_str(),
            Variable::Registry(v) => v.inner.name.as_str(),
        }
    }

    pub fn value(&self) -> Option<&str> {
        match self {
            Variable::String(v) => v.inner.value.as_deref(),
            Variable::PersonalFolder(_) => None,
            Variable::LocalAppData(_) => None,
            Variable::Registry(v) => v.inner.value.as_str().pipe(Some),
        }
    }
}
