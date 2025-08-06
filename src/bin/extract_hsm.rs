// src/bin/extract_hsm.rs

use clap::{Parser, ValueEnum};
use pretty_env_logger;
use schismrs_hgrid::hgrid::Hgrid;
use schismrs_vgrid::vqs::VQS;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    author,
    about = "Extract master grids from existing SCHISM VQS vertical grid",
    long_about = "Analyzes a SCHISM VQS vertical grid file (vgrid.in) and horizontal grid file (hgrid.gr3) \
                  to reverse-engineer the master depth/level pairs that were likely used to generate the VQS. \
                  This focuses on wet nodes only and uses the actual level counts from the sigma array."
)]
struct Cli {
    /// Path to the horizontal grid file (hgrid.gr3)
    hgrid_path: PathBuf,
    
    /// Path to the vertical grid file (vgrid.in)
    vgrid_path: PathBuf,
    
    #[clap(short, long, help = "Output file for the extracted master grids")]
    output: Option<PathBuf>,
    
    #[clap(short, long, default_value = "table", help = "Output format")]
    format: OutputFormat,
    
    #[clap(long, action, help = "Show detailed analysis information")]
    verbose: bool,
    
    #[clap(long, action, help = "Generate analysis statistics")]
    stats: bool,
    
    #[clap(long, help = "Save analysis statistics to file")]
    stats_output: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug)]
enum OutputFormat {
    /// Human-readable table format
    Table,
    /// CSV format for spreadsheet import
    Csv,
    /// JSON format for programmatic use
    Json,
    /// SCHISM hsm format (depths and levels as separate lines)
    Hsm,
    /// Fortran array format (for direct use in Fortran code)
    Fortran,
}

#[derive(Debug)]
struct ExtractionResults {
    master_depths: Vec<f64>,
    master_levels: Vec<usize>,
    total_nodes: usize,
    wet_nodes: usize,
    dry_nodes: usize,
    max_depth: f64,
    min_depth: f64,
    max_levels: usize,
    min_levels: usize,
}

#[derive(Debug)]
struct AnalysisStats {
    depth_distribution: Vec<(f64, usize)>, // (depth_bin, node_count)
    level_distribution: Vec<(usize, usize)>, // (level_count, node_count)
    depth_level_pairs: Vec<(f64, usize)>, // all wet node (depth, level) pairs
}

fn main() -> ExitCode {
    match entrypoint() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn entrypoint() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let cli = Cli::parse();
    
    if cli.verbose {
        println!("Loading horizontal grid from: {}", cli.hgrid_path.display());
    }
    
    // Load horizontal grid
    let hgrid = Hgrid::try_from(&cli.hgrid_path)
        .map_err(|e| format!("Failed to load horizontal grid: {}", e))?;
    
    if cli.verbose {
        println!("Loaded hgrid with {} nodes", hgrid.depths().len());
        println!("Loading vertical grid from: {}", cli.vgrid_path.display());
    }
    
    // Load vertical grid using the new try_from_file functionality
    let vqs = VQS::try_from_file(&hgrid, &cli.vgrid_path)
        .map_err(|e| format!("Failed to load vertical grid: {}", e))?;
    
    if cli.verbose {
        println!("Loaded VQS with {} vertical levels", vqs.nvrt());
        println!("Analyzing master grid structure...");
    }
    
    // Extract master grids
    let (master_depths, master_levels) = vqs.extract_master_grids(&hgrid)
        .map_err(|e| format!("Failed to extract master grids: {}", e))?;
    
    // Prepare results
    let results = prepare_extraction_results(&vqs, &hgrid, master_depths, master_levels)?;
    
    if cli.verbose {
        print_analysis_summary(&results);
    }
    
    // Output results
    let output_content = format_output(&results, &cli.format)?;
    
    match cli.output {
        Some(ref path) => {
            let mut file = File::create(path)?;
            write!(file, "{}", output_content)?;
            println!("Master grids written to: {}", path.display());
        }
        None => {
            print!("{}", output_content);
        }
    }
    
    // Generate statistics if requested
    if cli.stats || cli.stats_output.is_some() {
        if cli.verbose {
            println!("\nGenerating analysis statistics...");
        }
        
        let stats = generate_analysis_stats(&vqs, &hgrid)?;
        let stats_content = format_stats(&stats, &results)?;
        
        if let Some(ref path) = cli.stats_output {
            let mut file = File::create(path)?;
            write!(file, "{}", stats_content)?;
            println!("Analysis statistics written to: {}", path.display());
        }
        
        if cli.stats {
            println!("\n{}", stats_content);
        }
    }
    
    Ok(())
}

