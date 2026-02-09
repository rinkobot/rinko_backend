fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["../proto/bot.proto"], &["../proto"])?;
    
    println!("cargo:rerun-if-changed=../proto/bot.proto");
    Ok(())
}
