#!/usr/bin/env python3
"""
Download and stage the pre-compiled fpcalc (Chromaprint) sidecar binary from GitHub
into the src-tauri/binaries/ folder with platform-specific target-triple naming.

Run from repository root:
    python tools/download_fpcalc.py
"""

import os
import sys
import platform
import urllib.request
import tarfile
import zipfile
import tempfile
import shutil
from pathlib import Path

VERSION = "1.6.0"
GITHUB_RELEASE_URL = f"https://github.com/acoustid/chromaprint/releases/download/v{VERSION}"

def get_platform_info():
    """Detect OS, Architecture, and construct the target-triple suffix."""
    os_name = platform.system().lower()
    arch = platform.machine().lower()

    if arch in ["arm64", "aarch64"]:
        arch_tauri = "aarch64"
        arch_github = "arm64"
    elif arch in ["x86_64", "amd64", "x64"]:
        arch_tauri = "x86_64"
        arch_github = "x86_64"
    else:
        print(f"Unsupported architecture: {arch}")
        sys.exit(1)

    if os_name == "darwin":
        os_tauri = "apple-darwin"
        # We can use the macOS universal binary for all Mac hosts
        asset_name = f"chromaprint-fpcalc-{VERSION}-macos-universal.tar.gz"
        source_bin = "chromaprint-fpcalc-1.6.0-macos-universal/bin/fpcalc"
    elif os_name == "windows":
        os_tauri = "pc-windows-msvc"
        asset_name = f"chromaprint-fpcalc-{VERSION}-windows-x86_64.zip"
        source_bin = "chromaprint-fpcalc-1.6.0-windows-x86_64/bin/fpcalc.exe"
    elif os_name == "linux":
        os_tauri = "unknown-linux-gnu"
        asset_name = f"chromaprint-fpcalc-{VERSION}-linux-{arch_github}.tar.gz"
        source_bin = f"chromaprint-fpcalc-1.6.0-linux-{arch_github}/bin/fpcalc"
    else:
        print(f"Unsupported operating system: {os_name}")
        sys.exit(1)

    triple = f"{arch_tauri}-{os_tauri}"
    dest_binary_name = f"fpcalc-{triple}"
    if os_name == "windows":
        dest_binary_name += ".exe"

    return asset_name, source_bin, dest_binary_name

def download_and_extract(url: str, asset_name: str, source_bin: str, dest_path: Path):
    """Download the archive, extract the fpcalc binary, and place it in dest_path."""
    temp_dir = Path(tempfile.mkdtemp())
    archive_path = temp_dir / asset_name

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

    print("Extracting fpcalc binary from archive...")
    extracted_binary = None

    try:
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                for member in zip_ref.namelist():
                    # Handle both relative path matching and direct filename matching
                    if member == source_bin or Path(member).name == "fpcalc.exe":
                        zip_ref.extract(member, temp_dir)
                        extracted_binary = temp_dir / member
                        break
        else: # .tar.gz
            with tarfile.open(archive_path, 'r:gz') as tar_ref:
                for member in tar_ref.getmembers():
                    if member.name == source_bin or Path(member.name).name == "fpcalc":
                        tar_ref.extract(member, temp_dir)
                        extracted_binary = temp_dir / member.name
                        break
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

    print("=== Deep Cuts fpcalc Sidecar Downloader ===")

    asset_name, source_bin, dest_binary_name = get_platform_info()
    dest_path = binaries_dir / dest_binary_name
    download_url = f"{GITHUB_RELEASE_URL}/{asset_name}"

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

    download_and_extract(download_url, asset_name, source_bin, dest_path)
    print("\nfpcalc sidecar setup completed successfully!")

if __name__ == "__main__":
    main()
