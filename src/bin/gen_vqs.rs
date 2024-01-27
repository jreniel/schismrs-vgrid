use clap::{Args, Parser, Subcommand, ValueEnum};
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::transforms::quadratic::QuadraticTransformOpts;
use schismrs_vgrid::transforms::s::STransformOpts;
use schismrs_vgrid::transforms::StretchingFunction;
use schismrs_vgrid::vqs::{VQSBuilder, VQSKMeansBuilder};
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
    #[clap(short, long)]
    transform: StretchingFunctionKind,
    #[clap(short, long)]
    a_vqs0: Option<f64>,
    #[clap(short, long)]
    etal: Option<f64>,
    #[clap(short, long)]
    theta_f: Option<f64>,
    #[clap(short, long)]
    theta_b: Option<f64>,
    #[clap(long)]
    dz_bottom_min: f64,
    #[clap(subcommand)]
    mode: Modes,
}

#[derive(ValueEnum, Clone, Debug)]
enum StretchingFunctionKind {
    Quadratic,
    S,
    // Shchepetkin2005,
    // Geyer,
    // Shchepetkin2010,
    // FixedZ
    // MultiMaster
}

#[derive(Subcommand, Debug)]
enum Modes {
    Auto(AutoCliOpts),
    Hsm(HsmCliOpts),
}

#[derive(Args, Debug)]
struct AutoCliOpts {
    #[clap(short, long)]
    clusters: usize,
    #[clap(short, long)]
    shallow_threshold: Option<f64>,
    #[clap(short, long)]
    shallow_levels: Option<usize>,
}

#[derive(Args, Debug)]
struct HsmCliOpts {
    #[clap(short, long, value_delimiter = ' ', num_args = 1..)]
    depths: Vec<f64>,
    #[clap(short, long, value_delimiter = ' ', num_args = 1..)]
    nlevels: Vec<usize>,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    let hgrid = Hgrid::try_from(&cli.hgrid_path)?;
    let transform = match cli.transform {
        StretchingFunctionKind::Quadratic => {
            let mut quadratic_opts = None;
            if cli.a_vqs0.is_some() || cli.etal.is_some() {
                quadratic_opts = Some(QuadraticTransformOpts {
                    a_vqs0: cli.a_vqs0.as_ref(),
                    etal: cli.etal.as_ref(),
                });
            }
            if quadratic_opts.is_some() {
                StretchingFunction::Quadratic(quadratic_opts)
            } else {
                StretchingFunction::Quadratic(None)
            }
        }
        StretchingFunctionKind::S => {
            let mut s_opts = None;
            if cli.a_vqs0.is_some() || cli.etal.is_some() {
                s_opts = Some(STransformOpts {
                    a_vqs0: cli.a_vqs0.as_ref(),
                    etal: cli.etal.as_ref(),
                    theta_b: cli.theta_b.as_ref(),
                    theta_f: cli.theta_f.as_ref(),
                });
            }
            if s_opts.is_some() {
                StretchingFunction::S(s_opts)
            } else {
                StretchingFunction::S(None)
            }
        }
    };
    let vqs = match &cli.mode {
        Modes::Hsm(opts) => VQSBuilder::default()
            .hgrid(&hgrid)
            .depths(&opts.depths)
            .nlevels(&opts.nlevels)
            .stretching(&transform)
            .dz_bottom_min(&cli.dz_bottom_min)
            .build()?,
        Modes::Auto(opts) => {
            let mut builder = VQSKMeansBuilder::default();
            builder.hgrid(&hgrid);
            builder.stretching(&transform);
            builder.nclusters(&opts.clusters);
            builder.dz_bottom_min(&cli.dz_bottom_min);
            if let Some(shallow_levels) = &opts.shallow_levels {
                builder.shallow_levels(shallow_levels);
            }
            if let Some(shallow_threshold) = &opts.shallow_threshold {
                builder.shallow_threshold(shallow_threshold);
            }
            builder.build()?
        }
    };
    vqs.write_to_file(&cli.output_filepath)?;
    // let mut html_out = PathBuf::new();
    // html_out.push("depth_distribution.html");
    // vqs.make_html_plot(&html_out, 5)?;
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
