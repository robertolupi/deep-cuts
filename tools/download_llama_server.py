#!/usr/bin/env python3
"""
Download and extract pre-compiled llama-server from GitHub releases
directly into the src-tauri/binaries/ folder with platform-specific target-triple naming.

Run from repository root:
    python tools/download_llama_server.py
"""

import os
import sys
import platform
import urllib.request
import urllib.error
import tarfile
import zipfile
import tempfile
import shutil
from pathlib import Path

# Base release download URL template
GITHUB_RELEASES_URL = "https://github.com/ggml-org/llama.cpp/releases"

def get_latest_tag() -> str:
    """Fetch the latest release tag name from GitHub via redirects."""
    url = f"{GITHUB_RELEASES_URL}/latest"
    print("Fetching latest release tag from GitHub...")
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"})
    try:
        with urllib.request.urlopen(req) as response:
            actual_url = response.geturl()
            # E.g. https://github.com/ggml-org/llama.cpp/releases/tag/b4100
            tag = actual_url.split('/')[-1]
            if tag and tag.startswith('b'):
                print(f"Resolved latest release tag: {tag}")
                return tag
    except Exception as e:
        print(f"Error fetching redirect: {e}")
    
    # Robust fallback tag
    fallback = "b4100"
    print(f"Falling back to pinned release tag: {fallback}")
    return fallback

def get_platform_info():
    """Detect OS, Architecture, and construct the target-triple suffix."""
    os_name = platform.system().lower()
    arch = platform.machine().lower()

    # Translate standard architectures
    if arch in ["arm64", "aarch64"]:
        arch_tauri = "aarch64"
        arch_github = "arm64"
    elif arch in ["x86_64", "amd64", "x64"]:
        arch_tauri = "x86_64"
        arch_github = "x64"
    else:
        print(f"Unsupported architecture: {arch}")
        sys.exit(1)

    if os_name == "darwin":
        os_tauri = "apple-darwin"
        asset_suffix = f"bin-macos-{arch_github}.tar.gz"
        binary_name = "llama-server"
    elif os_name == "windows":
        os_tauri = "pc-windows-msvc"
        asset_suffix = f"bin-win-simple-{arch_github}.zip"
        binary_name = "llama-server.exe"
    elif os_name == "linux":
        os_tauri = "unknown-linux-gnu"
        # Standard Ubuntu build is generally highly compatible for Linux hosts
        asset_suffix = f"bin-ubuntu-{arch_github}.tar.gz"
        binary_name = "llama-server"
    else:
        print(f"Unsupported operating system: {os_name}")
        sys.exit(1)

    triple = f"{arch_tauri}-{os_tauri}"
    dest_binary_name = f"llama-server-{triple}"
    if os_name == "windows":
        dest_binary_name += ".exe"

    return asset_suffix, binary_name, dest_binary_name

