FROM python:3.11-slim
WORKDIR /app

# Install only what's needed for the mock service.
RUN pip install --no-cache-dir fastapi uvicorn[standard]

COPY mock_memory_service.py .
EXPOSE 8003
CMD ["uvicorn", "mock_memory_service:app", "--host", "0.0.0.0", "--port", "8003"]

