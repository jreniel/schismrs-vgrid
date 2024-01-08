use clap::{Args, Parser, Subcommand};
use pretty_env_logger;
// use rayon::prelude::*;
// use schismrs_lsc2::master_vqs::MasterVQSBuilder;
use schismrs_lsc2::transforms::StretchingFunction;
use schismrs_lsc2::vqs::VQSKMeansBuilder;
use schismrs_mesh::hgrid::Hgrid;
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
    #[clap(subcommand)]
    mode: Modes,
}

#[derive(Subcommand, Debug)]
enum Modes {
    Auto(AutoCliOpts),
}

#[derive(Args, Debug)]
struct AutoCliOpts {
    hgrid_path: PathBuf,
    clusters: usize,
    output_filepath: PathBuf,
    levels_per_cluster: Vec<usize>,
}

fn run_auto_lsc2(auto_cli_opts: &AutoCliOpts) -> Result<(), Box<dyn Error>> {
    let hgrid = Hgrid::try_from(&auto_cli_opts.hgrid_path)?;
    let vqs = VQSKMeansBuilder::default()
        .hgrid(&hgrid)
        .stretching(StretchingFunction::Quadratic(None))
        .nclusters(auto_cli_opts.clusters)
        .build()?;
    // println!("{}", vqs);
    // vqs.write_to_file(&auto_cli_opts.output_filepath)?;
    Ok(())
}

//fn _run_harcoded_example(auto_cli_opts: &AutoCliOpts) -> Result<(), Box<dyn Error>> {
//    // use schismrs_lsc2::transforms::StretchingFunction;
//    let hsm = vec![
//        50, 60, 80, 110, 150, 200, 260, 330, 410, 500, 600, 710, 830, 960, 1100, 1250, 1410, 1580,
//        1760,
//    ];
//    let m_vqs = hsm.len();
//    let nv_vqs: Vec<_> = (1..=m_vqs).map(|i| 14 + i).collect();
//    let hgrid = Hgrid::try_from(&auto_cli_opts.hgrid_path)?;
//    // (0..m_vqs).into_par_iter().for_each(|m| {
//    //     let nv = nv_vqs[m];
//    //     (0..nv).for_each(|k| {
//    //         // loop body here
//    //     });
//    // });

//    //
//    //
//    //
//    // let nvrt_m = nv_vqs[m_vqs];
//    // dbg! {nv_vqs};
//    // unimplemented!();
//    Ok(())
//}

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    match &cli.mode {
        Modes::Auto(auto) => {
            run_auto_lsc2(&auto)?;
        }
    }
    Ok(())
    // let _dz_bot_min = 3.0;
    // let mut nv_vqs = [0; 19];
    // let _a_vqs = [-1.0; 19];
    // for i in 1..=m_vqs {
    //     nv_vqs[i - 1] = 14 + i - 1;
    // }
    // if m_vqs < 2 {
    //     println!("Check vgrid.in: {}", m_vqs);
    //     return;
    // }
    // if hsm[0] < 0 {
    //     panic!("hsm(1)<0");
    // }
    // for m in 1..m_vqs {
    //     if hsm[m] <= hsm[m - 1] {
    //         println!("Check hsm: {}, {}, {}", m, hsm[m], hsm[m - 1]);
    //         return;
    //     }
    // }
    // let _a_vqs0 = -1.0;
    // let etal = 0.0;
    // if etal <= -hsm[0] as f64 {
    //     println!("elev<hsm: {}", etal);
    //     return;
    // }
    // let nvrt_m = nv_vqs[m_vqs - 1];
    // println!("nvrt in master vgrid={}", nvrt_m);
    // let mut z_mas = vec![vec![-1.0e5; m_vqs]; nvrt_m];
    // let theta_b = 0.5;
    // let theta_f = 0.5;
    // for m in 0..m_vqs {
    //     for k in 1..=nv_vqs[m] {
    //         let sigma = (k as f64 - 1.0) / (1.0 - nv_vqs[m] as f64);
    //         let cs1 = (1.0 - theta_b) * ((theta_f * sigma).sinh()) / ((theta_f).sinh());
    //         let cs2 = theta_b * ((theta_f * (sigma + 0.5)).tanh() - (theta_f * 0.5).tanh())
    //             / (2.0 * (theta_f * 0.5).tanh());
    //         let cs = cs1 + cs2;
    //         z_mas[k - 1][m] =
    //             etal * (1.0 + sigma) + hsm[0] as f64 * sigma + (hsm[m] - hsm[0]) as f64 * cs;
    //     }
    //     let k = nv_vqs[m];
    //     let _sc_w = vec![0.0; k + 1];
    //     let _Cs_w = vec![0.0; k + 1];
    //     // SIGMA_RUTGERS(k, &mut sc_w, &mut Cs_w);
    //     let V_RUTGERS = vec![0.0; k];
    //     // SIGMA_RUTGERS_VEC(hsm[m] as f64, k, &sc_w, &Cs_w, &mut V_RUTGERS);
    //     for i in 0..k {
    //         z_mas[i][m] = V_RUTGERS[i] * hsm[m] as f64;
    //     }
    //     println!("{:?}", z_mas.iter().map(|v| v[m]).collect::<Vec<_>>());
    // }
}

// let(m_vqs,dz_bot_min)=[19,3.0];
// let mut hsm=[50,60,80,110,150,200,260,330,410,500,600,710,830,960,1100,1250,1410,1580,1760];
// let(mut nv_vqs,a_vqs0)=(vec![0;19],-1.0);
// for i in 1..=m_vqs{nv_vqs[i-1]=14+i-1;}
// if m_vqs<2{ return;}
// if hsm[0]<0{panic!("hsm(1)<0");}
// for m in 1..m_vqs{if hsm[m]<=hsm[m-1]{return;}}
// let(mut etal,nvrt_m)=(0.0,nv_vqs[m_vqs-1]);
// let mut z_mas=vec![vec![-1.0e5;m_vqs];nvrt_m];
// let(theta_b,theta_f)=(0.5,0.5);
// for m in 0..m_vqs{for k in 1..=nv_vqs[m]{
// let(sigma,k)=(k as f64-1.0)/(1.0-nv_vqs[m] as f64);
// let(cs1,cs2)=(1.0-theta_b)*((theta_f*sigma).sinh())/((theta_f).sinh()),
// theta_b*((theta_f*(sigma+0.5)).tanh()-(theta_f*0.5).tanh())/(2.0*(theta_f*0.5).tanh());
// let cs=cs1+cs2;
// z_mas[k-1][m]=etal*(1.0+sigma)+hsm[0] as f64*sigma+(hsm[m]-hsm[0]) as f64*cs;}
// let(mut sc_w,Cs_w,V_RUTGERS)=(vec![0.0;nv_vqs[m]+1],vec![0.0;nv_vqs[m]+1],vec![0.0;nv_vqs[m]]);
// for i in 0..nv_vqs[m]{z_mas[i][m]=V_RUTGERS[i]*hsm[m] as f64;}}
