use anyhow::{Context, Result};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use walkdir::WalkDir;
use zip::ZipArchive;

mod metadata_stripper;
use metadata_stripper::{bulk_strip_metadata, MetadataArgs};

#[derive(Serialize, Deserialize)]
pub struct ZipFile {
    path: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Mp3File {
    path: String,
    size: u64,
    has_metadata: bool,
}

#[derive(Serialize, Deserialize)]
pub struct UnzipProgress {
    current_file: String,
    progress: u64,
    total: u64,
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct StripProgress {
    current_file: String,
    progress: u64,
    total: u64,
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct UnzipOptions {
    directory: String,
    output: String,
    workers: usize,
    skip_existing: bool,
}

#[derive(Serialize, Deserialize)]
pub struct StripOptions {
    directory: String,
    output: Option<String>,
    workers: usize,
    skip_clean: bool,
    keep_fields: Option<String>,
    remove_all: bool,
    dry_run: bool,
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
                path: path.to_string_lossy().to_string(),
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
) -> Result<()> {
    let path = PathBuf::from(&zip_file.path);
    let file_name = path.file_stem().unwrap().to_string_lossy();
    let extract_dir = output_dir.join(&*file_name);
    
    // Skip if directory exists and skip_existing is true
    if skip_existing && extract_dir.exists() {
        return Ok(());
    }
    
    // Create extraction directory
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("Failed to create directory {:?}", extract_dir))?;
    
    // Open zip file
    let file = fs::File::open(&path)
        .with_context(|| format!("Failed to open zip file {:?}", path))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive {:?}", path))?;
    
    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .with_context(|| format!("Failed to read file at index {} in {:?}", i, path))?;
        
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
    }
    
    Ok(())
}

async fn bulk_unzip(options: UnzipOptions) -> Result<Vec<String>> {
    let directory = PathBuf::from(&options.directory);
    let output = PathBuf::from(&options.output);
    
    let zip_files = find_zip_files(&directory).await?;
    
    if zip_files.is_empty() {
        return Ok(vec!["No zip files found".to_string()]);
    }
    
    // Create output directory
    fs::create_dir_all(&output)
        .with_context(|| format!("Failed to create output directory {:?}", output))?;
    
    // Process zip files with limited concurrency
    let chunks: Vec<_> = zip_files
        .chunks((zip_files.len() + options.workers - 1) / options.workers)
        .collect();
    
    let mut results = Vec::new();
    
    for chunk in chunks {
        let futures: Vec<_> = chunk
            .iter()
            .map(|zip_file| {
                let output_dir = output.clone();
                let skip_existing = options.skip_existing;
                
                async move {
                    match extract_zip_file(zip_file, &output_dir, skip_existing).await {
                        Ok(_) => format!("✅ Extracted: {}", zip_file.path),
                        Err(e) => format!("❌ Error extracting {}: {}", zip_file.path, e),
                    }
                }
            })
            .collect();
        
        let chunk_results = join_all(futures).await;
        results.extend(chunk_results);
    }
    
    Ok(results)
}

#[tauri::command]
pub async fn unzip_files(options: UnzipOptions) -> Result<Vec<String>, String> {
    bulk_unzip(options)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn strip_metadata(options: StripOptions) -> Result<Vec<String>, String> {
    let metadata_args = MetadataArgs {
        directory: PathBuf::from(&options.directory),
        output: options.output.map(PathBuf::from),
        workers: options.workers,
        skip_clean: options.skip_clean,
        keep_fields: options.keep_fields,
        remove_all: options.remove_all,
        dry_run: options.dry_run,
    };
    
    bulk_strip_metadata(metadata_args)
        .await
        .map(|_| vec!["Metadata stripping completed".to_string()])
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_zip_files(directory: String) -> Result<Vec<ZipFile>, String> {
    let path = PathBuf::from(directory);
    find_zip_files(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_mp3_files(directory: String) -> Result<Vec<Mp3File>, String> {
    let path = PathBuf::from(directory);
    metadata_stripper::find_mp3_files(&path)
        .await
        .map(|files| {
            files
                .into_iter()
                .map(|f| Mp3File {
                    path: f.path.to_string_lossy().to_string(),
                    size: f.size,
                    has_metadata: f.has_metadata,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
} 