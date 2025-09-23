pub mod lib;

use {
    clap::{
        Parser,
        Subcommand,
    },
};

#[derive(Display)]
#[allow(dead_code)]
#[strum(serialize_all = "snake_case")]
enum Api {
    Traits,
    Skills,
    ItemStats,
    Specializations,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    verb: Verb,
}

#[derive(Parser, Debug)]
struct Download {
    /// Provide a GW2 API endpoint, e.g. traits, skills, itemstats, specializations.
    kind: String,
    #[arg(
            long,
            short,
            require_equals = false,
            value_name = "limit",
            num_args = 0..=1,
            default_value_t = 100,
            default_missing_value = "100",
    )]
    /// Simultaneous download limit
    limit: usize,
}

#[derive(Subcommand, Debug)]
enum Verb {
    #[command(arg_required_else_help = true)]
    Download(Download),
    MapRatio,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();

    match args.verb {
        Verb::Download (download) => download_kind_json(download.limit, download.kind).await?,
        Verb::MapRatio => Map::do_map_ratio().await?,
    }

    Ok(())
}
