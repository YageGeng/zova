pub mod types;

use burn::prelude::*;
use types::*;

// 包含 burn-import 生成的模型代码
include!(concat!(env!("OUT_DIR"), "/doclayout_model/doclayout_yolo_static.rs"));

pub struct LayoutAnalyzer<B: Backend> {
    model: doclayout_yolo_static::Model<B>,
    input_size: usize,
}

impl<B: Backend> LayoutAnalyzer<B> {
    pub fn new(device: &B::Device) -> Self {
        let model = doclayout_yolo_static::Model::new(device);
        Self {
            model,
            input_size: 1024, // DocLayout 使用 1024x1024
        }
    }
    
    /// 分析图片，返回版面区域列表
    pub fn analyze(&self, 
        image: &image::DynamicImage
    ) -> Vec<LayoutRegion> {
        // 1. 预处理
        let input = self.preprocess(image);
        
        // 2. 模型推理
        let output = self.model.forward(input);
        
        // 3. 后处理：解码 YOLO 输出
        self.decode_output(output)
    }
    
    fn preprocess(
        &self, 
        image: &image::DynamicImage
    ) -> Tensor<B, 4> {
        // TODO: letterbox resize + normalize
        todo!()
    }
    
    fn decode_output(
        &self, 
        output: Tensor<B, 3>
    ) -> Vec<LayoutRegion> {
        // TODO: YOLO 输出解码 + NMS
        todo!()
    }
}