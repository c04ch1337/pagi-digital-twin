package main

import (
	"encoding/json"
	"net/http"
	"strconv"
)

// NewHTTPMux wires up the temporary HTTP endpoints for the model gateway.
//
// This is intentionally split out from main() so it can be verified via unit/integration
// tests without booting the full gRPC + LLM stack.
func NewHTTPMux(vectorClient RAGContextClient) *http.ServeMux {
	mux := http.NewServeMux()

	mux.HandleFunc("/api/v1/vector-test", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			w.WriteHeader(http.StatusMethodNotAllowed)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": "method not allowed"})
			return
		}

		q := r.URL.Query().Get("query")
		k := 2
		if ks := r.URL.Query().Get("k"); ks != "" {
			if parsed, err := strconv.Atoi(ks); err == nil && parsed > 0 {
				k = parsed
			}
		}
		if q == "" {
			w.WriteHeader(http.StatusBadRequest)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": "missing required query param: query"})
			return
		}

		matches, err := vectorClient.GetContext(r.Context(), VectorQueryRequest{QueryText: q, TopK: k})
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			_ = json.NewEncoder(w).Encode(map[string]any{"error": err.Error()})
			return
		}

		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(matches)
	})

	return mux
}
