from fastapi import FastAPI
import uvicorn
import os
import time
import json
from datetime import datetime

app = FastAPI(title="Mock Memory Service")
SERVICE_NAME = "mock_memory_service"
VERSION = "1.0.0"
PORT = int(os.environ.get("MEMORY_MOCK_PORT", 8003))


# Middleware for structured JSON logging
@app.middleware("http")
async def log_requests(request, call_next):
    start_time = time.time()
    response = await call_next(request)
    process_time = time.time() - start_time

    log_entry = {
        "timestamp": datetime.now().isoformat(),
        "level": "info",
        "service": SERVICE_NAME,
        "method": request.method,
        "path": request.url.path,
        "status": response.status_code,
        "latency_ms": round(process_time * 1000, 2),
    }
    print(json.dumps(log_entry))
    return response


@app.get("/health")
def health_check():
    return {"service": SERVICE_NAME, "status": "ok", "version": VERSION}


@app.get("/memory/latest")
def get_latest_memory():
    # Placeholder payload
    return {
        "source": SERVICE_NAME,
        "latest": {
            "id": f"mem-{int(time.time())}",
            "summary": "hello from mock memory: knowledge chunk 42 retrieved.",
            "type": "Episodic KB",
        },
    }


if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=PORT)

