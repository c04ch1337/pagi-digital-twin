use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

#[derive(Clone, Copy)]
struct Service {
    display_name: &'static str,
    manifest_path: &'static str,
}

fn main() -> ExitCode {
    // Order is intentional: build low-level services first.
    let services: [Service; 5] = [
        Service {
            display_name: "backend-rust-tools",
            manifest_path: "backend-rust-tools/Cargo.toml",
        },
        Service {
            display_name: "backend-rust-memory",
            manifest_path: "backend-rust-memory/Cargo.toml",
        },
        Service {
            display_name: "backend-rust-orchestrator",
            manifest_path: "backend-rust-orchestrator/Cargo.toml",
        },
        Service {
            display_name: "backend-rust-gateway",
            manifest_path: "backend-rust-gateway/Cargo.toml",
        },
        Service {
            display_name: "backend-rust-telemetry",
            manifest_path: "backend-rust-telemetry/Cargo.toml",
        },
    ];

    println!("==============================");
    println!("Backend Build Verification (P30)");
    println!("==============================\n");
    println!("Building {} Rust services...\n", services.len());

    let overall_start = Instant::now();

    let mut results: Vec<(Service, bool, std::time::Duration)> = Vec::with_capacity(services.len());

    for service in services {
        println!("----------------------------------------");
        println!("Building: {}", service.display_name);
        println!("Manifest: {}\n", service.manifest_path);

        let start = Instant::now();
        let status = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(service.manifest_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
        let elapsed = start.elapsed();

        let ok = match status {
            Ok(s) => s.success(),
            Err(_) => false,
        };

        if ok {
            println!("\nStatus: SUCCESS ({:.2?})", elapsed);
        } else {
            println!("\nStatus: FAILURE ({:.2?})", elapsed);
        }

        results.push((service, ok, elapsed));
        println!();
    }

    println!("========================================");
    println!("Build Summary");
    println!("========================================");

    let mut any_failed = false;
    for (service, ok, elapsed) in &results {
        let status = if *ok { "SUCCESS" } else { "FAILURE" };
        println!(
            "- {:<26} {:<7}  ({:.2?})",
            service.display_name, status, elapsed
        );
        if !ok {
            any_failed = true;
        }
    }

    println!("\nTotal elapsed: {:.2?}", overall_start.elapsed());

    if any_failed {
        println!("\nBuild Failed in One or More Services");
        ExitCode::from(1)
    } else {
        println!("\nAll Services Built Successfully");
        ExitCode::SUCCESS
    }
}

