# End-to-End Functional Test Script

## Ferrellgas AGI Multi Digital Twin Platform - Bare-Metal Testing Guide

This document provides step-by-step instructions for performing the first functional test of the fully assembled platform using bare-metal deployment.

---

## Prerequisites

1. **Rust Toolchain**: Ensure Rust 1.78+ is installed (`rustc --version`)
2. **Node.js**: Ensure Node.js 20+ is installed (`node --version`)
3. **Port Availability**: Verify ports `3000`, `8181`, `8182`, `50052`, `50054` are available
4. **Environment**: All services should be built and ready to run

---

## Phase 1: Service Startup (Bare-Metal)

Start all services in the correct dependency order. Use separate terminal windows/tabs for each service.

### Terminal 1: Memory Service

```bash
cd backend-rust-memory
export MEMORY_GRPC_PORT=50052
export LOG_LEVEL=info
cargo run
```

**Expected Output:**
```
INFO backend_rust_memory: Starting Memory gRPC server on 0.0.0.0:50052
INFO backend_rust_memory: Memory service initialized with mock data
```

**Verification:**
- Service should be listening on port `50052`
- No compilation errors
- Mock data initialized message appears

---

### Terminal 2: Tools Service

```bash
cd backend-rust-tools
export TOOLS_GRPC_PORT=50054
export SANDBOX_DIR=/tmp/pagi-sandbox
export SAFE_MODE=true
export LOG_LEVEL=info
cargo run
```

**Expected Output:**
```
INFO backend_rust_tools: Starting Tools gRPC server on 0.0.0.0:50054
INFO backend_rust_tools: Tools service initialized with policy enforcement
```

**Verification:**
- Service should be listening on port `50054`
- No compilation errors
- Policy configuration loaded

---

### Terminal 3: Orchestrator

```bash
cd backend-rust-orchestrator
export ORCHESTRATOR_HTTP_PORT=8182
export MEMORY_GRPC_ADDR=http://127.0.0.1:50052
export TOOLS_GRPC_ADDR=http://127.0.0.1:50054
export LOG_LEVEL=info
cargo run
```

**Expected Output:**
```
INFO backend_rust_orchestrator: Initializing Orchestrator
INFO backend_rust_orchestrator: Connected to Memory and Tools gRPC services
INFO backend_rust_orchestrator: Starting Orchestrator HTTP server
INFO backend_rust_orchestrator: Listening on 0.0.0.0:8182
```

**Verification:**
- Service should be listening on port `8182`
- Both gRPC clients connected successfully
- HTTP server started

---

### Terminal 4: Gateway

```bash
cd backend-rust-gateway
export GATEWAY_PORT=8181
export ORCHESTRATOR_URL=http://127.0.0.1:8182
export TELEMETRY_URL=http://127.0.0.1:8183
export LOG_LEVEL=info
cargo run
```

**Expected Output:**
```
INFO backend_rust_gateway: Initializing WebSocket Gateway
INFO backend_rust_gateway: Starting WebSocket Gateway server
INFO backend_rust_gateway: Listening on 0.0.0.0:8181
```

**Verification:**
- Service should be listening on port `8181`
- WebSocket endpoint ready at `ws://127.0.0.1:8181/ws/chat/:user_id`

---

### Terminal 5: Frontend

```bash
cd frontend-digital-twin
export VITE_WS_URL=ws://127.0.0.1:8181/ws/chat
export VITE_SSE_URL=http://127.0.0.1:8181/v1/telemetry/stream
npm run dev
```

**Expected Output:**
```
  VITE v6.2.0  ready in XXX ms

  ➜  Local:   http://localhost:3000/
  ➜  Network: use --host to expose
```

**Verification:**
- Frontend should be accessible at `http://localhost:3000`
- No build errors
- Browser can connect to the application

---

## Phase 2: Health Check Verification

Before proceeding with functional tests, verify all services are healthy.

### Gateway Health Check

```bash
curl http://127.0.0.1:8181/api/health
```

**Expected Response:**
```json
"Gateway operational"
```

