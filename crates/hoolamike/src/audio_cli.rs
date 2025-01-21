#[derive(clap::Args)]
pub struct AudioCliCommand {
    #[command(subcommand)]
    pub command: hoola_audio::Commands,
}
