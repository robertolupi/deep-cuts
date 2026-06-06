#!/usr/bin/env python3
"""
Test script for evaluating Gemma 4 E2B-it audio processing capabilities in Python
without spawning or relying on the pinned `llama-server`.
"""

import sys
import argparse
import subprocess

def run_via_litert_lm(audio_path, prompt):
    """
    Runs Gemma 4 E2B-it using LiteRT-LM (TFLite optimized for edge/on-device).
    This runs via `uvx litert-lm` or the local environment to run the 3.2GB model
    locally on the GPU.
    """
    print(f"\n[LiteRT-LM] Loading Gemma 4 E2B-it to evaluate: {audio_path}")
    print(f"[LiteRT-LM] Prompt: '{prompt}'")
    
    # We execute uvx litert-lm as a subprocess to keep dependencies clean
    cmd = [
        "uvx", "litert-lm", "run",
        "--from-huggingface-repo=litert-community/gemma-4-E2B-it-litert-lm",
        "gemma-4-E2B-it.litertlm",
        "--backend=gpu",
        "--audio-backend", "cpu", # litert-lm requires CPU backend for audio processing currently
        "--attachment", audio_path,
        "--prompt", prompt
    ]
    
    try:
        result = subprocess.run(cmd, check=True, text=True)
        return result.returncode == 0
    except subprocess.CalledProcessError as e:
        print(f"\nError executing LiteRT-LM: {e}", file=sys.stderr)
        return False
    except FileNotFoundError:
        print("\nError: 'uv' / 'uvx' tool not found in PATH.", file=sys.stderr)
        return False

def run_via_transformers(audio_path, prompt):
    """
    Runs Gemma 4 E2B-it using standard Hugging Face Transformers pipeline (PyTorch).
    """
    print(f"\n[Transformers] Loading google/gemma-4-E2B-it via PyTorch...")
    try:
        import torch
        from transformers import pipeline
    except ImportError:
        print("Error: Missing required packages. Run: pip install torch transformers accelerate", file=sys.stderr)
        return False

    print(f"[Transformers] Running model on device: {'cuda' if torch.cuda.is_available() else 'mps' if torch.backends.mps.is_available() else 'cpu'}")
    print(f"[Transformers] Processing audio file: {audio_path}")
    
    try:
        pipe = pipeline(
            task="any-to-any",
            model="google/gemma-4-E2B-it",
            device_map="auto",
            torch_dtype=torch.float16 if torch.cuda.is_available() or torch.backends.mps.is_available() else torch.float32
        )
        
        messages = [
            {
                "role": "user",
                "content": [
                    {"type": "audio", "audio": audio_path},
                    {"type": "text", "text": prompt}
                ]
            }
        ]
        
        print("[Transformers] Generating response...")
        result = pipe(messages)
        print("\n=== Model Response ===")
        print(result)
        print("======================")
        return True
    except Exception as e:
        print(f"\nError during transformers inference: {e}", file=sys.stderr)
        return False

def main():
    parser = argparse.ArgumentParser(description="Evaluate Gemma 4 E2B-it audio processing in Python.")
    parser.add_argument("audio", help="Path to the WAV/MP3 audio file to test.")
    parser.add_argument("--prompt", default="Describe what you hear in this audio.", help="Prompt to feed to the model.")
    parser.add_argument("--backend", choices=["litert", "transformers"], default="litert", 
                        help="Backend to use. 'litert' (default) uses LiteRT-LM (fast, quantized). 'transformers' uses PyTorch (unquantized/full).")
    
    args = parser.parse_args()
    
    if args.backend == "litert":
        success = run_via_litert_lm(args.audio, args.prompt)
    else:
        success = run_via_transformers(args.audio, args.prompt)
        
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()
