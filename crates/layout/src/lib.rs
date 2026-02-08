pub mod types;

use burn::prelude::*;
use burn::module::Module;
use burn::tensor::Tensor;
use types::*;

/// 版面分析器
/// 
/// 注意：当前为 stub 实现
/// 完整的 YOLO 推理需要手动实现 burn 模型结构
/// 或等待 burn-import 支持动态形状
pub struct LayoutAnalyzer<B: Backend> {
    device: B::Device,
}

impl<B: Backend> LayoutAnalyzer<B> {
    /// 创建新的分析器实例
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
    
    /// 分析图片，返回版面区域列表
    /// 
    /// 当前为 stub，返回空列表
    pub fn analyze(
        &self,
        _image: &image::DynamicImage
    ) -> Vec<LayoutRegion> {
        // TODO: 实现完整的 YOLO 推理
        // 1. 预处理图片到 1024x1024
        // 2. 运行模型推理
        // 3. 解码 YOLO 输出（NMS）
        
        Vec::new()
    }
}

/// 预处理图片
fn preprocess_image(
    image: &image::DynamicImage,
    target_size: u32
) -> Vec<f32> {
    // Resize 到目标尺寸
    let resized = image.resize_exact(
        target_size, 
        target_size, 
        image::imageops::FilterType::Lanczos3
    );
    
    // 转换为 RGB
    let rgb = resized.to_rgb8();
    
    // 归一化并转换为 CHW 格式
    let mut data = vec![0.0f32; 3 * target_size as usize * target_size as usize];
    for (i, pixel) in rgb.pixels().enumerate() {
        let x = i % target_size as usize;
        let y = i / target_size as usize;
        let offset = y * target_size as usize + x;
        
        data[offset] = pixel[0] as f32 / 255.0;
        data[offset + target_size as usize * target_size as usize] = pixel[1] as f32 / 255.0;
        data[offset + 2 * target_size as usize * target_size as usize] = pixel[2] as f32 / 255.0;
    }
    
    data
}