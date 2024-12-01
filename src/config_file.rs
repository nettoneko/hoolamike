use {
    crate::modlist_json::GameName,
    anyhow::{Context, Result},
    indexmap::IndexMap,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallationConfig {
    pub modlist_file: Option<PathBuf>,
}

pub type GamesConfig = IndexMap<GameName, GameConfig>;

fn default_games_config() -> GamesConfig {
    GamesConfig::new().tap_mut(|games| {
        games
            .insert(
                GameName::new("ExampleGame".into()),
                GameConfig {
                    root_directory: ["path", "to", "example", "game"]
                        .into_iter()
                        .fold(PathBuf::new(), |acc, next| acc.join(next)),
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
    pub fn find() -> Result<Self> {
        [format!("./{CONFIG_FILE_NAME}"), format!("~/.config/hoolamike/{CONFIG_FILE_NAME}")]
            .pipe(|config_paths| {
                config_paths
                    .clone()
                    .into_iter()
                    .map(PathBuf::from)
                    .find(|path| path.exists())
                    .with_context(|| format!("checking paths: {config_paths:?}"))
                    .context("no config file detected")
            })
            .tap_ok(|config| info!("found config at '{}'", config.display()))
            .and_then(|config| std::fs::read_to_string(config).context("reading file"))
            .and_then(|config| serde_yaml::from_str::<Self>(&config).context("parsing config file"))
            .with_context(|| format!("getting [{CONFIG_FILE_NAME}]"))
            .tap_ok(|config| {
                debug!("{config:?}");
            })
    }
}
