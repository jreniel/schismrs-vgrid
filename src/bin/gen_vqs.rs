use clap::{Parser, ValueEnum};
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::transforms::geyer::GeyerOpts;
use schismrs_vgrid::transforms::quadratic::QuadraticTransformOpts;
use schismrs_vgrid::transforms::s::STransformOpts;
use schismrs_vgrid::transforms::shchepetkin2005::Shchepetkin2005Opts;
use schismrs_vgrid::transforms::shchepetkin2010::Shchepetkin2010Opts;
use schismrs_vgrid::transforms::StretchingFunction;
use schismrs_vgrid::vqs::VQSBuilder;
use std::process::ExitCode;
use std::{error::Error, path::PathBuf};

#[derive(Parser, Debug)]
#[command(
    author,
    about = "Generate VQS (Variable Quadratic Sigma) vertical grid using the HSM method",
    long_about = None,
    version = env!("SCHISMRS_VGRID_VERSION")
)]
struct Cli {
    /// Path to the hgrid.gr3 file
    hgrid_path: PathBuf,

    /// Master grid depths (space-separated, e.g., "0.4 5 10 30")
    #[clap(short, long, value_delimiter = ' ', num_args = 1.., required = true)]
    depths: Vec<f64>,

    /// Number of levels at each master grid depth (space-separated, e.g., "2 10 15 30")
    #[clap(short, long, value_delimiter = ' ', num_args = 1.., required = true)]
    nlevels: Vec<usize>,

    /// Output file path (default: vgrid.in in current directory)
    #[clap(short, long)]
    output_filepath: Option<PathBuf>,

    /// Stretching function type
    #[clap(short, long, default_value = "s")]
    transform: StretchingFunctionKind,

    /// Stretching parameter: -1 skew towards bottom, 1 skew towards surface
    #[clap(short, long, default_value = "-1.0")]
    a_vqs0: Option<f64>,

    /// Reference elevation (positive down)
    #[clap(short, long, default_value = "0.")]
    etal: Option<f64>,

    /// Skew decay rate for quadratic transform
    #[clap(short, long, default_value = "0.03")]
    skew_decay_rate: Option<f64>,

    /// S-transform theta_f: surface/bottom focusing intensity (0.1-20)
    #[clap(long, default_value = "3.0")]
    theta_f: Option<f64>,

    /// S-transform theta_b: bottom layer focusing weight (0-1)
    #[clap(long, default_value = "0.5")]
    theta_b: Option<f64>,

    /// ROMS theta_s: surface stretching parameter (0-10)
    /// Used by Shchepetkin2005, Shchepetkin2010, and Geyer transforms
    #[clap(long, default_value = "5.0")]
    theta_s: Option<f64>,

    /// ROMS hc: critical depth in meters (>0)
    /// Used by Shchepetkin2005, Shchepetkin2010, and Geyer transforms
    #[clap(long, default_value = "5.0")]
    hc: Option<f64>,

    /// Minimum bottom layer thickness in meters
    #[clap(long, default_value = "0.3")]
    dz_bottom_min: Option<f64>,

    /// Show z_mas plot (requires plotly)
    #[clap(long, action)]
    show_zmas_plot: bool,

    /// Save z_mas plot to HTML file
    #[clap(long)]
    save_zmas_plot: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug, Default)]
enum StretchingFunctionKind {
    Quadratic,
    #[default]
    S,
    /// Shchepetkin (2005) UCLA-ROMS stretching
    Shchepetkin2005,
    /// Shchepetkin (2010) UCLA-ROMS double stretching
    Shchepetkin2010,
    /// R. Geyer stretching for high bottom boundary layer resolution
    Geyer,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();

    // Validate depths and nlevels have same length
    if cli.depths.len() != cli.nlevels.len() {
        return Err(format!(
            "depths ({}) and nlevels ({}) must have the same number of values",
            cli.depths.len(),
            cli.nlevels.len()
        )
        .into());
    }

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
        StretchingFunctionKind::Shchepetkin2005 => {
            let opts = Shchepetkin2005Opts::new(
                cli.etal.as_ref().unwrap(),
                cli.a_vqs0.as_ref().unwrap(),
                cli.theta_s.as_ref().unwrap(),
                cli.theta_b.as_ref().unwrap(),
                cli.hc.as_ref().unwrap(),
            );
            StretchingFunction::Shchepetkin2005(opts)
        }
        StretchingFunctionKind::Shchepetkin2010 => {
            let opts = Shchepetkin2010Opts::new(
                cli.etal.as_ref().unwrap(),
                cli.a_vqs0.as_ref().unwrap(),
                cli.theta_s.as_ref().unwrap(),
                cli.theta_b.as_ref().unwrap(),
                cli.hc.as_ref().unwrap(),
            );
            StretchingFunction::Shchepetkin2010(opts)
        }
        StretchingFunctionKind::Geyer => {
            let opts = GeyerOpts::new(
                cli.etal.as_ref().unwrap(),
                cli.a_vqs0.as_ref().unwrap(),
                cli.theta_s.as_ref().unwrap(),
                cli.theta_b.as_ref().unwrap(),
                cli.hc.as_ref().unwrap(),
            );
            StretchingFunction::Geyer(opts)
        }
    };

    // Build VQS
    let mut builder = VQSBuilder::default();
    builder.hgrid(&hgrid);
    builder.depths(&cli.depths);
    builder.nlevels(&cli.nlevels);
    if let Some(dz_min) = &cli.dz_bottom_min {
        builder.dz_bottom_min(dz_min);
    }
    builder.stretching(&transform);
    let vqs = builder.build()?;

    // Write output
    if let Some(output_path) = &cli.output_filepath {
        vqs.write_to_file(output_path)?;
        println!("Wrote {}", output_path.display());
    }

    // Handle plotting
    if cli.show_zmas_plot || cli.save_zmas_plot.is_some() {
        let zmas_plot = vqs.make_z_mas_plot()?;
        if let Some(save_path) = &cli.save_zmas_plot {
            zmas_plot.write_html(save_path);
            println!("Saved plot to {}", save_path.display());
        }
        if cli.show_zmas_plot {
            zmas_plot.show();
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    match entrypoint() {
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}
