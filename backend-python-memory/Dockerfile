FROM python:3.11-slim

WORKDIR /app

# Install runtime deps
COPY backend-python-memory/requirements.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

# Copy service code
COPY backend-python-memory/ ./

# HTTP API + embedded gRPC server (started on FastAPI startup)
EXPOSE 8003
EXPOSE 50052

CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8003"]

