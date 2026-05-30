#!/usr/bin/env python3
"""
Download Qwen2-Audio-7B-Instruct GGUF model files from HuggingFace.
Output files are written directly into the models/ directory.

Run from the repository root:
    python tools/download_models.py
"""

import os
import sys
import urllib.request
from pathlib import Path

REPO_URL = "https://huggingface.co/mradermacher/Qwen2-Audio-7B-Instruct-GGUF/resolve/main"

MODELS = [
    {
        "filename": "Qwen2-Audio-7B-Instruct.Q4_K_M.gguf",
        "size_desc": "4.7 GB"
    },
    {
        "filename": "Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf",
        "size_desc": "0.3 GB"
    }
]

def download_file(url: str, dest: Path, desc: str):
    """Download a file with simple text progress indication."""
    print(f"Downloading {dest.name} ({desc}) ...")
    
    def report_hook(block_num, block_size, total_size):
        if total_size <= 0:
            return
        downloaded = block_num * block_size
        percent = min(100.0, downloaded * 100.0 / total_size)
        sys.stdout.write(f"\r  Progress: {percent:.1f}% ({downloaded / (1024*1024):.1f} MB of {total_size / (1024*1024):.1f} MB)")
        sys.stdout.flush()

    try:
        urllib.request.urlretrieve(url, str(dest), reporthook=report_hook)
        print("\n  Download completed successfully!")
    except Exception as e:
        print(f"\n  ERROR: Failed to download {dest.name}: {e}")
        # Clean up partial download
        if dest.exists():
            dest.unlink()
        sys.exit(1)

def main():
    repo_root = Path(__file__).parent.parent
    models_dir = repo_root / "models"
    models_dir.mkdir(parents=True, exist_ok=True)

    print("=== Deep Cuts Qwen2-Audio Model Downloader ===")
    print(f"Destination folder: {models_dir.resolve()}\n")

    for m in MODELS:
        filename = m["filename"]
        size_desc = m["size_desc"]
        dest_path = models_dir / filename
        url = f"{REPO_URL}/{filename}"

        if dest_path.exists():
            print(f"File {filename} already exists, skipping.")
        else:
            download_file(url, dest_path, size_desc)

    print("\nAll models checked successfully!")

if __name__ == "__main__":
    main()
