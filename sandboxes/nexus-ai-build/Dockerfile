# Stage 1: Build the Environment
FROM python:3.11-slim AS builder

RUN apt-get update && apt-get install -y \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Ollama
RUN curl -fsSL https://ollama.com/install.sh | sh

# Setup Python Virtual Env for the Agent
WORKDIR /app
COPY ollama_pkg/ ./ollama_pkg/
RUN python3 -m venv /app/ai_env && \
    /app/ai_env/bin/pip install --no-cache-dir --no-index --find-links ./ollama_pkg/ ollama

# Stage 2: Final Runtime
FROM python:3.11-slim

WORKDIR /app
COPY --from=builder /usr/local/bin/ollama /usr/local/bin/ollama
COPY --from=builder /app/ai_env /app/ai_env
COPY memory_script.py .
COPY ~/ollama_memory/ ./knowledge_base/

# Ensure Ollama has a place to store weights
ENV OLLAMA_MODELS=/app/models
RUN mkdir -p /app/models

# Expose the API
EXPOSE 11434

# Start script: Runs Ollama in background, then the Agent
COPY entrypoint.sh .
RUN chmod +x entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
