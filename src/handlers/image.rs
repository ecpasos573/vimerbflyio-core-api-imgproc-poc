use actix_web::{post, get, web, HttpResponse, Responder, Error};
use crate::models::image::ImageInfoResponse;

use dotenv::dotenv;
use crate::config;


//use futures_util::StreamExt;
use reqwest::Client;
use serde::{Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf};
use tokio::{fs, fs::File, io::AsyncWriteExt};
use tracing::{error, info};
use url::Url;

use exiftool::ExifTool;

use chromiumoxide::cdp::browser_protocol::network::{EnableParams, EventRequestWillBeSent};
use chromiumoxide::browser::{Browser, BrowserConfig};
use std::panic::{catch_unwind, AssertUnwindSafe};

use scraper::{Html, Selector};

use std::time::Duration;
use tokio::time::sleep;



use actix_multipart::Multipart;
use futures_util::StreamExt as _;
use magick_rust::{magick_wand_genesis, MagickWand, PixelWand};
use std::io::Write;
use tempfile::NamedTempFile;
use magick_rust::bindings::FilterType;

use magick_rust::MagickError;

// Used to make sure MagickWand is initialized exactly once. Note that we
// do not bother shutting down, we simply exit when the tests are done.
use std::sync::Once;
static START: Once = Once::new();



// *************************************************************************************
// * Get image - mainly used for test only
// *************************************************************************************
#[get("/image/{id}")]
pub async fn get_image_info(path: web::Path<String>) -> Result<impl Responder, Error> {
    let image_id = path.into_inner();

    let response = match build_image_response(&image_id).await {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error building image response: {:?}", e);
            return Ok(HttpResponse::InternalServerError().body("Internal Server Error"));
        }
    };

    Ok(HttpResponse::Ok().json(response))
}

