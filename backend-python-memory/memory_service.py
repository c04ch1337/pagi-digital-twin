from __future__ import annotations

import asyncio
import hashlib
import json
import os
import sqlite3
import sys
from pathlib import Path
from typing import Any

import chromadb

from sentence_transformers import SentenceTransformer

from opentelemetry import trace
from opentelemetry.context import attach, detach
from opentelemetry.propagate import extract
from opentelemetry.propagators.textmap import Getter


RAG_KNOWLEDGE_BASES = ["Domain-KB", "Body-KB", "Soul-KB"]

# Mind-KB is an additional Chroma collection used for "evolving playbooks".
# Unlike the seeded KBs above, it starts empty and is appended to as tasks succeed.
MIND_KB_NAME = "Mind-KB"


class HashEmbeddingFunction:
	"""Backwards-compat placeholder.

	Historically, this service used a deterministic hash embedding to avoid pulling
	ML dependencies during early scaffolding.

	This is now superseded by SentenceTransformerEmbeddingFunction.
	"""

	def __init__(self, dim: int = 32):
		self.dim = dim

	def name(self) -> str:
		return "hash_embedding_v1"

	def get_config(self) -> dict[str, Any]:
		return {"dim": self.dim}

	@classmethod
	def build_from_config(cls, config: dict[str, Any]) -> "HashEmbeddingFunction":
		dim = int(config.get("dim", 32)) if isinstance(config, dict) else 32
		return cls(dim=dim)

	def __call__(self, input: list[str]) -> list[list[float]]:
		vectors: list[list[float]] = []
		for text in input:
			digest = hashlib.sha256(text.encode("utf-8")).digest()
			vec = [digest[i % len(digest)] / 255.0 for i in range(self.dim)]
			vectors.append(vec)
		return vectors


class SentenceTransformerEmbeddingFunction:
	"""Semantic embedding function using a locally-loaded SentenceTransformer model.

	This provides meaningful vector search for ChromaDB-backed RAG.
	"""

	def __init__(self, model_name: str = "all-MiniLM-L6-v2"):
		self.model_name = model_name
		self._model = SentenceTransformer(model_name)

	# Chroma embedding-function protocol metadata
	def name(self) -> str:
		return "sentence_transformers_v1"

	def get_config(self) -> dict[str, Any]:
		return {"model_name": self.model_name}

	@classmethod
	def build_from_config(cls, config: dict[str, Any]) -> "SentenceTransformerEmbeddingFunction":
		model_name = "all-MiniLM-L6-v2"
		if isinstance(config, dict):
			model_name = str(config.get("model_name") or model_name)
		return cls(model_name=model_name)

	def __call__(self, input: list[str]) -> list[list[float]]:
		# SentenceTransformer.encode returns numpy arrays by default; tolist() converts
		# to standard Python floats for compatibility with Chroma.
		embeddings = self._model.encode(input, convert_to_numpy=True)
		return embeddings.tolist()


_chroma_client: chromadb.ClientAPI | None = None
_embedding_fn = SentenceTransformerEmbeddingFunction(model_name=os.environ.get("EMBEDDING_MODEL_NAME", "all-MiniLM-L6-v2"))


def get_chroma_client() -> chromadb.ClientAPI:
	global _chroma_client
	if _chroma_client is None:
		# Prefer a remote Chroma server if configured (e.g. in Docker Compose).
		host = os.environ.get("CHROMA_HOST")
		port = os.environ.get("CHROMA_PORT")
		if host:
			# chromadb.HttpClient expects an int port; default is Chroma's HTTP port.
			port_i = int(port) if port and port.isdigit() else 8000
			_chroma_client = chromadb.HttpClient(host=host, port=port_i)
		else:
			# Default: local persistent Chroma (developer/bare-metal mode).
			_chroma_client = chromadb.PersistentClient(path="./chroma_data")
	return _chroma_client


