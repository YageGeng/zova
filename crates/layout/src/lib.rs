pub mod types;

use burn::prelude::*;
use types::*;

// 包含 burn-import 生成的模型代码
include!(concat!(env!("OUT_DIR"), "/model/doclayout_inferred.rs"));

/// 版面分析器
pub struct LayoutAnalyzer<B: Backend> {
    model: doclayout_inferred::Model<B>,
    device: B::Device,
}

impl<B: Backend> LayoutAnalyzer<B> {
    /// 创建新的分析器实例
    pub fn new(device: B::Device) -> Self {
        let model = doclayout_inferred::Model::new(&device);
        Self { model, device }
    }
    
    /// 分析图片，返回版面区域列表
    pub fn analyze(&self, 
        image: &image::DynamicImage
    ) -> Vec<LayoutRegion> {
        // 1. 预处理: resize 到 1024x1024
        let input = self.preprocess(image);
        
        // 2. 模型推理
        let output = self.model.forward(input);
        
        // 3. 后处理: 解码 YOLO 输出
        self.decode_output(output)
    }
    
    /// 预处理图片
    fn preprocess(
        &self, 
        image: &image::DynamicImage
    ) -> Tensor<B, 4> {
        // Resize 到 1024x1024 (保持宽高比的 letterbox)
        let resized = image.resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3);
        
        // 转换为 RGB float tensor [1, 3, 1024, 1024]
        let rgb = resized.to_rgb8();
        let data: Vec<f32> = rgb.pixels()
            .flat_map(|p| vec![p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
            .collect();
        
        // 重新排列为 CHW 格式
        let mut chw_data = vec![0.0f32; 3 * 1024 * 1024];
        for i in 0..(1024 * 1024) {
            chw_data[i] = data[i * 3];           // R
            chw_data[i + 1024 * 1024] = data[i * 3 + 1];   // G
            chw_data[i + 2 * 1024 * 1024] = data[i * 3 + 2]; // B
        }
        
        Tensor::from_data([1, 3, 1024, 1024], chw_data, &self.device)
    }
    
    /// 解码 YOLO 输出
    fn decode_output(
        &self, 
        output: Tensor<B, 3>
    ) -> Vec<LayoutRegion> {
        // TODO: 实现 YOLO 输出解码
        // output shape: [batch, num_predictions, 6]
        // 6 = [x_center, y_center, width, height, confidence, class_id]
        
        let data = output.to_data();
        let shape = data.shape.clone();
        
        println!("Output shape: {:?}", shape);
        
        // 临时返回空列表
        Vec::new()
    }
}