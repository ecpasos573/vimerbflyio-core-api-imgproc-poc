use serde::Serialize;

#[derive(Serialize)]
pub struct ImageInfoResponse {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}