def get_collection(name: str):
	# Keep one collection per conceptual KB.
	client = get_chroma_client()
	return client.get_or_create_collection(name=name, embedding_function=_embedding_fn)


def check_health() -> tuple[bool, str]:
	"""Deep health check for critical dependencies.

	Returns:
		(ok, message)

	We intentionally keep this non-destructive:
	- ChromaDB: heartbeat() if available, else list_collections() as a reachability probe.
	- SQLite: open DB and execute a trivial query.
	"""
	# 1) ChromaDB reachability
	try:
		client = get_chroma_client()
		if hasattr(client, "heartbeat"):
			client.heartbeat()  # type: ignore[attr-defined]
		else:
			client.list_collections()
	except Exception as e:
		return False, f"chroma_unhealthy: {e}"

	# 2) SQLite reachability
	try:
		with _open_session_db() as conn:
			conn.execute("SELECT 1").fetchone()
	except Exception as e:
		return False, f"sqlite_unhealthy: {e}"

	return True, "ok"


def seed_rag_collections() -> None:
	"""Ensure each RAG KB has at least one record so retrieval works immediately."""
	for kb in RAG_KNOWLEDGE_BASES:
		collection = get_collection(kb)
		if collection.count() == 0:
			collection.add(
				ids=[f"seed-{kb}"],
				documents=[f"Seed document for {kb}. This is initial mock knowledge."],
				metadatas=[{"knowledge_base": kb, "kind": "seed"}],
			)


# Seed collections at import time so the service is usable immediately.
seed_rag_collections()


def rag_retrieve(query: str, knowledge_bases: list[str] | None = None, top_k: int = 1) -> list[dict[str, Any]]:
	"""Query Chroma across multiple conceptual KB collections.

	Returns a flat list of match dicts including the KB source for each match.
	"""
	if top_k <= 0:
		top_k = 1

	kb_list = knowledge_bases or []
	if len(kb_list) == 0:
		kb_list = ["Body-KB"]

	matches: list[dict[str, Any]] = []
	for kb in kb_list:
		collection = get_collection(kb)
		res = collection.query(query_texts=[query], n_results=top_k)

		ids = (res.get("ids") or [[]])[0]
		docs = (res.get("documents") or [[]])[0]
		dists = (res.get("distances") or [[]])[0]

		for i in range(min(len(ids), len(docs), len(dists))):
			matches.append(
				{
					"id": ids[i],
					"text": docs[i],
					"distance": dists[i],
					"knowledge_base": kb,
					"source": "chroma",
				}
			)

	return matches


# --- Mind-KB: evolving playbooks (successful multi-step tool sequences) ---


def store_mind_playbook(session_id: str, prompt: str, history_sequence: list[dict[str, str]]):
	"""Stores a successful task sequence (Playbook) into the Mind-KB for future RAG retrieval.

	This is the persistence target for the Agent Planner's learning loop.
	
	The caller is expected to pass a minimal, LLM-readable slice of the session history,
	containing:
	- the original user prompt
	- tool-plans and tool-results
	- the final successful assistant completion
	"""
	mind_kb_collection = get_collection(MIND_KB_NAME)

	# 1) Summarize the sequence into a dense text format for vector search.
	playbook_text = summarize_history_for_mind_kb(prompt, history_sequence)

	# 2) Stable ID for dedupe.
	playbook_id = hashlib.sha256(playbook_text.encode("utf-8")).hexdigest()

	# 3) Store in Chroma DB.
	# Prefer upsert if available (avoids duplicate-id errors).
	if hasattr(mind_kb_collection, "upsert"):
		mind_kb_collection.upsert(
			ids=[playbook_id],
			documents=[playbook_text],
			metadatas=[
				{
					"source_session": session_id,
					"original_prompt": prompt,
					"kind": "playbook",
				}
			],
		)
	else:
		# Fallback for older Chroma clients.
		try:
			mind_kb_collection.add(
				ids=[playbook_id],
				documents=[playbook_text],
				metadatas=[
					{
						"source_session": session_id,
						"original_prompt": prompt,
						"kind": "playbook",
					}
				],
			)
		except Exception:
			# If the ID already exists, treat it as a no-op.
			pass

	return playbook_id


