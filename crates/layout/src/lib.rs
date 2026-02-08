pub mod types;

use burn::prelude::*;
use types::*;

// Stub 模型代码（自动生成）
include!(concat!(env!("OUT_DIR"), "/model.rs"));

/// 版面分析器
pub struct LayoutAnalyzer<B: Backend> {
    model: Model<B>,
    device: B::Device,
}

impl<B: Backend> LayoutAnalyzer<B> {
    /// 创建新的分析器实例
    pub fn new(device: B::Device) -> Self {
        let model = Model::new(&device);
        Self { model, device }
    }
    
    /// 分析图片，返回版面区域列表
    pub fn analyze(&self, 
        _image: &image::DynamicImage
    ) -> Vec<LayoutRegion> {
        // TODO: 实现完整的预处理和后处理
        // 目前返回空列表
        Vec::new()
    }
}