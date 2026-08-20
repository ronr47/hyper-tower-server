#!/bin/bash

# Force Ollama to listen to all network interfaces inside the container
export OLLAMA_HOST=0.0.0.0:11434

# Start Ollama in the background
ollama serve &

# Wait for it to be ready
sleep 5

# Run your script with the arguments passed to 'docker run'
/app/ai_env/bin/python3 /app/memory_script.py "$@"

# CRITICAL: Keep the container running in the background
tail -f /dev/null
