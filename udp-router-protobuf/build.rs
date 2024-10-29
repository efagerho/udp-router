extern crate tonic_build;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/codegen")
        .compile_protos(&["proto/management.proto"], &["proto"])?;
    Ok(())
}
