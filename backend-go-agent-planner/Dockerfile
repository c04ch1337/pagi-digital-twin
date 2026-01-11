# --- STAGE 1: BUILD ---
FROM golang:1.24 AS builder

# NOTE: this Dockerfile expects the Docker build context to be the repo root
# so it can include the replaced module `../backend-go-model-gateway`.

WORKDIR /src

COPY backend-go-agent-planner/ ./backend-go-agent-planner/
COPY backend-go-model-gateway/ ./backend-go-model-gateway/

WORKDIR /src/backend-go-agent-planner
RUN go mod download

# NOTE: Agent Planner uses SQLite (cgo via github.com/mattn/go-sqlite3), so CGO must be enabled.
RUN CGO_ENABLED=1 GOOS=linux go build -o /out/agent-planner

# --- STAGE 2: RUNTIME ---
FROM gcr.io/distroless/base-debian12
WORKDIR /app

COPY --from=builder /out/agent-planner ./agent-planner

EXPOSE 8080
CMD ["/app/agent-planner"]

