use clap::{Args, Parser, Subcommand, ValueEnum};
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::transforms::quadratic::QuadraticTransformOpts;
use schismrs_vgrid::transforms::s::STransformOpts;
use schismrs_vgrid::transforms::StretchingFunction;
use schismrs_vgrid::vqs::{VQSAutoBuilder, VQSBuilder, VQSKMeansBuilder};
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
    #[clap(short, long)]
    output_filepath: Option<PathBuf>,
    #[clap(short, long)]
    transform: StretchingFunctionKind,
    #[clap(
        short,
        long,
        default_value = "0.",
        help = "|a_vqs0|<=1. -- -1 skew towards bottom, 1. skew towards surface"
    )]
    a_vqs0: Option<f64>,
    #[clap(short, long, default_value = "0.", help = "defined as positive down")]
    etal: Option<f64>,
    #[clap(short, long, default_value = "0.3")]
    skew_decay_rate: Option<f64>,
    #[clap(
        long,
        help = "Range is (0., 20.]. Values closer to 0. make the transformation \
                more similar to traditional sigma. Larger values will increase \
                resolution at the top and bottom."
    )]
    theta_f: Option<f64>,
    #[clap(
        long,
        help = "Range is [0., 1.]. For values closer to 0. the surface is \
                resolved. For values closer to 1., but the surface and bottom \
                are resolved."
    )]
    theta_b: Option<f64>,
    #[clap(long)]
    dz_bottom_min: f64,
    #[clap(long, action)]
    show_zmas_plot: bool,
    #[clap(long)]
    save_zmas_plot: Option<PathBuf>,
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
    Kmeans(KmeansCliOpts),
    Hsm(HsmCliOpts),
    Auto(AutoCliOpts),
}

#[derive(Args, Debug)]
struct KmeansCliOpts {
    #[clap(short, long)]
    clusters: usize,
    #[clap(short, long, default_value = "2")]
    shallow_levels: Option<usize>,
    #[clap(long)]
    max_levels: usize,
}

#[derive(Args, Debug)]
struct HsmCliOpts {
    #[clap(short, long, value_delimiter = ' ', num_args = 1..)]
    depths: Vec<f64>,
    #[clap(short, long, value_delimiter = ' ', num_args = 1..)]
    nlevels: Vec<usize>,
}

#[derive(Args, Debug)]
struct AutoCliOpts {
    #[clap(long)]
    ngrids: usize,
    #[clap(
        long,
        default_value = "1.",
        help = "This is the first depth below etal. This input is positive down."
    )]
    initial_depth: Option<f64>,
    #[clap(long, default_value = "2")]
    shallow_levels: Option<usize>,
    #[clap(long)]
    max_levels: usize,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    let hgrid = Hgrid::try_from(&cli.hgrid_path)?;
    let transform = match cli.transform {
        StretchingFunctionKind::Quadratic => {
            let quadratic_opts = QuadraticTransformOpts {
                a_vqs0: cli.a_vqs0.as_ref().unwrap(),
                etal: cli.etal.as_ref().unwrap(),
                skew_decay_rate: cli.skew_decay_rate.as_ref().unwrap(),
            };
            StretchingFunction::Quadratic(quadratic_opts)
        }
        StretchingFunctionKind::S => {
            let s_opts = STransformOpts {
                a_vqs0: cli.a_vqs0.as_ref().unwrap(),
                etal: cli.etal.as_ref().unwrap(),
                theta_b: cli.theta_b.as_ref().unwrap(),
                theta_f: cli.theta_f.as_ref().unwrap(),
            };
            StretchingFunction::S(s_opts)
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
        Modes::Kmeans(opts) => {
            let mut builder = VQSKMeansBuilder::default();
            builder.hgrid(&hgrid);
            builder.stretching(&transform);
            builder.nclusters(&opts.clusters);
            builder.dz_bottom_min(&cli.dz_bottom_min);
            builder.etal(cli.etal.as_ref().unwrap());
            if let Some(shallow_levels) = &opts.shallow_levels {
                builder.shallow_levels(shallow_levels);
            }
            builder.max_levels(&opts.max_levels);
            builder.build()?
        }
        Modes::Auto(opts) => {
            let mut builder = VQSAutoBuilder::default();
            builder.hgrid(&hgrid);
            builder.stretching(&transform);
            builder.ngrids(&opts.ngrids);
            builder.dz_bottom_min(&cli.dz_bottom_min);
            builder.initial_depth(&opts.initial_depth.as_ref().unwrap());
            builder.shallow_levels(&opts.shallow_levels.as_ref().unwrap());
            builder.max_levels(&opts.max_levels);
            builder.build()?
        }
    };
    if cli.output_filepath.is_some() {
        vqs.write_to_file(&cli.output_filepath.as_ref().unwrap())?;
    };

    if cli.show_zmas_plot || cli.save_zmas_plot.is_some() {
        let zmas_plot = vqs.make_z_mas_plot()?;
        if cli.show_zmas_plot {
            zmas_plot.show();
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let exit_code = match entrypoint() {
        Err(e) => {
            eprintln!("Error: {}", e);
            return ExitCode::FAILURE;
        }
        Ok(_) => ExitCode::SUCCESS,
    };
    exit_code
}