### Orchestrator Health Check

```bash
curl http://127.0.0.1:8182/health
```

**Expected Response:**
```json
{
  "service": "backend-rust-orchestrator",
  "status": "ok",
  "version": "0.1.0"
}
```

### Memory Service Health Check (gRPC)

If you have `grpc_health_probe` installed:

```bash
grpc_health_probe -addr 127.0.0.1:50052
```

**Expected Response:**
```
status: SERVING
```

### Tools Service Health Check (gRPC)

```bash
grpc_health_probe -addr 127.0.0.1:50054
```

**Expected Response:**
```
status: SERVING
```

---

## Phase 3: Functional Test Cases

### Test Case 1: Memory Query (ACTION_MEMORY)

**Objective:** Verify the complete flow from frontend WebSocket → Gateway → Orchestrator → Memory Service → Response

**Steps:**

1. **Open Frontend**: Navigate to `http://localhost:3000` in your browser

2. **Create/Select Twin**: 
   - If no twin exists, create a new twin (e.g., "test-twin")
   - Select the twin to activate the chat interface

3. **Send Memory Query Message**:
   - In the chat input, type: `"search for recent SSH attacks"`
   - Press Enter or click Send

4. **Expected Frontend Behavior**:
   - Message appears in chat history
   - Connection status shows "Connected" (green indicator)
   - Response appears within 1-2 seconds
   - Response contains memory query results

5. **Expected Gateway Logs** (Terminal 4):
   ```
   INFO backend_rust_gateway: WebSocket connection established for user: test-twin
   INFO backend_rust_gateway: Processing chat message
   INFO backend_rust_gateway: Proxying request to Orchestrator
   ```

6. **Expected Orchestrator Logs** (Terminal 3):
   ```
   INFO backend_rust_orchestrator: Received chat request
   INFO backend_rust_orchestrator: Mock LLM planning
   INFO backend_rust_orchestrator: Executing memory query action
   INFO backend_rust_orchestrator: Memory query completed: X results
   ```

7. **Expected Memory Service Logs** (Terminal 1):
   ```
   INFO backend_rust_memory: QueryMemory request received
   INFO backend_rust_memory: Returning mock memory results
   ```

8. **Expected Response Format**:
   ```json
   {
     "type": "complete_message",
     "id": "uuid",
     "content": "Found X memory results for query 'search for recent SSH attacks'. Top result: ...",
     "is_final": true,
     "latency_ms": 150,
     "source_memories": ["Memory query: X results found"],
     "issued_command": null
   }
   ```

**Verification Checklist:**
- [ ] WebSocket connection established
- [ ] Message sent successfully
- [ ] Response received within 2 seconds
- [ ] Response contains memory query results
- [ ] All service logs show successful processing
- [ ] No error messages in any terminal

---

### Test Case 2: Secure Tool Execution (ACTION_TOOL)

**Objective:** Verify the complete flow for tool execution, including the CommandModal UI interaction

**Steps:**

1. **Send Tool Execution Message**:
   - In the chat input, type: `"write a test file with content hello world"`
   - Press Enter or click Send

2. **Expected Frontend Behavior**:
   - Message appears in chat history
   - **CommandModal appears** with:
     - Title: "Agent Command: ExecuteLocalTool"
     - Prompt text describing the tool execution request
     - "Authorize & Execute" button
     - "Deny" button

3. **Expected Gateway Logs** (Terminal 4):
   ```
   INFO backend_rust_gateway: Processing chat message
   INFO backend_rust_gateway: Proxying request to Orchestrator
   ```

4. **Expected Orchestrator Logs** (Terminal 3):
   ```
   INFO backend_rust_orchestrator: Received chat request
   INFO backend_rust_orchestrator: Mock LLM planning
   INFO backend_rust_orchestrator: Executing tool action
   INFO backend_rust_orchestrator: Tool execution: file_write completed
   ```

