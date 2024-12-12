use {
    crate::modlist_json::GameName,
    anyhow::{Context, Result},
    indexmap::IndexMap,
    serde::{Deserialize, Serialize},
    std::{
        collections::HashSet,
        iter::{empty, once},
        path::{Path, PathBuf},
    },
    tap::prelude::*,
    tracing::{debug, info},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NexusConfig {
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct DownloadersConfig {
    #[derivative(Default(value = "std::env::current_dir().unwrap().join(\"downloads\")"))]
    pub downloads_directory: PathBuf,
    pub nexus: NexusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
pub struct GameConfig {
    pub root_directory: PathBuf,
}

fn join_default_path(segments: impl IntoIterator<Item = &'static str>) -> PathBuf {
    empty()
        .chain(once("FIXME"))
        .chain(segments)
        .fold(PathBuf::new(), |acc, next| acc.join(next))
}

#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct InstallationConfig {
    #[derivative(Default(value = "join_default_path([\"path\",\"to\",\"file.wabbajack\" ])"))]
    pub wabbajack_file_path: PathBuf,
    #[derivative(Default(value = "std::env::current_dir().unwrap()"))]
    pub installation_path: PathBuf,
    pub whitelist_failed_directives: HashSet<String>,
}

pub type GamesConfig = IndexMap<GameName, GameConfig>;

fn default_games_config() -> GamesConfig {
    GamesConfig::new().tap_mut(|games| {
        games
            .insert(
                GameName::new("ExampleGame".into()),
                GameConfig {
                    root_directory: join_default_path(["path", "to", "example", "game"]),
                },
            )
            .pipe(|_| ())
    })
}
#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct HoolamikeConfig {
    pub downloaders: DownloadersConfig,
    pub installation: InstallationConfig,
    #[derivative(Default(value = "default_games_config()"))]
    pub games: GamesConfig,
}

pub static CONFIG_FILE_NAME: &str = "hoolamike.yaml";
impl HoolamikeConfig {
    pub fn write(&self) -> Result<String> {
        Self::default()
            .pipe_ref(serde_yaml::to_string)
            .context("serialization failed")
            .map(|config| format!("\n# default {CONFIG_FILE_NAME} file\n# edit it according to your needs:\n{config}"))
    }
    pub fn find(path: &Path) -> Result<(PathBuf, Self)> {
        path.exists()
            .then(|| path.to_owned())
            .with_context(|| format!("config path [{}] does not exist", path.display()))
            .tap_ok(|config| info!("found config at '{}'", config.display()))
            .and_then(|config_path| {
                std::fs::read_to_string(&config_path)
                    .context("reading file")
                    .and_then(|config| serde_yaml::from_str::<Self>(&config).context("parsing config file"))
                    .map(|config| (config_path, config))
            })
            .with_context(|| format!("getting [{CONFIG_FILE_NAME}]"))
            .tap_ok(|config| {
                debug!("{config:?}");
            })
    }
}
