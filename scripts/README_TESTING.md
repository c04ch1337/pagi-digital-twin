# AGI System Testing Scripts

This directory contains diagnostic and integration test scripts for the Ferrellgas AGI Multi Digital Twin Platform.

## Diagnostic Scripts

### `check_agi_health.sh` (Linux/macOS)

A bash script that verifies the networking and gRPC reachability between Telemetry, Orchestrator, and Memory services.

**Usage:**
```bash
# With default ports
./scripts/check_agi_health.sh

# With custom ports (via environment variables)
export ORCHESTRATOR_GRPC_ADDR="127.0.0.1:50057"
export MEMORY_GRPC_ADDR="127.0.0.1:50052"
./scripts/check_agi_health.sh
```

**Requirements:**
- `grpcurl` - Install with: `brew install grpcurl` (macOS) or download from [GitHub](https://github.com/fullstorydev/grpcurl)
- `curl` - Usually pre-installed on Linux/macOS

### `check_agi_health.ps1` (Windows)

A PowerShell script with the same functionality for Windows systems.

**Usage:**
```powershell
# With default ports
.\scripts\check_agi_health.ps1

# With custom ports
.\scripts\check_agi_health.ps1 -OrchestratorGrpcAddr "127.0.0.1:50057" -MemoryGrpcAddr "127.0.0.1:50052"
```

**Requirements:**
- `grpcurl` - Download from [GitHub releases](https://github.com/fullstorydev/grpcurl/releases) or install via `scoop install grpcurl`
- PowerShell 5.1 or later

## Integration Tests

### Rust Integration Tests

Located in `backend-rust-telemetry/tests/integration_tests.rs`, these tests validate the full Multi-Modal Insight Loop workflow.

**Running the tests:**
```bash
cd backend-rust-telemetry

# Run all tests (including ignored ones that require services)
cargo test --test integration_tests -- --ignored

# Run a specific test
cargo test --test integration_tests test_multi_modal_insight_loop -- --ignored
```

**Prerequisites:**
- Orchestrator service running with `LLM_PROVIDER=openrouter` and `OPENROUTER_API_KEY` set
- Memory service running
- Telemetry transcription worker running (or the test will simulate processing)

**What the tests verify:**
1. ✅ Mock transcript file creation
2. ✅ File watcher detection (or manual processing)
3. ✅ Orchestrator summarization
4. ✅ Summary JSON file creation with proper structure
5. ✅ Validation of summary content (non-empty fields)

## Test Workflow

The integration test follows this workflow:

```
1. Create test transcript file
   ↓
2. Wait for file watcher to detect (or trigger manually)
   ↓
3. Orchestrator processes transcript → generates summary
   ↓
4. Summary saved as [filename].summary.json
   ↓
5. Validate JSON structure and content
   ↓
6. (Optional) Query Memory Service to verify indexing
   ↓
7. Cleanup test files
```

## Troubleshooting

### Diagnostic Script Issues

**"grpcurl: command not found"**
- Install grpcurl using the methods mentioned above
- Ensure it's in your PATH

**"Service OFFLINE"**
- Check that services are running: `ps aux | grep -E "(orchestrator|telemetry|memory)"`
- Verify ports are not blocked by firewall
- Check service logs for errors

**"gRPC connection failed"**
- Verify the service is listening on the correct port
- Check environment variables (ORCHESTRATOR_GRPC_PORT, MEMORY_GRPC_PORT)
- Ensure services are started in the correct order (Memory → Orchestrator → Telemetry)

### Integration Test Issues

**"Summary file was not created within timeout"**
- Ensure Orchestrator service is running
- Check that `LLM_PROVIDER=openrouter` and `OPENROUTER_API_KEY` is set
- Verify the transcription worker is running or manually trigger processing
- Check Orchestrator logs for errors

**"Summary text is empty"**
- The LLM may have returned an empty response
- Check Orchestrator logs for LLM errors
- Verify OpenRouter API key is valid

**"Memory service check requires running service"**
- The test currently skips Memory verification
- To enable, ensure Memory service is running and update the test to query it

## Continuous Monitoring

For production deployments, consider:

1. **Watchdog Service**: Run `check_agi_health.sh` every 5 minutes via cron/systemd timer
2. **Alerting**: Send notifications (email, Slack, etc.) when tests fail
3. **Metrics**: Export test results to Prometheus/Grafana for visualization
4. **Automated Rollback**: Script to clear pending transcription tasks if Orchestrator goes down

## Environment Variables Reference

```bash
# Orchestrator
ORCHESTRATOR_HTTP_PORT=8182
ORCHESTRATOR_GRPC_PORT=50057
ORCHESTRATOR_GRPC_ADDR=http://127.0.0.1:50057

# Telemetry
TELEMETRY_PORT=8183
TELEMETRY_STORAGE_DIR=./storage

# Memory
MEMORY_GRPC_PORT=50052
MEMORY_GRPC_ADDR=http://127.0.0.1:50052

# Gateway
GATEWAY_PORT=8181

# LLM Configuration (required for summarization)
LLM_PROVIDER=openrouter
OPENROUTER_API_KEY=your_key_here
OPENROUTER_MODEL=google/gemini-2.0-flash-exp
```

## Next Steps

- [ ] Set up automated health checks via cron/systemd
- [ ] Integrate with Prometheus/Grafana for metrics
- [ ] Create rollback scripts for service failures
- [ ] Add Memory service query verification to integration tests
- [ ] Set up alerting for test failures