fn prepare_extraction_results(
    vqs: &VQS, 
    hgrid: &Hgrid, 
    master_depths: Vec<f64>, 
    master_levels: Vec<usize>
) -> Result<ExtractionResults, Box<dyn Error>> {
    let bathymetry = hgrid.depths();
    let depths: Vec<f64> = bathymetry.iter().map(|&d| -d).collect();
    
    let mut wet_nodes = 0;
    let mut dry_nodes = 0;
    let mut max_depth = 0.0f64;
    let mut min_depth = f64::INFINITY;
    let mut max_levels = 0;
    let mut min_levels = usize::MAX;
    
    for (node_idx, &depth) in depths.iter().enumerate() {
        if depth <= 0.0 {
            dry_nodes += 1;
            continue;
        }
        
        wet_nodes += 1;
        max_depth = max_depth.max(depth);
        min_depth = min_depth.min(depth);
        
        let levels = count_levels_at_node(vqs, node_idx);
        if levels > 0 {
            max_levels = std::cmp::max(max_levels, levels);
            min_levels = std::cmp::min(min_levels, levels);
        }
    }
    
    if min_depth == f64::INFINITY {
        min_depth = 0.0f64;
    }
    if min_levels == usize::MAX {
        min_levels = 0;
    }
    
    Ok(ExtractionResults {
        master_depths,
        master_levels,
        total_nodes: bathymetry.len(),
        wet_nodes,
        dry_nodes,
        max_depth,
        min_depth,
        max_levels,
        min_levels,
    })
}

fn count_levels_at_node(vqs: &VQS, node_idx: usize) -> usize {
    // Count non-NaN and non -9.0 values in the sigma array for this node
    let mut count = 0;
    for level in 0..vqs.nvrt() {
        let sigma_val = vqs.sigma()[[level, node_idx]];
        // Check if the value is valid (not NaN and not -9.0 which indicates below bottom)
        if !sigma_val.is_nan() && (sigma_val - (-9.0)).abs() > 1e-10 {
            count += 1;
        }
    }
    count
}

fn print_analysis_summary(results: &ExtractionResults) {
    println!("=== VQS Analysis Summary ===");
    println!("Total nodes: {}", results.total_nodes);
    println!("Wet nodes: {} ({:.1}%)", 
             results.wet_nodes, 
             100.0 * results.wet_nodes as f64 / results.total_nodes as f64);
    println!("Dry nodes: {} ({:.1}%)", 
             results.dry_nodes, 
             100.0 * results.dry_nodes as f64 / results.total_nodes as f64);
    println!("Depth range: {:.1}m to {:.1}m", results.min_depth, results.max_depth);
    println!("Level range: {} to {}", results.min_levels, results.max_levels);
    println!("Extracted {} master grids", results.master_depths.len());
    println!();
}

fn generate_analysis_stats(vqs: &VQS, hgrid: &Hgrid) -> Result<AnalysisStats, Box<dyn Error>> {
    let bathymetry = hgrid.depths();
    let depths: Vec<f64> = bathymetry.iter().map(|&d| -d).collect();
    
    let mut depth_level_pairs = Vec::new();
    let mut depth_bins = std::collections::HashMap::new();
    let mut level_bins = std::collections::HashMap::new();
    
    let depth_bin_size = 5.0; // 5m bins
    
    for (node_idx, &depth) in depths.iter().enumerate() {
        if depth <= 0.0 {
            continue; // Skip dry nodes
        }
        
        let levels = count_levels_at_node(vqs, node_idx);
        if levels < 2 {
            continue; // Skip nodes with too few levels
        }
        
        depth_level_pairs.push((depth, levels));
        
        // Bin by depth
        let depth_bin = (depth / depth_bin_size).floor() as usize;
        *depth_bins.entry(depth_bin).or_insert(0) += 1;
        
        // Bin by levels
        *level_bins.entry(levels).or_insert(0) += 1;
    }
    
    // Convert to sorted vectors
    let mut depth_distribution: Vec<_> = depth_bins.into_iter()
        .map(|(bin, count)| (bin as f64 * depth_bin_size, count))
        .collect();
    depth_distribution.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    
    let mut level_distribution: Vec<_> = level_bins.into_iter().collect();
    level_distribution.sort_by_key(|&(levels, _)| levels);
    
    Ok(AnalysisStats {
        depth_distribution,
        level_distribution,
        depth_level_pairs,
    })
}

