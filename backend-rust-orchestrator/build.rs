fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build memory service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(
            &["../backend-rust-memory/proto/memory.proto"],
            &["../backend-rust-memory/proto/"],
        )?;

    // Build tools service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(
            &["../backend-rust-tools/proto/tools.proto"],
            &["../backend-rust-tools/proto/"],
        )?;

    // Build orchestrator admin proto (server + client)
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/orchestrator_admin.proto"], &["proto/"])?;

    Ok(())
}
