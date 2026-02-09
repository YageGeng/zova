#![cfg_attr(not(test), no_std)]
#![recursion_limit = "135"]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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

/// Initialize WASM module
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    log::info!("Zova WASM module initialized");
}

/// Layout analyzer
#[wasm_bindgen]
pub struct LayoutAnalyzer;

#[wasm_bindgen]
impl LayoutAnalyzer {
    /// Create new analyzer
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        log::info!("Initializing LayoutAnalyzer");
        Self
    }

    /// Analyze image and return layout regions (stub)
    pub fn analyze(
        &self,
        _image_data: Vec<f32>,
        width: usize,
        height: usize,
    ) -> Result<JsValue, JsValue> {
        log::info!("Analyzing image {}x{}", width, height);

        // Return stub result
        let result = PdfResult {
            pages: vec![PageResult {
                page_num: 0,
                width: width as f32,
                height: height as f32,
                blocks: vec![
                    BlockResult {
                        id: "p0-b0".to_string(),
                        bbox: [50.0, 50.0, 200.0, 100.0],
                        class: "Title".to_string(),
                        text: Some("Title Block".to_string()),
                    },
                    BlockResult {
                        id: "p0-b1".to_string(),
                        bbox: [50.0, 150.0, 500.0, 300.0],
                        class: "Text".to_string(),
                        text: Some("Text Block".to_string()),
                    },
                ],
            }],
        };

        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
}