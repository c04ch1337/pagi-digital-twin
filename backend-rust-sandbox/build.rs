fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Rebuild if the proto changes.
	println!("cargo:rerun-if-changed=../backend-go-model-gateway/proto/model.proto");

	tonic_build::configure()
		.build_server(true)
		.build_client(false)
		.compile_protos(
			&["../backend-go-model-gateway/proto/model.proto"],
			&["../backend-go-model-gateway/proto"],
		)?;

	Ok(())
}

