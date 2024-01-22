use clap::Parser;
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::sz::SZBuilder;
use std::process::ExitCode;
use std::{error::Error, path::PathBuf};

const VERSION: &'static str = concat! {
    env! {"CARGO_PKG_VERSION"},
    "-",
    env! {"VERGEN_GIT_DESCRIBE"}
};

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
#[command(version = VERSION)]
struct Cli {
    hgrid_path: PathBuf,
    output_filepath: PathBuf,
    #[clap(long)]
    slevels: usize,
    #[clap(long, value_delimiter = ' ', num_args = 1..)]
    zlevels: Option<Vec<f64>>,
    #[clap(long, default_value = "1.")]
    theta_f: Option<f64>,
    #[clap(long, default_value = "0.001")]
    theta_b: Option<f64>,
    #[clap(
        long,
        help = "Critical layer depth.",
        alias = "hc",
        default_value = "30."
    )]
    critical_depth: Option<f64>,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    let hgrid = Hgrid::try_from(&cli.hgrid_path)?;
    let mut builder = SZBuilder::default();
    builder.hgrid(&hgrid);
    builder.slevels(&cli.slevels);
    builder.theta_f(cli.theta_f.as_ref().unwrap());
    builder.theta_b(cli.theta_b.as_ref().unwrap());
    builder.critical_depth(cli.critical_depth.as_ref().unwrap());
    if cli.zlevels.is_some() {
        builder.zlevels(cli.zlevels.as_ref().unwrap());
    }
    let sz = builder.build()?;
    sz.write_to_file(&cli.output_filepath)?;
    Ok(())
}

fn main() -> ExitCode {
    match entrypoint() {
        Err(e) => {
            eprintln!("Error: {:?}: {}", e, e);
            return ExitCode::FAILURE;
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}
