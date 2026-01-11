#!/usr/bin/env bash
set -euo pipefail

# Generate Go protobuf + gRPC stubs from the shared model.proto.
# Run from repo root:
#   ./scripts/gen_go_stubs.sh

PROTO_FILE="backend-go-model-gateway/proto/model.proto"
OUT_DIR="backend-go-model-gateway/proto"

if [[ ! -f "${PROTO_FILE}" ]]; then
  echo "ERROR: proto file not found at '${PROTO_FILE}'" >&2
  exit 1
fi

command -v protoc >/dev/null 2>&1 || {
  echo "ERROR: protoc not found. Install protobuf compiler (protoc)." >&2
  exit 1
}

command -v protoc-gen-go >/dev/null 2>&1 || {
  echo "ERROR: protoc-gen-go not found. Install with:" >&2
  echo "  go install google.golang.org/protobuf/cmd/protoc-gen-go@latest" >&2
  exit 1
}

command -v protoc-gen-go-grpc >/dev/null 2>&1 || {
  echo "ERROR: protoc-gen-go-grpc not found. Install with:" >&2
  echo "  go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest" >&2
  exit 1
}

echo "Generating Go stubs from ${PROTO_FILE} -> ${OUT_DIR}"

protoc \
  --go_out="./${OUT_DIR}" --go_opt=paths=source_relative \
  --go-grpc_out="./${OUT_DIR}" --go-grpc_opt=paths=source_relative \
  "${PROTO_FILE}"

echo "Done. Generated/updated Go stubs in ${OUT_DIR}"

