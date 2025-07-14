use anyhow::{Context, Result};
use clap::Parser;
use futures::future::join_all;
use id3::{Tag, TagLike};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct MetadataArgs {
    /// Directory containing MP3 files to process
    #[arg(short, long, default_value = ".")]
    pub directory: PathBuf,

    /// Output directory for processed files (if not specified, files are modified in place)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Number of concurrent workers
    #[arg(short, long, default_value = "4")]
    pub workers: usize,

    /// Skip files that already have no metadata
    #[arg(short, long)]
    pub skip_clean: bool,

    /// Keep only specific metadata fields (comma-separated: title,artist,album,year)
    #[arg(short, long)]
    pub keep_fields: Option<String>,

    /// Remove all metadata completely
    #[arg(short, long)]
    pub remove_all: bool,

    /// Show what would be done without actually doing it
    #[arg(short, long)]
    pub dry_run: bool,
}

#[derive(Clone)]
pub struct Mp3File {
    path: PathBuf,
    size: u64,
    has_metadata: bool,
}

pub async fn find_mp3_files(directory: &Path) -> Result<Vec<Mp3File>> {
    let mut mp3_files = Vec::new();
    
    for entry in WalkDir::new(directory)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "mp3") {
            let metadata = fs::metadata(path)
                .with_context(|| format!("Failed to read metadata for {:?}", path))?;
            
            let has_metadata = Tag::read_from_path(path).is_ok();
            
            mp3_files.push(Mp3File {
                path: path.to_path_buf(),
                size: metadata.len(),
                has_metadata,
            });
        }
    }
    
    Ok(mp3_files)
}

pub async fn strip_metadata_file(
    mp3_file: &Mp3File,
    output_dir: Option<&Path>,
    keep_fields: Option<&str>,
    remove_all: bool,
    dry_run: bool,
    progress_bar: ProgressBar,
) -> Result<()> {
    let file_name = mp3_file.path.file_name().unwrap().to_string_lossy();
    
    // Determine output path
    let output_path = if let Some(output_dir) = output_dir {
        output_dir.join(&*file_name)
    } else {
        mp3_file.path.clone()
    };
    
    if !dry_run {
        // Create output directory if needed
        if let Some(output_dir) = output_dir {
            fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create directory {:?}", output_dir))?;
        }
        
        // Copy file to output location if different
        if output_path != mp3_file.path {
            fs::copy(&mp3_file.path, &output_path)
                .with_context(|| format!("Failed to copy file from {:?} to {:?}", mp3_file.path, output_path))?;
        }
        
        // Process metadata
        if let Ok(tag) = Tag::read_from_path(&output_path) {
            if remove_all {
                // Remove all metadata by writing an empty tag
                let empty_tag = Tag::new();
                empty_tag.write_to_path(&output_path, id3::Version::Id3v24)
                    .with_context(|| format!("Failed to write stripped metadata to {:?}", output_path))?;
            } else if let Some(fields_to_keep) = keep_fields {
                // Keep only specified fields
                let fields: Vec<&str> = fields_to_keep.split(',').collect();
                let mut new_tag = Tag::new();
                
                for field in fields {
                    match field.trim() {
                        "title" => {
                            if let Some(title) = tag.title() {
                                new_tag.set_title(title);
                            }
                        }
                        "artist" => {
                            if let Some(artist) = tag.artist() {
                                new_tag.set_artist(artist);
                            }
                        }
                        "album" => {
                            if let Some(album) = tag.album() {
                                new_tag.set_album(album);
                            }
                        }
                        "year" => {
                            if let Some(year) = tag.year() {
                                new_tag.set_year(year);
                            }
                        }
                        "track" => {
                            if let Some(track) = tag.track() {
                                new_tag.set_track(track);
                            }
                        }
                        "genre" => {
                            if let Some(genre) = tag.genre() {
                                new_tag.set_genre(genre);
                            }
                        }
                        _ => {
                            // Try to copy custom frames
                            if let Some(frame) = tag.get(field) {
                                new_tag.add_frame(frame.clone());
                            }
                        }
                    }
                }
                
                // Replace the tag
                new_tag.write_to_path(&output_path, id3::Version::Id3v24)
                    .with_context(|| format!("Failed to write filtered metadata to {:?}", output_path))?;
            }
        }
    }
    
    progress_bar.finish_with_message(format!("Processed: {}", file_name));
    Ok(())
}

pub async fn bulk_strip_metadata(args: MetadataArgs) -> Result<()> {
    println!("🔍 Scanning for MP3 files in {:?}...", args.directory);
    let mp3_files = find_mp3_files(&args.directory).await?;
    
    if mp3_files.is_empty() {
        println!("❌ No MP3 files found in {:?}", args.directory);
        return Ok(());
    }
    
    let files_with_metadata: Vec<_> = mp3_files.iter()
        .filter(|f| f.has_metadata)
        .collect();
    
    println!("📦 Found {} MP3 files:", mp3_files.len());
    println!("📊 Files with metadata: {}", files_with_metadata.len());
    let total_size: u64 = mp3_files.iter().map(|f| f.size).sum();
    println!("📊 Total size: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
    
    if args.skip_clean && files_with_metadata.is_empty() {
        println!("✅ All files are already clean (no metadata found)");
        return Ok(());
    }
    
    // Create output directory if specified
    if let Some(ref output_dir) = args.output {
        fs::create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output directory {:?}", output_dir))?;
    }
    
    // Setup progress tracking
    let multi_progress = MultiProgress::new();
    let style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
        .unwrap()
        .progress_chars("#>-");
    
    // Process files with limited concurrency
    let files_to_process: Vec<_> = if args.skip_clean {
        files_with_metadata.into_iter().cloned().collect()
    } else {
        mp3_files
    };
    
    let chunks: Vec<_> = files_to_process
        .chunks((files_to_process.len() + args.workers - 1) / args.workers)
        .collect();
    
    let futures: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let chunk = chunk.to_vec();
            let output_dir = args.output.clone();
            let keep_fields = args.keep_fields.clone();
            let remove_all = args.remove_all;
            let dry_run = args.dry_run;
            let multi_progress = multi_progress.clone();
            let style = style.clone();
            
            async move {
                for mp3_file in chunk {
                    let progress_bar = multi_progress.add(ProgressBar::new(1));
                    progress_bar.set_style(style.clone());
                    progress_bar.set_message(format!("Processing: {}", mp3_file.path.file_name().unwrap().to_string_lossy()));
                    
                    if let Err(e) = strip_metadata_file(
                        &mp3_file,
                        output_dir.as_deref(),
                        keep_fields.as_deref(),
                        remove_all,
                        dry_run,
                        progress_bar,
                    ).await {
                        eprintln!("❌ Error processing {:?}: {}", mp3_file.path, e);
                    }
                }
            }
        })
        .collect();
    
    // Wait for all processing to complete
    join_all(futures).await;
    
    if args.dry_run {
        println!("🔍 Dry run completed! No files were modified.");
    } else {
        println!("✅ Bulk metadata stripping completed!");
        if let Some(ref output_dir) = args.output {
            println!("📁 Processed files saved to: {:?}", output_dir);
        } else {
            println!("📁 Files were modified in place");
        }
    }
    
    Ok(())
} 