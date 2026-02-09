extern crate alloc;

pub mod model;
pub mod types;

use alloc::vec::Vec;
use burn::prelude::*;
use types::*;

pub use model::doclayout::*;
pub use types::*;

/// 版面分析器
pub struct LayoutAnalyzer<B: Backend> {
    _phantom: core::marker::PhantomData<B>,
}

impl<B: Backend> LayoutAnalyzer<B> {
    /// 创建新的分析器
    pub fn new(_device: B::Device) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
    
    /// 分析图片，返回版面区域列表
    /// 
    /// 当前为 stub 实现，返回空列表
    pub fn analyze(&self, _image: &image::DynamicImage) -> Vec<LayoutRegion> {
        // TODO: 实现实际的 YOLO 推理
        // 1. 预处理图片到 1024x1024
        // 2. 运行模型推理
        // 3. 解码 YOLO 输出（NMS）
        
        Vec::new()
    }
}