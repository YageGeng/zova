#![cfg_attr(not(test), no_std)]
#![recursion_limit = "135"]

pub mod model;

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use burn::backend::NdArray;
use burn::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::model::doclayout::Model as DocLayoutModel;

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

/// Layout analyzer using DocLayout-YOLO model
#[wasm_bindgen]
pub struct LayoutAnalyzer {
    model: ModelType,
}

#[allow(clippy::large_enum_variant)]
enum ModelType {
    WithNdArrayBackend(Model<NdArray<f32>>),
}

#[wasm_bindgen]
impl LayoutAnalyzer {
    /// Create new analyzer with NdArray backend
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        log::info!("Initializing LayoutAnalyzer");
        let device = Default::default();
        Self {
            model: ModelType::WithNdArrayBackend(Model::new(&device)),
        }
    }

    /// Analyze image and return layout regions
    pub async fn analyze(
        &self,
        image_data: Vec<f32>,
        width: usize,
        height: usize,
    ) -> Result<JsValue, JsValue> {
        log::info!("Analyzing image {}x{}", width, height);

        let result = match &self.model {
            ModelType::WithNdArrayBackend(model) => {
                model.forward(&image_data, width, height).await
            }
        };

        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
}

/// Layout analysis model wrapper
pub struct Model<B: Backend> {
    model: DocLayoutModel<B>,
}

impl<B: Backend> Model<B> {
    /// Create model from embedded weights
    pub fn new(device: &B::Device) -> Self {
        Self {
            model: DocLayoutModel::from_embedded(device),
        }
    }

    /// Run inference on image
    pub async fn forward(
        &self,
        image_data: &[f32],
        width: usize,
        height: usize,
    ) -> PdfResult {
        // Convert to tensor [1, 3, H, W]
        let input = Tensor::<B, 1>::from_floats(image_data, &B::Device::default())
            .reshape([1, 3, height, width]);

        // Run model
        let output = self.model.forward(input);

        // Decode YOLO output (stub)
        let _ = output;
        
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