use actix_files::NamedFile;
use actix_web::{HttpRequest, Result};
use std::path::PathBuf;

pub async fn file_helper(req: HttpRequest, base_path: &str) -> Result<NamedFile> {
    // Build the requested file path relative to the base_path
    let file_path: PathBuf = req.match_info().query("filename").parse().unwrap_or_default();
    let full_path = PathBuf::from(base_path).join(file_path);

    Ok(NamedFile::open(full_path)?)
}


std::fs::create_dir_all("./uploads").expect("Could not create uploads folder");