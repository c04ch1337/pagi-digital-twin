fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&["proto/tools.proto"], &["proto/"])?;
    Ok(())
}
