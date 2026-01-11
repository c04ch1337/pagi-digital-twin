#!/usr/bin/env bash
set -euo pipefail

# Generates self-signed mTLS certificates for local research/testing.
#
# Output directory (project root): tls_certs/
#
# Outputs:
#   - tls_certs/ca.crt
#   - tls_certs/server.crt, tls_certs/server.key
#   - tls_certs/client.crt, tls_certs/client.key
#
# Note: We also keep the CA private key at tls_certs/ca.key (required to re-issue certs).

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/tls_certs"

rm -rf "${OUT_DIR}"
mkdir -p "${OUT_DIR}"

CA_KEY="${OUT_DIR}/ca.key"
CA_CRT="${OUT_DIR}/ca.crt"

SERVER_KEY="${OUT_DIR}/server.key"
SERVER_CSR="${OUT_DIR}/server.csr"
SERVER_CRT="${OUT_DIR}/server.crt"
SERVER_EXT="${OUT_DIR}/server.ext"

CLIENT_KEY="${OUT_DIR}/client.key"
CLIENT_CSR="${OUT_DIR}/client.csr"
CLIENT_CRT="${OUT_DIR}/client.crt"
CLIENT_EXT="${OUT_DIR}/client.ext"

echo "[gen_certs] generating CA..."
openssl genrsa -out "${CA_KEY}" 4096
openssl req -x509 -new -nodes -key "${CA_KEY}" -sha256 -days 3650 \
  -out "${CA_CRT}" \
  -subj "/C=US/ST=IL/L=Chicago/O=PAGI/OU=Dev/CN=pagi-dev-ca"

echo "[gen_certs] generating server certificate..."
openssl genrsa -out "${SERVER_KEY}" 4096
openssl req -new -key "${SERVER_KEY}" -out "${SERVER_CSR}" \
  -subj "/C=US/ST=IL/L=Chicago/O=PAGI/OU=Dev/CN=model-gateway"

cat >"${SERVER_EXT}" <<'EOF'
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = model-gateway
DNS.2 = localhost
IP.1 = 127.0.0.1
EOF

openssl x509 -req -in "${SERVER_CSR}" -CA "${CA_CRT}" -CAkey "${CA_KEY}" \
  -CAserial "${OUT_DIR}/ca.srl" -CAcreateserial \
  -out "${SERVER_CRT}" -days 3650 -sha256 -extfile "${SERVER_EXT}"

echo "[gen_certs] generating client certificate..."
openssl genrsa -out "${CLIENT_KEY}" 4096
openssl req -new -key "${CLIENT_KEY}" -out "${CLIENT_CSR}" \
  -subj "/C=US/ST=IL/L=Chicago/O=PAGI/OU=Dev/CN=agent-planner"

cat >"${CLIENT_EXT}" <<'EOF'
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = clientAuth
EOF

openssl x509 -req -in "${CLIENT_CSR}" -CA "${CA_CRT}" -CAkey "${CA_KEY}" \
  -CAserial "${OUT_DIR}/ca.srl" \
  -out "${CLIENT_CRT}" -days 3650 -sha256 -extfile "${CLIENT_EXT}"

rm -f "${SERVER_CSR}" "${CLIENT_CSR}" "${SERVER_EXT}" "${CLIENT_EXT}" "${OUT_DIR}/ca.srl"

echo "[gen_certs] done. certs written to: ${OUT_DIR}"
