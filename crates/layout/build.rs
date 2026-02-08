use burn_import::onnx::ModelGen;
use std::env;
use std::path::PathBuf;

fn main() {
    // 模型路径
    let model_path = "../../models/doclayout_yolo_static.onnx";
    
    // 检查模型是否存在
    if !PathBuf::from(model_path).exists() {
        panic!("Model not found: {}", model_path);
    }
    
    println!("cargo:rerun-if-changed={}", model_path);
    
    // 使用 burn-import 生成模型代码
    ModelGen::new()
        .input(model_path)
        .out_dir("doclayout_model")
        .run_from_script();
    
    println!("cargo:warning=Model code generated successfully");
}