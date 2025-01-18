use {
    crate::{
        compression::{bethesda_archive::BethesdaArchive, ProcessArchive},
        config_file::{ExtrasConfig, HoolamikeConfig, InstallationConfig},
        modlist_json::GameName,
        progress_bars_v2::IndicatifWrapIoExt,
        utils::{MaybeWindowsPath, PathReadWrite},
    },
    anyhow::{bail, Context, Result},
    manifest_file::{
        asset::{Asset, FullLocation, LocationIndex, MaybeFullLocation, NewAsset},
        kind_guard::WithKindGuard,
        location::Location,
        variable::{LocalAppDataVariable, PersonalFolderVariable, RegistryVariable, StringVariable, Variable},
    },
    normalize_path::NormalizePath,
    num::ToPrimitive,
    rayon::iter::{IntoParallelIterator, ParallelIterator},
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        collections::BTreeMap,
        io::{BufReader, Read},
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tracing::{debug, info, info_span, instrument},
};

pub mod manifest_file;
pub mod templating {
    /// returns (left, variable_name, right)
    pub fn find_template_marker(input: &str) -> Option<(&str, &str, &str)> {
        input.split_once('%').and_then(|(left, right)| {
            right
                .split_once('%')
                .map(|(variable_name, right)| (left, variable_name, right))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    path_to_ttw_mpi_file: PathBuf,
    variables: BTreeMap<String, String>,
}

#[derive(clap::Args)]
pub struct CliConfig {}

const MANIFEST_PATH: &str = "_package/index.json";

type LocationsLookup = BTreeMap<LocationIndex, Location>;

struct ResolverContext {
    locations: LocationsLookup,
    installation: InstallationConfig,
    variables: VariablesContext,
}

struct VariablesContext {
    variables: BTreeMap<String, Variable>,
    ttw_config_variables: BTreeMap<String, String>,
    hoolamike_installation_config: HoolamikeConfig,
}

impl VariablesContext {
    #[instrument(skip(self))]
    fn resolve_variable(&self, maybe_with_variable: &str) -> Result<Cow<str>> {
        match self::templating::find_template_marker(maybe_with_variable) {
            Some((left, variable_name, right)) => info_span!("variable_found", %variable_name).in_scope(|| match variable_name {
                "DESTINATION" => self
                    .hoolamike_installation_config
                    .installation
                    .installation_path
                    .display()
                    .to_string()
                    .pipe(Cow::<str>::Owned)
                    .tap(|value| info!(%variable_name, %value, "⭐⭐⭐ MAGICALLY ⭐⭐⭐ filling the variable using hoolamike derived context"))
                    .pipe(Ok),
                "FO3ROOT" => self
                    .hoolamike_installation_config
                    .games
                    .get(&GameName::new("Fallout3".to_string()))
                    .context("'Fallout3' is not found in hoolamike defined games")
                    .map(|p| p.root_directory.display().to_string().pipe(Cow::Owned))
                    .tap_ok(|value| info!(%variable_name, %value, "⭐⭐⭐ MAGICALLY ⭐⭐⭐ filling the variable using hoolamike derived context")),

                "FNVROOT" => self
                    .hoolamike_installation_config
                    .games
                    .get(&GameName::new("FalloutNewVegas".to_string()))
                    .context("'FalloutNewVegas' is not found in hoolamike defined games")
                    .map(|p| p.root_directory.display().to_string().pipe(Cow::Owned))
                    .tap_ok(|value| info!(%variable_name, %value, "⭐⭐⭐ MAGICALLY ⭐⭐⭐ filling the variable using hoolamike derived context")),

                variable_name => match self.variables.get(variable_name) {
                    Some(variable) => Err(())
                        .or_else(|_| {
                            self.ttw_config_variables
                                .get(variable_name)
                                .map(|v| v.as_str().pipe(Cow::Borrowed))
                                .with_context(|| format!("no variable defined in hoolamike config: '{variable_name}'"))
                        })
                        .or_else(|reason| {
                            variable
                                .value()
                                .filter(|v| {
                                    !v.is_empty().tap(|is_empty| {
                                        if *is_empty {
                                            tracing::warn!("variable [{variable_name}] is empty which means it should be filled by the user");
                                        }
                                    })
                                })
                                .map(Cow::Borrowed)
                                .context("variable not found in installer variable definition section")
                                .with_context(|| format!("HINT: you can override the variables in hoolamike config\n({reason:?})"))
                        }),
                    None => Err(anyhow::anyhow!("ttw installer does not define this variable: '{variable_name}'")),
                }
                .map(|variable| format!("{left}{variable}{right}"))
                .map(Cow::Owned)
                .inspect(|updated_value| tracing::info!(%updated_value, "updated templated value"))
                .and_then(|updated| self.resolve_variable(&updated)),
            }),
            None => Ok(Cow::Owned(
                maybe_with_variable
                    .to_string()
                    .tap(|value| tracing::debug!(%value, "value does not contain variables")),
            )),
        }
    }
}

impl MaybeFullLocation {
    fn lookup_from_both_source_and_target(self, source: &FullLocation) -> FullLocation {
        match self.path {
            Some(path) => FullLocation { location: self.location, path },
            None => FullLocation {
                location: self.location,
                path: source.path.clone(),
            },
        }
    }
}

impl FullLocation {
    #[instrument(level = "DEBUG", skip(from_reader, locations))]
    fn insert_into(self, locations: &LocationsLookup, from_reader: &mut impl Read) -> Result<()> {
        locations
            .get(&self.location)
            .with_context(|| format!("no location for {self:#?}"))
            .inspect(|location| tracing::debug!("{location:#?}"))
            .and_then(|location| match location {
                Location::Folder(folder) => folder
                    .inner
                    .value
                    .clone()
                    .pipe(MaybeWindowsPath)
                    .pipe(MaybeWindowsPath::into_path)
                    .pipe(|folder| folder.join(self.path.0.into_path()).normalize())
                    .open_file_write()
                    .and_then(|(target_path, mut target_file)| {
                        std::io::copy(from_reader, &mut target_file)
                            .with_context(|| format!("copying into [{target_path:#?}]"))
                            .map(|wrote| tracing::info!(?target_path, "wrote [{wrote}bytes]"))
                    }),
                Location::ReadArchive(read_archive) => anyhow::bail!("cannot insert into Location::ReadArchive({read_archive:#?})"),
                Location::WriteArchive(write_archive) => anyhow::bail!("Location::WriteArchive({write_archive:#?})"),
            })
    }
    fn into_reader(self, locations: &LocationsLookup) -> Result<Box<dyn Read>> {
        locations
            .get(&self.location)
            .with_context(|| format!("no location for {self:#?}"))
            .inspect(|location| tracing::debug!("{location:#?}"))
            .and_then(|location| match location {
                Location::Folder(folder) => folder
                    .inner
                    .value
                    .clone()
                    .pipe(MaybeWindowsPath)
                    .pipe(MaybeWindowsPath::into_path)
                    .pipe(|path| path.join(self.path.0.into_path()).normalize())
                    .pipe(|source| {
                        source
                            .open_file_read()
                            .map(|(_, file)| Box::new(file) as Box<dyn Read>)
                    }),
                Location::ReadArchive(read_archive) => anyhow::bail!("Location::ReadArchive({read_archive:#?})"),
                Location::WriteArchive(write_archive) => anyhow::bail!("Location::WriteArchive({write_archive:#?})"),
            })
    }
}

#[instrument(skip_all)]
pub fn install(cli_config: CliConfig, hoolamike_config: HoolamikeConfig) -> Result<()> {
    let ExtensionConfig {
        path_to_ttw_mpi_file,
        variables: ttw_config_variables,
    } = hoolamike_config
        .extras
        .tale_of_two_wastelands
        .as_ref()
        .context("no tale of two wastelands configured in hoolamike.yaml")?;

    let manifest_file::Manifest {
        package,
        variables,
        locations,
        tags,
        checks,
        file_attrs,
        post_commands,
        assets,
    } = crate::compression::bethesda_archive::BethesdaArchive::open(path_to_ttw_mpi_file)
        .and_then(|mut archive| {
            archive
                .get_handle(Path::new(MANIFEST_PATH))
                .context("extracting the manifest out of MPI file")
        })
        .map(BufReader::new)
        .and_then(|reader| {
            String::new()
                .pipe(|mut out| {
                    info_span!("extracing_manifest")
                        .wrap_read(0, reader)
                        .read_to_string(&mut out)
                        .map(|_| out)
                        .context("extracting")
                })
                .and_then(|manifest| serde_json::from_str::<manifest_file::Manifest>(&manifest).context("parsing"))
                .context("parsing extracted manifest file")
        })
        .with_context(|| format!("extracting manifest out of [{path_to_ttw_mpi_file:?}]"))?;

    info!(package=%serde_json::to_string_pretty(&package).unwrap_or_else(|e| format!("[{e:#?}]")), "got manifest file");

    let _span = info_span!(
        "installing_ttw",
        version=%package.version,
        title=%package.title,
    )
    .entered();
    let variables = variables
        .release()
        .into_iter()
        .map(|variable| (variable.name().to_string(), variable))
        .collect::<BTreeMap<_, _>>();

    let variables_context = VariablesContext {
        variables,
        ttw_config_variables: ttw_config_variables.clone(),
        hoolamike_installation_config: hoolamike_config.clone(),
    };
    let locations = locations
        .release()
        .into_iter()
        .enumerate()
        .map(|(idx, mut location)| {
            idx.to_u8()
                .context("too many assets")
                .map(LocationIndex)
                .and_then(|idx| {
                    variables_context
                        .resolve_variable(location.value_mut())
                        .map(|resolved| (idx, location.tap_mut(|location| *location.value_mut() = resolved.to_string())))
                })
        })
        .collect::<Result<BTreeMap<LocationIndex, Location>>>()
        .context("collecting locations")?;

    assets
        .into_iter()
        .try_for_each(move |asset| {
            info_span!("handling_asset", kind=?manifest_file::asset::AssetRawKind::from(&asset)).in_scope(|| {
                match asset.clone() {
                    Asset::New(NewAsset { tags, status, source, target }) => {
                        let target = target.lookup_from_both_source_and_target(&source);
                        BethesdaArchive::open(&path_to_ttw_mpi_file)
                            .context("opening mpi file")
                            .and_then(|mut archive| {
                                archive
                                    .get_handle(&source.path.0.into_path())
                                    .and_then(|mut handle| target.insert_into(&locations, &mut handle))
                            })
                    }
                    Asset::Copy(copy_asset) => Err(anyhow::anyhow!("Asset::Copy({copy_asset:#?}) not implemented")),
                    Asset::Patch(patch_asset) => Err(anyhow::anyhow!("Asset::Patch({patch_asset:#?}) not implemented")),
                    Asset::XwmaFuz(xwma_fuz_asset) => Err(anyhow::anyhow!("Asset::XwmaFuz({xwma_fuz_asset:#?}) not implemented")),
                    Asset::OggEnc2(ogg_enc2_asset) => Err(anyhow::anyhow!("Asset::OggEnc2({ogg_enc2_asset:#?}) not implemented")),
                    Asset::AudioEnc(audio_enc_asset) => Err(anyhow::anyhow!("Asset::AudioEnc({audio_enc_asset:#?}) not implemented")),
                }
                .with_context(|| format!("handling [{asset:#?}]"))
            })
        })
        .context("executing asset operations")
}
