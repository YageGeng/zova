#![cfg_attr(not(test), no_std)]
#![recursion_limit = "135"]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use burn::backend::NdArray;
use burn::prelude::*;
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

/// Layout region from analyzer
#[derive(Serialize, Deserialize)]
pub struct LayoutRegion {
    pub id: usize,
    pub bbox: BoundingBox,
    pub class: String,
    pub confidence: f32,
}

#[derive(Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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
pub struct LayoutAnalyzer {
    device: NdArrayDevice,
}

#[wasm_bindgen]
impl LayoutAnalyzer {
    /// Create new analyzer
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        log::info!("Initializing LayoutAnalyzer");
        let device: NdArrayDevice = Default::default();
        Self { device }
    }

    /// Analyze image and return layout regions
    /// Input: RGB float32 array in CHW format [0-1]
    pub fn analyze(
        &self,
        image_data: Vec<f32>,
        width: usize,
        height: usize,
    ) -> Result<JsValue, JsValue> {
        log::info!("Analyzing image {}x{}", width, height);

        // Create tensor from input data
        let tensor: Tensor<NdArray, 1> = Tensor::from_floats(image_data.as_slice(), &self.device);
        let input = tensor.reshape([1, 3, height, width]);

        // For now, return stub result (real model integration needs async)
        // TODO: Integrate with layout crate model
        let result = self.stub_analyze(width, height);

        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    fn stub_analyze(&self,
        width: usize,
        height: usize,
    ) -> PdfResult {
        // Return stub result for now
        PdfResult {
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
        }
    }
}