fn format_output(results: &ExtractionResults, format: &OutputFormat) -> Result<String, Box<dyn Error>> {
    match format {
        OutputFormat::Table => format_table_output(results),
        OutputFormat::Csv => format_csv_output(results),
        OutputFormat::Json => format_json_output(results),
        OutputFormat::Hsm => format_hsm_output(results),
        OutputFormat::Fortran => format_fortran_output(results),
    }
}

fn format_table_output(results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("Master Grid Analysis Results\n");
    output.push_str("============================\n\n");
    
    output.push_str(&format!("Grid #{:<3} | Depth (m) | Levels\n", ""));
    output.push_str("---------|-----------|-------\n");
    
    for (i, (depth, levels)) in results.master_depths.iter().zip(results.master_levels.iter()).enumerate() {
        output.push_str(&format!("{:8} | {:9.1} | {:6}\n", i + 1, depth, levels));
    }
    
    output.push_str("\nGrid Summary:\n");
    output.push_str(&format!("- {} master grids identified\n", results.master_depths.len()));
    output.push_str(&format!("- Depth range: {:.1}m to {:.1}m\n", results.min_depth, results.max_depth));
    output.push_str(&format!("- Level range: {} to {}\n", results.min_levels, results.max_levels));
    
    output.push_str("\nNode Summary:\n");
    output.push_str(&format!("- Total nodes: {}\n", results.total_nodes));
    output.push_str(&format!("- Wet nodes: {} ({:.1}%)\n", 
                            results.wet_nodes,
                            100.0 * results.wet_nodes as f64 / results.total_nodes as f64));
    output.push_str(&format!("- Dry nodes: {} ({:.1}%)\n", 
                            results.dry_nodes,
                            100.0 * results.dry_nodes as f64 / results.total_nodes as f64));
    
    Ok(output)
}

fn format_csv_output(results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("grid_number,depth_m,levels\n");
    
    for (i, (depth, levels)) in results.master_depths.iter().zip(results.master_levels.iter()).enumerate() {
        output.push_str(&format!("{},{:.1},{}\n", i + 1, depth, levels));
    }
    
    Ok(output)
}

fn format_json_output(results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str("  \"master_grids\": [\n");
    
    for (i, (depth, levels)) in results.master_depths.iter().zip(results.master_levels.iter()).enumerate() {
        if i > 0 {
            output.push_str(",\n");
        }
        output.push_str("    {\n");
        output.push_str(&format!("      \"grid_number\": {},\n", i + 1));
        output.push_str(&format!("      \"depth_m\": {:.1},\n", depth));
        output.push_str(&format!("      \"levels\": {}\n", levels));
        output.push_str("    }");
    }
    
    output.push_str("\n  ],\n");
    output.push_str("  \"summary\": {\n");
    output.push_str(&format!("    \"total_grids\": {},\n", results.master_depths.len()));
    output.push_str("    \"depth_range\": {\n");
    output.push_str(&format!("      \"min_m\": {:.1},\n", results.min_depth));
    output.push_str(&format!("      \"max_m\": {:.1}\n", results.max_depth));
    output.push_str("    },\n");
    output.push_str("    \"level_range\": {\n");
    output.push_str(&format!("      \"min\": {},\n", results.min_levels));
    output.push_str(&format!("      \"max\": {}\n", results.max_levels));
    output.push_str("    },\n");
    output.push_str("    \"nodes\": {\n");
    output.push_str(&format!("      \"total\": {},\n", results.total_nodes));
    output.push_str(&format!("      \"wet\": {},\n", results.wet_nodes));
    output.push_str(&format!("      \"dry\": {},\n", results.dry_nodes));
    output.push_str(&format!("      \"wet_percentage\": {:.1}\n", 
                            100.0 * results.wet_nodes as f64 / results.total_nodes as f64));
    output.push_str("    }\n");
    output.push_str("  }\n");
    output.push_str("}\n");
    
    Ok(output)
}

