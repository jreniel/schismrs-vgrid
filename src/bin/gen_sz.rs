use clap::Parser;
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::sz::SZBuilder;
use std::process::ExitCode;
use std::{error::Error, path::PathBuf};

fn greater_than_two(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("`{}` isn't a valid positive integer", s))?;

    if value >= 2 {
        Ok(value)
    } else {
        Err(format!(
            "The value must be greater or equal than 2, got {}",
            value
        ))
    }
}

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
struct Cli {
    hgrid_path: PathBuf,
    #[clap(short, long)]
    output_filepath: Option<PathBuf>,
    #[clap(
        long,
        help = "Number of sigma-levels. Must be an integer greater or equal than 0.",
        value_parser=clap::builder::ValueParser::new(greater_than_two),
        default_value = "2",
        )
    ]
    slevels: Option<usize>,
    #[clap(long, value_delimiter = ' ', num_args = 1..,
        help = "Space delimited list of depths for each z-level. \
                Optional. Defaults to pure sigma grid."
        )]
    zlevels: Option<Vec<f64>>,

    #[clap(
        long,
        default_value = "0.1",
        help = "Range is (0., 20.]. Values closer to 0. make the transformation \
                more similar to traditional sigma. Larger values will increase \
                resolution at the top and bottom."
    )]
    theta_f: Option<f64>,

    #[clap(
        long,
        default_value = "0.",
        help = "Range is [0., 1.]. For values closer to 0. the surface is \
                resolved. For values closer to 1., both the surface and bottom \
                are resolved."
    )]
    theta_b: Option<f64>,
    #[clap(
        long,
        help = "Critical layer depth. Value must be > 5.",
        alias = "hc",
        default_value = "5."
    )]
    critical_depth: f64,
    #[clap(
        short,
        long,
        default_value = "0.",
        help = "Water level offset. Not typically needed."
    )]
    etal: Option<f64>,
    #[clap(long, action)]
    show_plot: bool,
    #[clap(long)]
    save_plot: Option<PathBuf>,
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    let hgrid = Hgrid::try_from(&cli.hgrid_path)?;
    let mut builder = SZBuilder::default();
    builder.hgrid(&hgrid);
    builder.slevels(&cli.slevels.as_ref().unwrap());
    builder.theta_f(&cli.theta_f.as_ref().unwrap());
    builder.theta_b(&cli.theta_b.as_ref().unwrap());
    builder.critical_depth(&cli.critical_depth);
    builder.etal(&cli.etal.as_ref().unwrap());
    if cli.zlevels.is_some() {
        builder.zlevels(cli.zlevels.as_ref().unwrap());
    }
    let sz = builder.build()?;
    if cli.output_filepath.is_some() {
        sz.write_to_file(&cli.output_filepath.as_ref().unwrap())?;
    } else {
        println!("{}", sz)
    };

    if cli.show_plot || cli.save_plot.is_some() {
        let zcor_plot = sz.make_vertical_distribution_plot(10)?;
        if cli.show_plot {
            zcor_plot.show();
        }
    }
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
