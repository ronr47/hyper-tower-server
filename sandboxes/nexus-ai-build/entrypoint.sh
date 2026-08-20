#!/bin/bash
# Start Ollama in the background
ollama serve &

# Wait for server to wake up
sleep 5

# Pull the model (Only needed if not already in /app/models)
ollama pull qwen2.5-coder:1.5b

# Run your adaptive script
./ai_env/bin/python3 memory_script.py "System Check: Are you fully offline and operational?"