5. **Expected Tools Service Logs** (Terminal 2):
   ```
   INFO backend_rust_tools: ExecutionRequest received
   INFO backend_rust_tools: Policy check passed
   INFO backend_rust_tools: Simulating command execution
   INFO backend_rust_tools: Execution completed successfully
   ```

6. **User Interaction - Authorize Execution**:
   - Click "Authorize & Execute" button in CommandModal
   - Modal should close
   - Response should appear in chat

7. **Expected Response Format** (after authorization):
   ```json
   {
     "type": "complete_message",
     "id": "uuid",
     "content": "Tool 'file_write' executed successfully. Output: ...",
     "is_final": true,
     "latency_ms": 200,
     "source_memories": ["Tool execution: file_write completed"],
     "issued_command": null
   }
   ```

8. **Alternative Test - Deny Execution**:
   - Send another tool command: `"run command ls -la"`
   - When CommandModal appears, click "Deny"
   - Expected: Modal closes, error message appears in chat

**Verification Checklist:**
- [ ] Message sent successfully
- [ ] CommandModal appears for tool execution
- [ ] Modal displays correct tool name and prompt
- [ ] Authorization button works
- [ ] Response received after authorization
- [ ] Deny button works correctly
- [ ] All service logs show successful processing
- [ ] Tools service logs show policy check and execution

---

### Test Case 3: Direct Response (ACTION_RESPONSE)

**Objective:** Verify the default response path when no specific action is triggered

**Steps:**

1. **Send Generic Message**:
   - In the chat input, type: `"Hello, how are you?"`
   - Press Enter or click Send

2. **Expected Frontend Behavior**:
   - Message appears in chat history
   - Response appears without CommandModal
   - Response is a direct message from the agent

3. **Expected Orchestrator Logs** (Terminal 3):
   ```
   INFO backend_rust_orchestrator: Received chat request
   INFO backend_rust_orchestrator: Mock LLM planning
   INFO backend_rust_orchestrator: Generating direct response
   ```

4. **Expected Response Format**:
   ```json
   {
     "type": "complete_message",
     "id": "uuid",
     "content": "I understand you said: 'Hello, how are you?'. Processing your request...",
     "is_final": true,
     "latency_ms": 100,
     "source_memories": ["Direct response generated"],
     "issued_command": null
   }
   ```

**Verification Checklist:**
- [ ] Message sent successfully
- [ ] Response received without CommandModal
- [ ] Response is appropriate for the input
- [ ] No errors in service logs

---

## Phase 4: Error Handling Tests

### Test Case 4: Service Unavailable

**Objective:** Verify graceful error handling when a downstream service is unavailable

**Steps:**

1. **Stop Memory Service** (Terminal 1):
   - Press `Ctrl+C` to stop the Memory service

2. **Send Memory Query**:
   - In the frontend, send: `"search for test data"`

3. **Expected Behavior**:
   - Error response appears in chat
   - Error message indicates service unavailable
   - Gateway logs show connection error
   - Orchestrator logs show gRPC error

4. **Expected Error Response**:
   ```json
   {
     "type": "status_update",
     "status": "error",
     "details": "Orchestrator error: Request to orchestrator failed: ..."
   }
   ```

**Verification Checklist:**
- [ ] Error message displayed to user
- [ ] Services continue running (no crashes)
- [ ] Error logs are clear and actionable

---

### Test Case 5: Invalid WebSocket Message

**Objective:** Verify error handling for malformed requests

**Steps:**

1. **Connect via WebSocket** (using `wscat` or similar):
   ```bash
   wscat -c ws://127.0.0.1:8181/ws/chat/test-user
   ```

2. **Send Invalid JSON**:
   ```
   {"invalid": "json", missing: quotes}
   ```

3. **Expected Behavior**:
   - Error response sent back
   - Gateway logs show parsing error
   - Connection remains open

4. **Expected Error Response**:
   ```json
   {
     "type": "status_update",
     "status": "error",
     "details": "Invalid JSON payload: ..."
   }
   ```

**Verification Checklist:**
- [ ] Error message sent to client
- [ ] Connection remains stable
- [ ] Logs show parsing error clearly

---

