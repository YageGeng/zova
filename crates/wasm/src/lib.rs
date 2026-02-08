use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

/// PDF Processing Result
#[derive(Serialize, Deserialize)]
pub struct PdfResult {
    pub pages: Vec<PageResult>,
}

#[derive(Serialize, Deserialize)]
pub struct PageResult {
    pub page_num: usize,
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<BlockResult>,
}

#[derive(Serialize, Deserialize)]
pub struct BlockResult {
    pub id: String,
    pub bbox: [f32; 4],
    pub class: String,
    pub text: Option<String>,
}

/// Process PDF bytes and return layout analysis (stub)
#[wasm_bindgen]
pub fn process_pdf(pdf_bytes: &[u8]) -> Result<String, String> {
    console_error_panic_hook::set_once();
    
    log::info!("Processing PDF with {} bytes", pdf_bytes.len());
    
    // Stub implementation - returns dummy result
    let result = PdfResult {
        pages: vec![PageResult {
            page_num: 0,
            width: 595.0,
            height: 842.0,
            blocks: vec![
                BlockResult {
                    id: "p0-b0".to_string(),
                    bbox: [50.0, 50.0, 545.0, 100.0],
                    class: "Title".to_string(),
                    text: Some("Sample Title".to_string()),
                },
                BlockResult {
                    id: "p0-b1".to_string(),
                    bbox: [50.0, 120.0, 545.0, 300.0],
                    class: "Text".to_string(),
                    text: Some("Sample text content from WASM!".to_string()),
                },
            ],
        }],
    };
    
    serde_json::to_string(&result)
        .map_err(|e| e.to_string())
}

/// Initialize WASM module
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    log::info!("Zova WASM module initialized");
}