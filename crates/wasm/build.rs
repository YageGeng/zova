use burn_onnx::ModelGen;

fn main() {
    println!("cargo:rerun-if-changed=src/model");
    
    // Generate model code with embedded weights for WASM
    ModelGen::new()
        .input("src/model/doclayout.onnx")
        .out_dir("model/")
        .embed_states(true)  // Embed weights in binary
        .run_from_script();
}