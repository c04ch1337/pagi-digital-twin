# --- STAGE 1: BUILD ---
FROM golang:1.22 AS builder
WORKDIR /app

# Copy go mod files
COPY go.mod ./go.mod

# Download dependencies
RUN go mod download

# Copy source code
COPY main.go ./main.go

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -o /pagi-go-bff

# --- STAGE 2: RUNTIME ---
FROM gcr.io/distroless/base-debian12
WORKDIR /app
COPY --from=builder /pagi-go-bff ./pagi-go-bff
EXPOSE 8002

# Read port from environment variable GO_BFF_PORT, default to 8002
CMD ["/app/pagi-go-bff"]

