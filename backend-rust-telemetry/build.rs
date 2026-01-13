fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build orchestrator service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(
            &["../backend-rust-orchestrator/proto/orchestrator.proto"],
            &["../backend-rust-orchestrator/proto/"],
        )?;

    // Build memory service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(
            &["../backend-rust-memory/proto/memory.proto"],
            &["../backend-rust-memory/proto/"],
        )?;

    Ok(())
}