def summarize_history_for_mind_kb(prompt: str, history_sequence: list[dict[str, str]]) -> str:
	"""Format the playbook sequence into dense, LLM-usable text.

	This is intentionally simple and deterministic. A future iteration can replace
	this with an LLM-based summarizer.
	"""
	lines: list[str] = []
	lines.append(f"Playbook for: {prompt}")
	lines.append("---")
	lines.append("Steps:")

	step = 1
	for item in history_sequence or []:
		if not isinstance(item, dict):
			continue
		role = str(item.get("role", "")).strip() or "unknown"
		content = str(item.get("content", "")).strip()
		if not content:
			continue

		# Normalize role labels (we use 'tool_result' as a pseudo-role).
		if role == "tool":
			role = "tool_result"

		prefix = {
			"user": "User prompt",
			"assistant": "Planner/Assistant",
			"tool_result": "Tool returned",
		}.get(role, role)

		lines.append(f"{step}) {prefix}: {content}")
		step += 1

	return "\n".join(lines)


# --- gRPC: expose RAG retrieval over the shared ModelGateway service ---

_PROTO_DIR = Path(__file__).resolve().parent / "proto"


def _ensure_proto_generated() -> None:
	"""Generate Python gRPC stubs into ./proto if they don't exist yet."""
	if (_PROTO_DIR / "model_pb2.py").exists() and (_PROTO_DIR / "model_pb2_grpc.py").exists():
		return

	# Generate stubs from the local copy of model.proto.
	from grpc_tools import protoc

	proto_file = _PROTO_DIR / "model.proto"
	result = protoc.main(
		[
			"grpc_tools.protoc",
			f"-I{_PROTO_DIR}",
			f"--python_out={_PROTO_DIR}",
			f"--grpc_python_out={_PROTO_DIR}",
			str(proto_file),
		]
	)
	if result != 0:
		raise RuntimeError(f"protoc failed with exit code {result}")


_ensure_proto_generated()

# Import proto modules - ensure proto directory is on sys.path for generated imports
# The generated model_pb2_grpc.py uses `import model_pb2`, so proto dir must be on sys.path
if str(_PROTO_DIR) not in sys.path:
	sys.path.insert(0, str(_PROTO_DIR))

import grpc  # noqa: E402
import model_pb2  # type: ignore[import-untyped]  # noqa: E402
import model_pb2_grpc  # type: ignore[import-untyped]  # noqa: E402

# Optional dependency: grpcio-health-checking.
#
# We want the Memory service to keep running in bare-metal/dev even if the extra
# package isn't installed yet. In Docker/CI, requirements.txt includes it.
try:  # noqa: E402
	from grpc_health.v1 import health_pb2, health_pb2_grpc  # type: ignore[import-untyped]
except Exception:  # noqa: E402
	health_pb2 = None
	health_pb2_grpc = None


class _GRPCMetadataGetter(Getter[list[tuple[str, str]]]):
	def get(self, carrier: list[tuple[str, str]], key: str) -> list[str] | None:
		if not key:
			return None
		key = key.lower()
		vals = [v for (k, v) in carrier if k.lower() == key]
		return vals or None

	def keys(self, carrier: list[tuple[str, str]]) -> list[str]:
		return [k for (k, _v) in carrier]


