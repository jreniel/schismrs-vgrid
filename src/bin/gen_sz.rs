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
    #[clap(long)]
    theta_f: Option<f64>,
    #[clap(long)]
    theta_b: Option<f64>,
    #[clap(long, help = "Critical layer depth", alias = "hc")]
    critical_depth: Option<f64>,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    let hgrid = Hgrid::try_from(&cli.hgrid_path)?;
    let mut builder = SZBuilder::default();
    builder.hgrid(&hgrid);
    builder.slevels(&cli.slevels);
    if cli.zlevels.is_some() {
        builder.zlevels(cli.zlevels.as_ref().unwrap());
    }
    if cli.theta_f.is_some() {
        builder.theta_f(cli.theta_f.as_ref().unwrap());
    }
    if cli.theta_b.is_some() {
        builder.theta_b(cli.theta_b.as_ref().unwrap());
    }
    if cli.critical_depth.is_some() {
        builder.critical_depth(cli.critical_depth.as_ref().unwrap());
    }
    let sz = builder.build()?;
    sz.write_to_file(&cli.output_filepath)?;
    Ok(())
}

fn main() -> ExitCode {
    let exit_code = match entrypoint() {
        Err(e) => {
            eprintln!("Error: {:?}: {}", e, e);
            return ExitCode::FAILURE;
        }
        Ok(_) => ExitCode::SUCCESS,
    };
    exit_code
}
