from __future__ import annotations

import os

from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor


# Module-level tracer (becomes non-noop after setup_tracing() sets the provider).
tracer = trace.get_tracer(__name__)


def setup_tracing(service_name: str):
    """Initialize OpenTelemetry tracing and return a tracer.

    Default exporter target is OTLP/gRPC at http://localhost:4317.
    Override with OTEL_EXPORTER_OTLP_ENDPOINT.
    """

    global tracer

    endpoint = os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317")

    resource = Resource.create(
        {
            "service.name": service_name,
        }
    )

    provider = TracerProvider(resource=resource)
    trace.set_tracer_provider(provider)

    exporter = OTLPSpanExporter(endpoint=endpoint, insecure=True)
    provider.add_span_processor(BatchSpanProcessor(exporter))

    tracer = trace.get_tracer(service_name)
    return tracer