class ModelGatewayServicer(model_pb2_grpc.ModelGatewayServicer):
	async def GetRAGContext(self, request, context):
		# Extract OpenTelemetry trace context from incoming gRPC metadata.
		md = list(context.invocation_metadata())
		ctx = extract(_GRPCMetadataGetter(), md)
		token = attach(ctx)
		try:
			tracer = trace.get_tracer(__name__)
			with tracer.start_as_current_span("GetRAGContext"):
				query = request.query
				top_k = int(request.top_k) if request.top_k else 1
				kb_list = list(request.knowledge_bases)

				matches = rag_retrieve(query=query, knowledge_bases=kb_list, top_k=top_k)
				pb_matches = [
					model_pb2.RAGMatch(
						id=m.get("id", ""),
						text=m.get("text", ""),
						distance=float(m.get("distance", 0.0)),
						knowledge_base=m.get("knowledge_base", ""),
						source=m.get("source", ""),
					)
					for m in matches
				]
				return model_pb2.RAGContextResponse(matches=pb_matches)
		finally:
			detach(token)

	async def GetPlan(self, request, context):
		# This service exists only for RAG retrieval in the memory service.
		context.set_code(grpc.StatusCode.UNIMPLEMENTED)
		context.set_details("GetPlan is not implemented in backend-python-memory")
		return model_pb2.PlanResponse()


if health_pb2_grpc is not None and health_pb2 is not None:
	class HealthServicer(health_pb2_grpc.HealthServicer):
		"""gRPC Health Checking Protocol implementation."""

		async def Check(self, request, context):
			ok, msg = check_health()
			if not ok:
				context.set_details(msg)
				return health_pb2.HealthCheckResponse(status=health_pb2.HealthCheckResponse.NOT_SERVING)
			return health_pb2.HealthCheckResponse(status=health_pb2.HealthCheckResponse.SERVING)

		async def Watch(self, request, context):
			# Not needed for container/K8s readiness probes.
			await context.abort(grpc.StatusCode.UNIMPLEMENTED, "Watch is not implemented")
else:
	HealthServicer = None  # type: ignore[assignment]


_grpc_server: grpc.aio.Server | None = None


async def _serve_grpc(port: int) -> None:
	global _grpc_server
	server = grpc.aio.server()
	model_pb2_grpc.add_ModelGatewayServicer_to_server(ModelGatewayServicer(), server)
	if health_pb2_grpc is not None and HealthServicer is not None:
		health_pb2_grpc.add_HealthServicer_to_server(HealthServicer(), server)
	server.add_insecure_port(f"[::]:{port}")
	await server.start()
	_grpc_server = server
	await server.wait_for_termination()


def start_grpc_server_background(port: int = 50052) -> None:
	"""Start the gRPC server on the current asyncio event loop."""
	loop = asyncio.get_running_loop()
	loop.create_task(_serve_grpc(port))


def get_mock_session_history(session_id: str) -> list[dict]:
	return get_session_history(session_id)


# --- SQLite persistence for Episodic/Heart-KB (session history) ---

_SESSION_DB_PATH = "./session_history.db"


def _init_session_schema(conn: sqlite3.Connection) -> None:
	conn.execute(
		"""
		CREATE TABLE IF NOT EXISTS sessions (
			session_id TEXT PRIMARY KEY,
			history_json TEXT NOT NULL
		);
		"""
	)
	conn.commit()


def _open_session_db() -> sqlite3.Connection:
	conn = sqlite3.connect(_SESSION_DB_PATH)
	conn.row_factory = sqlite3.Row
	_init_session_schema(conn)
	return conn


def get_session_history(session_id: str) -> list[dict[str, Any]]:
	"""Load a session's chat history from SQLite.

	If no session exists yet, initialize an empty history row and return [].
	"""
	with _open_session_db() as conn:
		row = conn.execute(
			"SELECT history_json FROM sessions WHERE session_id = ?",
			(session_id,),
		).fetchone()
		if row is not None:
			raw = row["history_json"]
			try:
				parsed = json.loads(raw)
				return parsed if isinstance(parsed, list) else []
			except Exception:
				return []

		conn.execute(
			"INSERT INTO sessions (session_id, history_json) VALUES (?, ?)",
			(session_id, json.dumps([])),
		)
		conn.commit()
		return []

