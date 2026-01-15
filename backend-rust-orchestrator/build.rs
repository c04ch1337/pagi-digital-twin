fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build memory service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &["../backend-rust-memory/proto/memory.proto"],
            &["../backend-rust-memory/proto/"],
        )?;

    // Build tools service proto (client only)
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &["../backend-rust-tools/proto/tools.proto"],
            &["../backend-rust-tools/proto/"],
        )?;

    // Build orchestrator protos (server + client)
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/orchestrator.proto",
                "proto/orchestrator_admin.proto",
                "proto/handshake.proto",
                "proto/memory_exchange.proto",
            ],
            &["proto/"],
        )?;

    Ok(())
}