// *************************************************************************************
// * Get the exif data of the url
// *************************************************************************************
#[get("/get_image_metadata_info")]
async fn get_image_metadata_info(query: web::Query<HashMap<String, String>>) -> impl Responder {
    let image_url = match query.get("image_url") {
        Some(url) => url,
        None => return HttpResponse::BadRequest().body("Missing image_url parameter"),
    };

    match process_file_from_url(image_url).await {
        Ok(metadata) => {
            info!("File processed: {:?}", metadata);
            HttpResponse::Ok().json(metadata)
        }
        Err(e) => {
            error!("Failed to process file: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}


// *************************************************************************************
// Get all URLs from the web page source
// *************************************************************************************

#[derive(Serialize)]
struct ImagesFromSourceResponse {
    images: Vec<String>,
}

#[get("/getimagesfromsource")]
async fn get_images_from_source(query: web::Query<HashMap<String, String>>) -> actix_web::Result<impl Responder> {
    let page_url = query.get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'url' query parameter"))?
        .to_owned();

    // Launch Chromium browser
    // This bypasses the sandbox entirely.
    // Safe inside Docker, as long as you don’t let untrusted code run inside the container.
    let args: Vec<String> = vec![
        "--no-sandbox".into(),
        "--disable-setuid-sandbox".into(),
        "--disable-dev-shm-usage".into(), // optional, fixes /dev/shm mount issues
    ];

    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder().chrome_executable("/usr/bin/chromium")
        .args(args)
        .build().unwrap())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Browser launch failed: {}", e)))?;

    // Spawn handler task to drive the browser event loop
    actix_rt::spawn(async move {
        while handler.next().await.is_some() {}
    });

    // Open new page and navigate
    let page = browser.new_page(&page_url)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to open page: {}", e)))?;

    // Wait for full page load event
    page.wait_for_navigation()
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to wait for page load: {}", e)))?;


    // Get the total scroll height of the page dynamically
    let scroll_height_js = "document.body.scrollHeight";

    let eval_result = page.evaluate(scroll_height_js).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to get scrollHeight: {}", e)))?;

    let val_opt: Option<serde_json::Value> = eval_result.into_value()
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to parse evaluate result: {}", e)))?;

    let total_height = match val_opt {
        Some(v) => v.as_u64().unwrap_or(2000),
        None => 2000,
    } as i64;

    let step = 300;
    let mut current = 0;

    while current < total_height {
        let scroll_js = format!("window.scrollTo(0, {});", current);
        page.evaluate(scroll_js)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to scroll page: {}", e)))?;
        sleep(Duration::from_millis(500)).await;
        current += step;
    }

    // Final scroll to bottom just to be sure
    page.evaluate(format!("window.scrollTo(0, {});", total_height))
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to scroll to bottom: {}", e)))?;
    sleep(Duration::from_millis(700)).await;
    
    // ----

    // Get the full page content HTML after scrolling
    let content = page.content().await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to get page content: {}", e)))?;

    // Parse HTML and extract all <img> src attributes
    let document = Html::parse_document(&content);
    let selector = Selector::parse("img").unwrap();

    let mut images = Vec::new();

    for img in document.select(&selector) {
        // Iterate over all attributes of the <img> tag
        let mut found_url = None;

        for (attr_name, attr_value) in img.value().attrs() {
            if attr_name.to_lowercase().contains("url") && !attr_value.trim().is_empty() {
                found_url = Some(attr_value.to_string());
                break; // break after first attribute containing "url"
            }
        }

        if let Some(url) = found_url {
            images.push(url);
        } else if let Some(src) = img.value().attr("src") {
            // fallback if no url-containing attribute found, and src is present
            if !src.trim().is_empty() {
                images.push(src.to_string());
            }
        }
    }

    Ok(HttpResponse::Ok().json(ImagesFromSourceResponse { images }))
}


// *************************************************************************************
// Get all URL from the network request
// *************************************************************************************

#[derive(Serialize)]
struct ImagesFromRequestResponse {
    images: Vec<String>,
}

#[get("/getimagesfromrequests")]
async fn get_images_from_requests(query: web::Query<HashMap<String, String>>) -> actix_web::Result<impl Responder> {
    let page_url = query.get("url")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'url' query parameter"))?
        .to_owned();

    // Launch Chromium browser
    // This bypasses the sandbox entirely.
    // Safe inside Docker, as long as you don’t let untrusted code run inside the container.
    let args: Vec<String> = vec![
        "--no-sandbox".into(),
        "--disable-setuid-sandbox".into(),
        "--disable-dev-shm-usage".into(), // optional, fixes /dev/shm mount issues
    ];

    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder().chrome_executable("/usr/bin/chromium")
        .args(args)
        .build().unwrap())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Browser launch failed: {}", e)))?;

    // Spawn task to drive browser event loop
    actix_rt::spawn(async move {
        while handler.next().await.is_some() {}
    });

    // Open new page but DO NOT navigate yet
    let page = browser.new_page("about:blank")
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to open blank page: {}", e)))?;

    // Subscribe to network request events BEFORE navigation
    let mut events = page.event_listener::<EventRequestWillBeSent>()
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to subscribe to network events: {}", e)))?;

    // Navigate to the target URL now
    page.goto(&page_url)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to navigate page: {}", e)))?;

    // Wait for full page load event
    page.wait_for_navigation()
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to wait for page load: {}", e)))?;

    // Scroll down slowly to trigger lazy loading images
    let scroll_height_js = "document.body.scrollHeight";

    let eval_result = page.evaluate(scroll_height_js).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to get scrollHeight: {}", e)))?;

    let val_opt: Option<serde_json::Value> = eval_result.into_value()
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to parse evaluate result: {}", e)))?;

    let total_height = match val_opt {
        Some(v) => v.as_u64().unwrap_or(2000),
        None => 2000,
    } as i64;

    let step = 300;
    let mut current = 0;

    while current < total_height {
        let scroll_js = format!("window.scrollTo(0, {});", current);
        page.evaluate(scroll_js)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to scroll page: {}", e)))?;
        sleep(Duration::from_millis(500)).await;
        current += step;
    }

    // Final scroll to bottom just to be sure
    page.evaluate(format!("window.scrollTo(0, {});", total_height))
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to scroll to bottom: {}", e)))?;
    sleep(Duration::from_millis(700)).await;

    // Collect image URLs from network requests captured during navigation and scrolling
    let mut images = Vec::new();
    let timeout = std::time::Instant::now() + Duration::from_secs(1); // short timeout to drain remaining events

    while let Some(event_arc) = events.next().await {
        let event = &*event_arc;
        let url = &event.request.url;

        if is_image_url(url) {
            images.push(url.clone());
        }

        if std::time::Instant::now() > timeout {
            break;
        }
    }

    Ok(HttpResponse::Ok().json(ImagesFromRequestResponse { images }))
}




// *************************************************************************************
// Get resized image - upload
// *************************************************************************************
// Reference: https://github.com/nlfiedler/magick-rust/blob/master/tests/lib.rs

#[post("/resize")]
async fn resize_image(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // Create a temporary file
    let mut temp_file = NamedTempFile::new().unwrap();

    while let Some(item) = payload.next().await {
        let mut field = item?;
        println!("Uploading file field: {:?}", field.name());

        while let Some(chunk) = field.next().await {
            let data = chunk?;
            temp_file.write_all(&data)?;
        }

        println!("File saved to temp: {:?}", temp_file.path());
    }

    // Initialize ImageMagick
    magick_wand_genesis();

    let result = (|| {
        let mut wand = MagickWand::new();
        wand.read_image(temp_file.path().to_str().unwrap())?;

        // Resize (300x300, Lanczos = 22)
        wand.resize_image(300, 300, magick_rust::FilterType::Lanczos)?;

        wand.set_image_format("JPEG")?;
        let blob = wand.write_image_blob("JPEG")?;
        Ok::<_, magick_rust::MagickError>(blob)
    })();

    // Explicit cleanup (drop temp file)
    temp_file.close().unwrap_or_else(|e| {
        eprintln!("Failed to delete temp file: {:?}", e);
    });

    match result {
        Ok(blob) => {
            println!("Image resized successfully!");
            Ok(HttpResponse::Ok()
                .content_type("image/jpeg")
                .body(blob))
        }
        Err(e) => {
            eprintln!("Error while processing image: {:?}", e);
            Ok(HttpResponse::InternalServerError().body("Image processing failed"))
        }
    }
}




/*
#[post("/resize")]
async fn resize_image(mut payload: Multipart) -> Result<HttpResponse, Error> {

    // Collect uploaded file
    let mut temp_file = NamedTempFile::new().unwrap();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        
        // Log the uploaded file field name
        println!("Uploading file field: {:?}", field.name());
        
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            temp_file.write_all(&data)?;
        }

        // Log after file is written
        println!("File saved to temp: {:?}", temp_file.path());
    }


    // Initialize ImageMagick
    magick_wand_genesis();

    /*
        START.call_once(|| {
            magick_wand_genesis();
        });
    */

    // Load image into ImageMagick
    let mut wand = MagickWand::new();
    wand.read_image(temp_file.path().to_str().unwrap())
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Image read error: {:?}", e))
        })?;

    // Example: resize to 300x300
    wand.resize_image(300, 300, magick_rust::FilterType::Lanczos)
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Resize error: {:?}", e))
        })?;

    // Export as JPEG
    wand.set_image_format("JPEG").unwrap();
    let blob = wand.write_image_blob("JPEG").unwrap();

    Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(blob))
    

    // Ok(HttpResponse::Ok().json({"data": "response"}))
}
*/
    