def download_and_extract(url: str, asset_suffix: str, source_bin: str, dest_path: Path):
    """Download the archive, extract the llama-server binary, and place it in dest_path."""
    temp_dir = Path(tempfile.mkdtemp())
    archive_path = temp_dir / f"archive.{'zip' if asset_suffix.endswith('.zip') else 'tar.gz'}"

    print(f"Downloading from: {url}")
    print(f"Downloading pre-compiled archive ({archive_path.name}) ...")

    def report_hook(block_num, block_size, total_size):
        if total_size <= 0:
            return
        downloaded = block_num * block_size
        percent = min(100.0, downloaded * 100.0 / total_size)
        sys.stdout.write(f"\r  Progress: {percent:.1f}% ({downloaded / (1024*1024):.1f} MB of {total_size / (1024*1024):.1f} MB)")
        sys.stdout.flush()

    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        with urllib.request.urlopen(req) as response, open(archive_path, 'wb') as out_file:
            # We don't have reporthook directly in urlopen, but we can stream it
            total_size = int(response.info().get('Content-Length', 0))
            downloaded = 0
            block_size = 8192
            while True:
                buffer = response.read(block_size)
                if not buffer:
                    break
                downloaded += len(buffer)
                out_file.write(buffer)
                if total_size > 0:
                    percent = min(100.0, downloaded * 100.0 / total_size)
                    sys.stdout.write(f"\r  Progress: {percent:.1f}% ({downloaded / (1024*1024):.1f} MB of {total_size / (1024*1024):.1f} MB)")
                    sys.stdout.flush()
        print("\n  Download complete!")
    except Exception as e:
        print(f"\n  ERROR: Download failed: {e}")
        shutil.rmtree(temp_dir)
        sys.exit(1)

    print("Extracting files from archive...")
    extracted_binary = None
    extracted_libs = []

    # Common shared library extensions
    lib_extensions = {".dylib", ".so", ".dll"}

    try:
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                for member in zip_ref.namelist():
                    member_path = Path(member)
                    if member_path.name == source_bin:
                        zip_ref.extract(member, temp_dir)
                        extracted_binary = temp_dir / member
                    elif member_path.suffix in lib_extensions:
                        zip_ref.extract(member, temp_dir)
                        extracted_libs.append(temp_dir / member)
        else: # .tar.gz
            with tarfile.open(archive_path, 'r:gz') as tar_ref:
                for member in tar_ref.getmembers():
                    member_path = Path(member.name)
                    if member_path.name == source_bin:
                        tar_ref.extract(member, temp_dir)
                        extracted_binary = temp_dir / member.name
                    elif member_path.suffix in lib_extensions:
                        tar_ref.extract(member, temp_dir)
                        extracted_libs.append(temp_dir / member.name)
    except Exception as e:
        print(f"ERROR: Extraction failed: {e}")
        shutil.rmtree(temp_dir)
        sys.exit(1)

    if extracted_binary and extracted_binary.exists():
        # Ensure parent folder exists
        dest_path.parent.mkdir(parents=True, exist_ok=True)

        # Clean up existing binary if it exists
        if dest_path.exists():
            dest_path.unlink()
        
        # Copy binary to final destination
        shutil.copy2(extracted_binary, dest_path)
        print(f"Staged binary successfully at: {dest_path.resolve()}")
        
        # Copy associated shared libraries to same directory
        for lib in extracted_libs:
            dest_lib = dest_path.parent / lib.name
            if dest_lib.exists():
                dest_lib.unlink()
            shutil.copy2(lib, dest_lib)
            print(f"Staged dynamic library successfully at: {dest_lib.resolve()}")

        # Set executable permissions on Unix systems
        if platform.system().lower() != "windows":
            dest_path.chmod(dest_path.stat().st_mode | 0o111)
            print("Set executable permissions (+x)")
    else:
        print(f"ERROR: Could not find '{source_bin}' inside the downloaded archive.")
        shutil.rmtree(temp_dir)
        sys.exit(1)

    # Clean up temp files
    shutil.rmtree(temp_dir)

def main():
    repo_root = Path(__file__).parent.parent
    binaries_dir = repo_root / "src-tauri" / "binaries"

    print("=== Deep Cuts llama-server Sidecar Downloader ===")

    tag = get_latest_tag()
    asset_suffix, source_bin, dest_binary_name = get_platform_info()
    
    dest_path = binaries_dir / dest_binary_name
    download_url = f"{GITHUB_RELEASES_URL}/download/{tag}/llama-{tag}-{asset_suffix}"

    print(f"Platform: {platform.system()} ({platform.machine()})")
    print(f"Source file inside release: {source_bin}")
    print(f"Target sidecar name: {dest_binary_name}")
    print(f"Staging folder: {binaries_dir.resolve()}\n")

    if dest_path.exists():
        print(f"Staged binary '{dest_binary_name}' already exists.")
        choice = input("Do you want to force re-download and overwrite it? (y/N): ").strip().lower()
        if choice != 'y':
            print("Skipping download.")
            return

    download_and_extract(download_url, asset_suffix, source_bin, dest_path)
    print("\nllama-server sidecar setup completed successfully!")

if __name__ == "__main__":
    main()