## Phase 5: Performance Verification

### Test Case 6: Concurrent Requests

**Objective:** Verify system handles multiple concurrent requests

**Steps:**

1. **Send Multiple Messages Rapidly**:
   - Send 5-10 messages in quick succession
   - Mix memory queries, tool executions, and direct messages

2. **Expected Behavior**:
   - All messages processed
   - Responses received in order (or with clear ordering)
   - No service crashes
   - Logs show concurrent processing

3. **Monitor Resource Usage**:
   - Check CPU and memory usage in each terminal
   - Verify no memory leaks
   - Check for connection pool exhaustion

**Verification Checklist:**
- [ ] All messages processed successfully
- [ ] No service crashes or panics
- [ ] Resource usage remains stable
- [ ] Logs show concurrent request handling

---

## Phase 6: Cleanup and Shutdown

### Graceful Shutdown

1. **Stop Frontend** (Terminal 5):
   - Press `Ctrl+C`

2. **Stop Gateway** (Terminal 4):
   - Press `Ctrl+C`
   - Verify WebSocket connections closed gracefully

3. **Stop Orchestrator** (Terminal 3):
   - Press `Ctrl+C`
   - Verify HTTP connections closed

4. **Stop Tools Service** (Terminal 2):
   - Press `Ctrl+C`

5. **Stop Memory Service** (Terminal 1):
   - Press `Ctrl+C`

### Verification

- All services stopped cleanly
- No orphaned processes
- Ports are free (can be verified with `netstat` or `lsof`)

---

## Troubleshooting Guide

### Common Issues and Solutions

#### Issue: Port Already in Use

**Symptoms:**
```
Error: Address already in use (os error 48)
```

**Solution:**
```bash
# Find process using port
lsof -i :8181  # or netstat -tulpn | grep 8181

# Kill process or use different port
export GATEWAY_PORT=8185
```

#### Issue: gRPC Connection Failed

**Symptoms:**
```
ERROR: Failed to connect to Memory service
```

**Solution:**
1. Verify Memory service is running on correct port
2. Check `MEMORY_GRPC_ADDR` environment variable
3. Ensure services are started in correct order

#### Issue: WebSocket Connection Failed

**Symptoms:**
```
WebSocket connection failed
```

**Solution:**
1. Verify Gateway is running
2. Check `VITE_WS_URL` in frontend matches Gateway port
3. Check browser console for detailed error

#### Issue: CommandModal Not Appearing

**Symptoms:**
- Tool execution message sent but no modal appears

**Solution:**
1. Check browser console for JavaScript errors
2. Verify `CommandModal` component is rendered in `App.tsx`
3. Check that `AgentCommand` is properly parsed
4. Verify WebSocket message contains `issued_command` field

#### Issue: Services Not Communicating

**Symptoms:**
- Requests timeout or return errors

**Solution:**
1. Verify all environment variables are set correctly
2. Check service logs for connection errors
3. Verify network connectivity between services
4. Check that services are on the same network (localhost)

---

## Success Criteria

The system is considered **fully functional** when:

- [x] All services start without errors
- [x] Health checks pass for all services
- [x] Memory queries return results
- [x] Tool execution triggers CommandModal
- [x] Direct responses work correctly
- [x] Error handling is graceful
- [x] Concurrent requests are handled
- [x] Services shut down cleanly

---

## Next Steps

After successful completion of these tests:

1. **Integration Testing**: Test with real LLM integration
2. **Load Testing**: Test with higher concurrent load
3. **Security Testing**: Verify policy enforcement
4. **Endurance Testing**: Run system for extended periods
5. **Production Readiness**: Review logs, metrics, and monitoring

---

## Notes

- All test cases use **mock LLM planning** (keyword-based)
- Memory service returns **mock data** (not real vector DB)
- Tools service **simulates execution** (not real command execution)
- For production testing, replace mocks with real implementations

---

**Test Script Version:** 1.0  
**Last Updated:** 2024-01-10  
**Platform:** Ferrellgas AGI Multi Digital Twin Platform