/// ***********************************************************
/// PRIVATE FUNCTIONS

async fn build_image_response(image_id: &str) -> Result<ImageInfoResponse, Box<dyn std::error::Error>> {
    // Here you'd fetch real metadata from DB/storage, which might fail
    Ok(ImageInfoResponse {
        url: format!("https://cdn.example.com/images/{}.jpg", image_id),
        filename: format!("{}.jpg", image_id),
        size: 512_000,
        mime_type: "image/jpeg".into(),
        width: 1024,
        height: 768,
    })
}

/// Metadata struct for HTTP response, holding EXIF JSON data
#[derive(Serialize, Debug)]
struct FileMetadata {
    filename: String,
    exif: Value,
}

/// Map MIME types to file extensions
fn extension_from_content_type(content_type: &str) -> &str {
    let mut map = HashMap::new();

    // Images
    map.insert("image/jpeg", "jpg");
    map.insert("image/pjpeg", "jpg"); // progressive JPEG
    map.insert("image/png", "png");
    map.insert("image/gif", "gif");
    map.insert("image/bmp", "bmp");
    map.insert("image/webp", "webp");
    map.insert("image/tiff", "tiff");
    map.insert("image/x-tiff", "tiff");
    map.insert("image/x-icon", "ico");
    map.insert("image/vnd.microsoft.icon", "ico");
    map.insert("image/heif", "heif");
    map.insert("image/heic", "heic");
    map.insert("image/avif", "avif");
    map.insert("image/svg+xml", "svg");
    map.insert("image/x-canon-cr2", "cr2");
    map.insert("image/x-nikon-nef", "nef");

    // Non-image (optional extras you already had)
    map.insert("application/pdf", "pdf");
    map.insert("text/plain", "txt");
    map.insert("application/zip", "zip");

    map.get(content_type).copied().unwrap_or("bin")
}

