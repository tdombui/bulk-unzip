use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

mod metadata_stripper;
use metadata_stripper::{bulk_strip_metadata, MetadataArgs};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Bulk extract zip files
    Unzip {
        /// Directory containing zip files to extract
        #[arg(short, long, default_value = ".")]
        directory: PathBuf,

        /// Output directory for extracted files
        #[arg(short, long, default_value = "extracted")]
        output: PathBuf,

        /// Number of concurrent extractions
        #[arg(short, long, default_value = "4")]
        workers: usize,

        /// Skip existing extracted directories
        #[arg(short, long)]
        skip_existing: bool,
    },
    
    /// Strip metadata from MP3 files
    Strip {
        /// Directory containing MP3 files to process
        #[arg(short, long, default_value = ".")]
        directory: PathBuf,

        /// Output directory for processed files (if not specified, files are modified in place)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Number of concurrent workers
        #[arg(short, long, default_value = "4")]
        workers: usize,

        /// Skip files that already have no metadata
        #[arg(short, long)]
        skip_clean: bool,

        /// Keep only specific metadata fields (comma-separated: title,artist,album,year)
        #[arg(short, long)]
        keep_fields: Option<String>,

        /// Remove all metadata completely
        #[arg(short, long)]
        remove_all: bool,

        /// Show what would be done without actually doing it
        #[arg(short, long)]
        dry_run: bool,
    },
}

#[derive(Clone)]
struct ZipFile {
    path: PathBuf,
    size: u64,
}

async fn find_zip_files(directory: &Path) -> Result<Vec<ZipFile>> {
    let mut zip_files = Vec::new();
    
    for entry in WalkDir::new(directory)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "zip") {
            let metadata = fs::metadata(path)
                .with_context(|| format!("Failed to read metadata for {:?}", path))?;
            zip_files.push(ZipFile {
                path: path.to_path_buf(),
                size: metadata.len(),
            });
        }
    }
    
    Ok(zip_files)
}

async fn extract_zip_file(
    zip_file: &ZipFile,
    output_dir: &Path,
    skip_existing: bool,
    progress_bar: ProgressBar,
) -> Result<()> {
    let file_name = zip_file.path.file_stem().unwrap().to_string_lossy();
    let extract_dir = output_dir.join(&*file_name);
    
    // Skip if directory exists and skip_existing is true
    if skip_existing && extract_dir.exists() {
        progress_bar.finish_with_message(format!("Skipped existing: {}", file_name));
        return Ok(());
    }
    
    // Create extraction directory
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("Failed to create directory {:?}", extract_dir))?;
    
    // Open zip file
    let file = fs::File::open(&zip_file.path)
        .with_context(|| format!("Failed to open zip file {:?}", zip_file.path))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive {:?}", zip_file.path))?;
    
    let total_entries = archive.len();
    progress_bar.set_length(total_entries as u64);
    
    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .with_context(|| format!("Failed to read file at index {} in {:?}", i, zip_file.path))?;
        
        let outpath = extract_dir.join(file.name());
        
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory {:?}", outpath))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)
                        .with_context(|| format!("Failed to create parent directory {:?}", p))?;
                }
            }
            
            let mut outfile = fs::File::create(&outpath)
                .with_context(|| format!("Failed to create file {:?}", outpath))?;
            
            std::io::copy(&mut file, &mut outfile)
                .with_context(|| format!("Failed to write file {:?}", outpath))?;
        }
        
        progress_bar.inc(1);
    }
    
    progress_bar.finish_with_message(format!("Completed: {}", file_name));
    Ok(())
}

async fn bulk_unzip(directory: PathBuf, output: PathBuf, workers: usize, skip_existing: bool) -> Result<()> {
    println!("🔍 Scanning for zip files in {:?}...", directory);
    let zip_files = find_zip_files(&directory).await?;
    
    if zip_files.is_empty() {
        println!("❌ No zip files found in {:?}", directory);
        return Ok(());
    }
    
    println!("📦 Found {} zip files:", zip_files.len());
    let total_size: u64 = zip_files.iter().map(|f| f.size).sum();
    println!("📊 Total size: {:.2} GB", total_size as f64 / 1024.0 / 1024.0 / 1024.0);
    
    // Create output directory
    fs::create_dir_all(&output)
        .with_context(|| format!("Failed to create output directory {:?}", output))?;
    
    // Setup progress tracking
    let multi_progress = MultiProgress::new();
    let style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
        .unwrap()
        .progress_chars("#>-");
    
    // Process zip files with limited concurrency
    let chunks: Vec<_> = zip_files
        .chunks((zip_files.len() + workers - 1) / workers)
        .collect();
    
    let futures: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec();
            let output_dir = output.clone();
            let skip_existing = skip_existing;
            let multi_progress = multi_progress.clone();
            let style = style.clone();
            
            async move {
                for zip_file in chunk {
                    let progress_bar = multi_progress.add(ProgressBar::new(0));
                    progress_bar.set_style(style.clone());
                    progress_bar.set_message(format!("Extracting: {}", zip_file.path.file_name().unwrap().to_string_lossy()));
                    
                    if let Err(e) = extract_zip_file(&zip_file, &output_dir, skip_existing, progress_bar).await {
                        eprintln!("❌ Error extracting {:?}: {}", zip_file.path, e);
                    }
                }
            }
        })
        .collect();
    
    // Wait for all extractions to complete
    join_all(futures).await;
    
    println!("✅ Bulk extraction completed! Files extracted to: {:?}", output);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    match args.command {
        Commands::Unzip { directory, output, workers, skip_existing } => {
            bulk_unzip(directory, output, workers, skip_existing).await
        }
        Commands::Strip { directory, output, workers, skip_clean, keep_fields, remove_all, dry_run } => {
            let metadata_args = MetadataArgs {
                directory,
                output,
                workers,
                skip_clean,
                keep_fields,
                remove_all,
                dry_run,
            };
            bulk_strip_metadata(metadata_args).await
        }
    }
} 