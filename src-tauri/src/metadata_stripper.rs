use anyhow::{Context, Result};
use futures::future::join_all;
use id3::{Tag, TagLike};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct MetadataArgs {
    pub directory: PathBuf,
    pub output: Option<PathBuf>,
    pub workers: usize,
    pub skip_clean: bool,
    pub keep_fields: Option<String>,
    pub remove_all: bool,
    pub dry_run: bool,
}

#[derive(Clone)]
pub struct Mp3File {
    pub path: PathBuf,
    pub size: u64,
    pub has_metadata: bool,
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
    
    Ok(())
}

pub async fn bulk_strip_metadata(args: MetadataArgs) -> Result<()> {
    let mp3_files = find_mp3_files(&args.directory).await?;
    
    if mp3_files.is_empty() {
        return Ok(());
    }
    
    let files_with_metadata: Vec<_> = mp3_files.iter()
        .filter(|f| f.has_metadata)
        .collect();
    
    if args.skip_clean && files_with_metadata.is_empty() {
        return Ok(());
    }
    
    // Create output directory if specified
    if let Some(ref output_dir) = args.output {
        fs::create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output directory {:?}", output_dir))?;
    }
    
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
            
            async move {
                for mp3_file in chunk {
                    if let Err(e) = strip_metadata_file(
                        &mp3_file,
                        output_dir.as_deref(),
                        keep_fields.as_deref(),
                        remove_all,
                        dry_run,
                    ).await {
                        eprintln!("❌ Error processing {:?}: {}", mp3_file.path, e);
                    }
                }
            }
        })
        .collect();
    
    // Wait for all processing to complete
    join_all(futures).await;
    
    Ok(())
} 