/// Extract filename from Content-Disposition or URL
fn extract_filename(url: &str, content_disposition: Option<&str>, ext: &str) -> String {
    if let Some(cd) = content_disposition {
        if let Some(start) = cd.find("filename=") {
            let fname = &cd[start + 9..];
            let fname_clean = fname.trim_matches(&['"', '\''][..]).to_string();
            if !fname_clean.is_empty() {
                return fname_clean;
            }
        }
    }

    if let Ok(parsed) = Url::parse(url) {
        if let Some(seg) = parsed.path_segments().and_then(|s| s.last()) {
            if !seg.is_empty() {
                return seg.to_string();
            }
        }
    }

    format!("downloaded_file.{}", ext)
}

/// Get EXIF metadata from file using exiftool crate
async fn get_exif_data_rust(path: &std::path::Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    info!("get_exif_data_rust: Getting the exif data...");
    let mut exiftool = ExifTool::new()?;
    let json_value = exiftool.json(path, &[])?;  // no await
    Ok(json_value)
}




/// Download file from URL, save, get EXIF metadata, then delete file if done and if error
async fn process_file_from_url(
    url: &str,
) -> Result<FileMetadata, Box<dyn std::error::Error>> {
    dotenv().ok();      // Transfer this in the centralized object
    let config = config::AppConfig::from_env();

    let client = Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    info!("Response status: {}", response.status());
    for (key, value) in response.headers() {
        info!("Header: {}: {:?}", key, value);
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let ext = extension_from_content_type(content_type);

    let content_disposition = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok());
    let filename = extract_filename(url, content_disposition, ext);

    info!("Create working folder...");
    //let custom_dir = PathBuf::from("/app/workingdir/downloads");
    let custom_dir = PathBuf::from(config.working_dir);
    fs::create_dir_all(&custom_dir).await?;

    let mut local_path = custom_dir.clone();
    local_path.push(&filename);

    info!("Downloading file into working folder...");
    let mut stream = response.bytes_stream();
    let mut file = File::create(&local_path).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    // Try to get EXIF metadata
    match get_exif_data_rust(&local_path).await {
        Ok(exif) => {
            if let Err(e) = fs::remove_file(&local_path).await {
                error!("Failed to delete temp file after EXIF error: {}", e);
            }
            // Success — return metadata, keep file if you want
            Ok(FileMetadata { filename, exif })
        }
        Err(err) => {
            // Error — delete file before returning error
            if let Err(e) = fs::remove_file(&local_path).await {
                error!("Failed to delete temp file after EXIF error: {}", e);
            }
            Err(err)
        }
    }
}



fn is_image_url(url: &str) -> bool {
    let url = url.to_lowercase();
    url.ends_with(".png")
        || url.ends_with(".jpg")
        || url.ends_with(".jpeg")
        || url.ends_with(".gif")
        || url.ends_with(".webp")
        || url.ends_with(".bmp")
        || url.ends_with(".svg")
        || url.ends_with(".tiff")
        || url.ends_with(".tif")
        || url.ends_with(".ico")
        || url.ends_with(".heic")
        || url.ends_with(".avif")
        || url.ends_with(".jfif")
        || url.ends_with(".apng")
}


