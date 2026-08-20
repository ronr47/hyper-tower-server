FROM python:3.12-slim
ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app
COPY ollama /usr/local/bin/ollama
COPY ollama_pkg/ ./ollama_pkg/
COPY memory_script.py .
COPY entrypoint.sh .

# Added --no-deps to prevent it from looking for things you don't have
RUN python3 -m venv /app/ai_env &&     /app/ai_env/bin/pip install --no-index --find-links ./ollama_pkg/ ollama --only-binary=:all:

RUN mkdir -p /app/knowledge_base /app/models
ENV OLLAMA_MODELS=/app/models
RUN chmod +x entrypoint.sh
EXPOSE 11434
ENTRYPOINT ["./entrypoint.sh"]
