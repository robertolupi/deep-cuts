---
name: add-tauri-sidecar
description: How to bundle an external binary and its dylib dependencies with the Tauri app, patch rpaths, sign everything, and resolve the path at runtime
---

# Adding a Tauri Sidecar Binary

Sidecars are external executables bundled inside the `.app` and resolved at runtime. The project already uses this for `llama-server` and `fpcalc`. Follow the same pattern for any new binary.

> **Important**: if the binary depends on dynamic libraries (`.dylib`), there are extra macOS-specific steps around rpath patching and code signing. Read the full skill before starting.

---

## 1. Prepare the binary and its dylibs

### Simple binary (no dylib dependencies)

Copy the binary to `src-tauri/binaries/` with the **target-triple** suffix:

```
src-tauri/binaries/my-tool-aarch64-apple-darwin
```

```bash
chmod +x src-tauri/binaries/my-tool-aarch64-apple-darwin
```

### Binary with dylib dependencies (macOS)

If the binary links against `.dylib` files (check with `otool -L my-tool`), you must:

1. Copy the binary **and all its dylibs** into `src-tauri/binaries/`
2. Patch all hardcoded absolute rpath references to use `@rpath` / `@loader_path` instead
3. Add `@loader_path/../Frameworks` to the RPATH of every file so they find each other when Tauri bundles them into `Contents/Frameworks/`
4. Re-sign everything with your Apple Developer ID

**Use the helper script** `tools/bundle-llama-server.sh` as a reference — it does all of this for llama-server. Create a similar script for your binary.

The key `install_name_tool` operations each file needs:
```bash
# Replace Homebrew lib rpath with @loader_path
install_name_tool -rpath "/opt/homebrew/opt/foo/lib" "@loader_path" "$file"

# Add Frameworks rpath so bundled app finds dylibs in Contents/Frameworks/
install_name_tool -add_rpath "@loader_path/../Frameworks" "$file"

# Patch any remaining absolute /opt/homebrew/... install names → @rpath
install_name_tool -change "/opt/homebrew/opt/foo/lib/libfoo.dylib" "@rpath/libfoo.dylib" "$file"

# Re-sign (required after any install_name_tool modification)
codesign --force --options runtime --sign "Developer ID Application: Roberto Lupi (83BHH8484C)" "$file"
```

---

## 2. Declare in `tauri.conf.json`

```json
"bundle": {
  "externalBin": [
    "binaries/my-tool"
  ],
  "macOS": {
    "frameworks": [
      "binaries/libfoo.dylib",
      "binaries/libbar.0.dylib"
    ]
  }
}
```

**Critical**: dylib dependencies go in `bundle.macOS.frameworks`, **not** `bundle.resources`. The `frameworks` key places them in `Contents/Frameworks/` inside the `.app`, which is the path that `@loader_path/../Frameworks` in the rpath resolves to. Using `resources` puts them in the wrong location and causes a crash at runtime.

The path in `externalBin` is **without** the target-triple suffix — Tauri appends it automatically at bundle time.

---

## 3. Resolve the binary path at runtime in Rust

Do **not** use Tauri's `Command::sidecar()` API — this project resolves sidecar paths manually via `AppHandle`. Follow the pattern from `src-tauri/src/llama.rs` and `src-tauri/src/acoustid.rs`:

```rust
use tauri::{AppHandle, Manager};
use std::path::PathBuf;

fn get_my_tool_path(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(target_arch = "aarch64")]
    const TARGET_ARCH: &str = "aarch64";
    #[cfg(target_arch = "x86_64")]
    const TARGET_ARCH: &str = "x86_64";

    #[cfg(target_os = "macos")]
    const TARGET_OS: &str = "apple-darwin";
    #[cfg(target_os = "linux")]
    const TARGET_OS: &str = "unknown-linux-gnu";
    #[cfg(target_os = "windows")]
    const TARGET_OS: &str = "pc-windows-msvc";

    let triple = format!("{}-{}", TARGET_ARCH, TARGET_OS);
    let filename = format!("my-tool-{}", triple);

    // 1. Next to the executable — production .app where Tauri strips the triple suffix
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let p = exe_dir.join("my-tool");       // suffix-stripped name
            if p.exists() { return Some(p); }
            let p = exe_dir.join(&filename);        // triple-suffixed name (fallback)
            if p.exists() { return Some(p); }
        }
    }

    // 2. Resource dir — alternate production layout
    if let Ok(res_dir) = app.path().resource_dir() {
        let p = res_dir.join("binaries").join(&filename);
        if p.exists() { return Some(p); }
    }

    // 3. Dev paths relative to repo root (CARGO_MANIFEST_DIR points to src-tauri/)
    let dev_paths = vec![
        std::path::Path::new("src-tauri/binaries").join(&filename),
        std::path::Path::new("binaries").join(&filename),
    ];
    for p in dev_paths {
        if p.exists() { return Some(p); }
    }

    None
}
```

> **Location 1 must come first.** In a production `.app`, the binary lives in `Contents/MacOS/` next to the main executable. Checking the resource dir first causes a miss and the binary is never found.

Then spawn it:

```rust
let path = get_my_tool_path(app)
    .ok_or_else(|| "my-tool binary not found".to_string())?;

let output = std::process::Command::new(&path)
    .arg("--some-flag")
    .output()
    .map_err(|e| format!("Failed to run my-tool: {}", e))?;
```

---

## 4. Keeping the binary up to date

If the binary comes from Homebrew, update `tools/bundle-llama-server.sh` (or create a sibling script) that:
1. Copies the new binary + dylibs from Homebrew
2. Patches all rpaths with `install_name_tool`
3. Re-signs everything

Re-run the script after every `brew upgrade` of the relevant formula.

---

## 5. Checklist

- [ ] Binary in `src-tauri/binaries/` with target-triple suffix, `chmod +x`
- [ ] All `.dylib` dependencies in `src-tauri/binaries/`
- [ ] `@loader_path/../Frameworks` added to RPATH of binary and all dylibs
- [ ] All absolute `/opt/homebrew/...` install names patched to `@rpath/...`
- [ ] Everything re-signed with `codesign --force --options runtime`
- [ ] `externalBin` entry added to `tauri.conf.json`
- [ ] dylibs listed under `bundle.macOS.frameworks` (NOT `bundle.resources`)
- [ ] Runtime path resolver checks exe dir **before** resource dir
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` still passes
