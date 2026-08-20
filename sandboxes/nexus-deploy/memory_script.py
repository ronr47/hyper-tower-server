import ollama
import os

def get_context():
    memory_dir = os.path.expanduser("~/ollama_memory")
    context = ""
    if os.path.exists(memory_dir):
        for filename in os.listdir(memory_dir):
            with open(os.path.join(memory_dir, filename), 'r') as f:
                context += f.read() + "\n"
    return context

def ask_ai(query):
    context = get_context()
    # Inject context directly into the prompt for immediate adaptation
    full_prompt = f"--- MEMORY CONTEXT ---\n{context}\n--- USER QUERY ---\n{query}"
    
    response = ollama.generate(
        model='qwen2.5-advanced',
        prompt=full_prompt
    )
    print(response['response'])

if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        ask_ai(" ".join(sys.argv[1:]))
