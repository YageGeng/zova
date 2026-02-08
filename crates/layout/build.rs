// Stub build.rs - burn-import cannot handle this model's dynamic shapes
// Using stub implementation until burn-import supports Shape/Gather/Range ops

fn main() {
    println!("cargo:warning=Using stub model - burn-import cannot handle dynamic shapes");
    
    // 创建 stub 模型代码
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let stub_code = r#"
use burn::prelude::*;

/// Stub DocLayout model
#[derive(Module, Debug)]
pub struct Model<B: Backend> {
    phantom: std::marker::PhantomData<B>,
}

impl<B: Backend> Model<B> {
    /// Create new model instance
    pub fn new(_device: &B::Device) -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
    
    /// Forward pass - returns dummy output
    pub fn forward(&self, input: Tensor<B, 4>) -> Tensor<B, 3> {
        // TODO: Replace with actual YOLO implementation
        // For now, return dummy output [1, 300, 6]
        let device = input.device();
        Tensor::zeros([1, 300, 6], &device)
    }
}
"#;
    
    std::fs::write(format!("{}/model.rs", out_dir), stub_code)
        .expect("Failed to write stub model");
}