fn format_hsm_output(results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("! Extracted master grids for SCHISM VQS\n");
    output.push_str(&format!("! Found {} master grids from {} wet nodes (out of {} total)\n", 
                            results.master_depths.len(), results.wet_nodes, results.total_nodes));
    output.push_str("!\n");
    output.push_str("! Master depths (positive down, meters):\n");
    output.push_str("hsm = (/ ");
    
    for (i, depth) in results.master_depths.iter().enumerate() {
        if i > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("{:.1}", depth));
    }
    output.push_str(" /)\n");
    
    output.push_str("!\n");
    output.push_str("! Corresponding number of levels:\n");
    output.push_str("nv_vqs = (/ ");
    for (i, levels) in results.master_levels.iter().enumerate() {
        if i > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("{}", levels));
    }
    output.push_str(" /)\n");
    
    Ok(output)
}

fn format_fortran_output(results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("! Extracted master grids for SCHISM VQS (Fortran format)\n");
    output.push_str(&format!("! Analysis results: {} grids from {} wet nodes\n", 
                            results.master_depths.len(), results.wet_nodes));
    output.push_str("!\n");
    
    output.push_str(&format!("      m_vqs = {}\n", results.master_depths.len()));
    output.push_str("      allocate(hsm(m_vqs), nv_vqs(m_vqs))\n");
    output.push_str("      \n");
    
    output.push_str("      ! Master depths (positive down)\n");
    output.push_str("      hsm = (/ &\n");
    for (i, depth) in results.master_depths.iter().enumerate() {
        if i == results.master_depths.len() - 1 {
            output.push_str(&format!("        {:.1} &\n", depth));
        } else {
            output.push_str(&format!("        {:.1}, &\n", depth));
        }
    }
    output.push_str("      /)\n");
    output.push_str("      \n");
    
    output.push_str("      ! Number of levels for each master grid\n");
    output.push_str("      nv_vqs = (/ &\n");
    for (i, levels) in results.master_levels.iter().enumerate() {
        if i == results.master_levels.len() - 1 {
            output.push_str(&format!("        {} &\n", levels));
        } else {
            output.push_str(&format!("        {}, &\n", levels));
        }
    }
    output.push_str("      /)\n");
    
    Ok(output)
}

fn format_stats(stats: &AnalysisStats, results: &ExtractionResults) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.push_str("=== Analysis Statistics ===\n\n");
    
    output.push_str("Depth Distribution (5m bins):\n");
    output.push_str("Depth Range (m) | Node Count\n");
    output.push_str("----------------|----------\n");
    for (depth_bin, count) in &stats.depth_distribution {
        output.push_str(&format!("{:6.1} - {:6.1} | {:8}\n", depth_bin, depth_bin + 5.0, count));
    }
    output.push_str("\n");
    
    output.push_str("Level Distribution:\n");
    output.push_str("Levels | Node Count\n");
    output.push_str("-------|----------\n");
    for (levels, count) in &stats.level_distribution {
        output.push_str(&format!("{:6} | {:8}\n", levels, count));
    }
    output.push_str("\n");
    
    output.push_str(&format!("Total depth-level data points: {}\n", stats.depth_level_pairs.len()));
    output.push_str(&format!("Extracted master grids: {}\n", results.master_depths.len()));
    output.push_str(&format!("Data reduction: {:.1}% (from {} to {} representative points)\n",
                            100.0 * (1.0 - results.master_depths.len() as f64 / stats.depth_level_pairs.len() as f64),
                            stats.depth_level_pairs.len(),
                            results.master_depths.len()));
    
    Ok(output)
}