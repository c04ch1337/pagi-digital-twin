package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"testing"
)

type fakeRAGClient struct{}

func (fakeRAGClient) GetContext(_ context.Context, req VectorQueryRequest) ([]VectorQueryMatch, error) {
	kb := "Body-KB"
	if len(req.KnowledgeBases) > 0 {
		kb = req.KnowledgeBases[0]
	}
	return []VectorQueryMatch{
		{
			ID:            "fake-1",
			Score:         0.99,
			Text:          "Fake context snippet from " + kb + " - query: \"" + req.QueryText + "\" (top_k=3)",
			Source:        "fake",
			KnowledgeBase: kb,
		},
	}, nil
}

func TestVectorTestEndpoint_DefaultsToBodyKBAndEchoesQueryAndTopK(t *testing.T) {
	vectorClient := fakeRAGClient{}
	srv := httptest.NewServer(NewHTTPMux(vectorClient))
	t.Cleanup(srv.Close)

	queryText := "What is the protocol for new users?"
	topK := 3

	u, err := url.Parse(srv.URL + "/api/v1/vector-test")
	if err != nil {
		t.Fatalf("parse base url: %v", err)
	}
	q := u.Query()
	q.Set("query", queryText)
	q.Set("k", "3")
	u.RawQuery = q.Encode()

	resp, err := http.Get(u.String())
	if err != nil {
		t.Fatalf("http get: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected status 200, got %d", resp.StatusCode)
	}

	var matches []VectorQueryMatch
	if err := json.NewDecoder(resp.Body).Decode(&matches); err != nil {
		t.Fatalf("decode json: %v", err)
	}

	if len(matches) != 1 {
		t.Fatalf("expected exactly 1 match (default KB behavior), got %d", len(matches))
	}

	if matches[0].ID != "fake-1" {
		t.Fatalf("unexpected ids: %#v", matches)
	}

	expected1 := "Fake context snippet from Body-KB - query: \"" + queryText + "\" (top_k=3)"
	if matches[0].Text != expected1 {
		t.Fatalf("unexpected match[0].text\nexpected: %q\nactual:   %q", expected1, matches[0].Text)
	}

	if matches[0].Source != "fake" {
		t.Fatalf("unexpected sources: %#v", matches)
	}

	if matches[0].KnowledgeBase != "Body-KB" {
		t.Fatalf("unexpected knowledge base: %#v", matches)
	}

	_ = topK // explicit to emphasize the intent of the test
}

func TestVectorTestEndpoint_MissingQueryParam_Returns400(t *testing.T) {
	vectorClient := fakeRAGClient{}
	srv := httptest.NewServer(NewHTTPMux(vectorClient))
	t.Cleanup(srv.Close)

	resp, err := http.Get(srv.URL + "/api/v1/vector-test?k=3")
	if err != nil {
		t.Fatalf("http get: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusBadRequest {
		t.Fatalf("expected status 400, got %d", resp.StatusCode)
	}
}
