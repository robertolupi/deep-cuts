use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub struct LlamaServerState {
    pub child: Mutex<Option<Child>>,
    pub port: Mutex<Option<u16>>,
}

fn find_free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind ephemeral port: {}", e))
        .map(|l| l.local_addr().unwrap().port())
}

pub struct LlamaServerGuard<'a> {
    pub app: &'a AppHandle,
    pub killed_on_drop: bool,
}

impl<'a> Drop for LlamaServerGuard<'a> {
    fn drop(&mut self) {
        if self.killed_on_drop {
            log::info!(
                "[llama-server] LlamaServerGuard dropped. Terminating background process..."
            );
            terminate_llama_server(self.app);
        }
    }
}

/// Resolves the absolute path of the bundled sidecar binary at runtime.
fn get_sidecar_path(app: &AppHandle) -> Option<PathBuf> {
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
    let filename = if cfg!(target_os = "windows") {
        format!("llama-server-{}.exe", triple)
    } else {
        format!("llama-server-{}", triple)
    };

    // 1. Check tauri resource dir (production package)
    if let Ok(res_dir) = app.path().resource_dir() {
        let p = res_dir.join("binaries").join(&filename);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Check local dev directories relative to repository root
    let dev_paths = vec![
        Path::new("src-tauri/binaries").join(&filename),
        Path::new("binaries").join(&filename),
        Path::new("../src-tauri/binaries").join(&filename),
    ];

    for p in dev_paths {
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Spawns the llama-server executable. Tries the bundled sidecar first, then fallbacks.
fn spawn_llama_server(app: &AppHandle, model_path: &str, mmproj_path: &str, port: u16) -> Result<Child, String> {
    let mut executables = vec![];

    // 1. Prioritize the bundled sidecar binary
    if let Some(sidecar_path) = get_sidecar_path(app) {
        if let Some(path_str) = sidecar_path.to_str() {
            executables.push(path_str.to_string());
        }
    }

    // 2. System fallbacks (dev convenience)
    executables.extend(vec![
        "/opt/homebrew/bin/llama-server".to_string(),
        "/usr/local/bin/llama-server".to_string(),
        "llama-server".to_string(),
    ]);

    let mut last_err = String::new();
    for exec in executables {
        let child = std::process::Command::new(&exec)
            .arg("-m")
            .arg(model_path)
            .arg("--mmproj")
            .arg(mmproj_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        match child {
            Ok(c) => {
                log::info!(
                    "[llama-server] Successfully spawned background llama-server via: {}",
                    exec
                );
                return Ok(c);
            }
            Err(e) => {
                last_err = format!("Failed to spawn executable '{}': {}", exec, e);
            }
        }
    }

    Err(format!(
        "Could not find or run `llama-server`! Ensure you have installed llama.cpp or placed the sidecar in src-tauri/binaries/. Details: {}",
        last_err
    ))
}

pub fn get_llama_port(app: &AppHandle) -> Option<u16> {
    app.state::<LlamaServerState>()
        .port
        .lock()
        .ok()
        .and_then(|g| *g)
}

/// Assures the background llama-server is active and healthy on a dynamically-chosen port.
/// If an external server is already running on the stored port, uses it instead and returns Guard with killed_on_drop=false.
pub fn ensure_llama_server_running(app: &AppHandle) -> Result<LlamaServerGuard<'_>, String> {
    let state = app.state::<LlamaServerState>();
    let mut lock = state.child.lock().map_err(|e| e.to_string())?;
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;

    // Check if child is still running
    let already_running = if let Some(ref mut child) = *lock {
        match child.try_wait() {
            Ok(None) => true, // Still running
            _ => {
                *lock = None;
                *port_lock = None;
                false
            }
        }
    } else {
        false
    };

    if already_running {
        return Ok(LlamaServerGuard {
            app,
            killed_on_drop: true,
        });
    }

    // Bind an ephemeral port now; the listener is dropped immediately so llama-server can reuse it.
    // There is a small TOCTOU window, but it's negligible for a local loopback process.
    let port = find_free_port()?;
    *port_lock = Some(port);

    // Try pinging the health endpoint in case llama-server was started externally on this port
    let health_url = format!("http://127.0.0.1:{}/health", port);
    let check_existing = ureq::get(&health_url)
        .timeout(Duration::from_millis(150))
        .call();

    if let Ok(resp) = check_existing {
        if resp.status() == 200 {
            log::info!(
                "[llama-server] Server already running externally on port {}",
                port
            );
            return Ok(LlamaServerGuard {
                app,
                killed_on_drop: false,
            });
        }
    }

    // Resolve Qwen2-Audio model paths via the unified model manager in embeddings.rs
    let model_path_buf =
        crate::embeddings::get_model_path("Qwen2-Audio-7B-Instruct.Q4_K_M.gguf", Some(app));
    let mmproj_path_buf =
        crate::embeddings::get_model_path("Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf", Some(app));

    if !model_path_buf.exists() || !mmproj_path_buf.exists() {
        return Err(format!(
            "Qwen2-Audio GGUF model files not found! Model path: {:?}, Project path: {:?}. Please run the download script `python3 tools/download_models.py` or place them in your models directory.",
            model_path_buf, mmproj_path_buf
        ));
    }

    let model_path = model_path_buf.to_string_lossy().into_owned();
    let mmproj_path = mmproj_path_buf.to_string_lossy().into_owned();

    log::info!(
        "[llama-server] Spawning background server on port {} with model: {}",
        port,
        model_path
    );

    let child = spawn_llama_server(app, &model_path, &mmproj_path, port)?;

    // Store child handle
    *lock = Some(child);

    // Wait for the server to load weights and report healthy
    log::info!("[llama-server] Waiting for background server to load weights...");
    let start_time = Instant::now();
    let max_duration = Duration::from_secs(120); // allow up to 120s slow CPU boot

    while start_time.elapsed() < max_duration {
        // Double check if child crashed early
        if let Some(ref mut c) = *lock {
            if let Ok(Some(status)) = c.try_wait() {
                return Err(format!(
                    "[llama-server] Server process exited prematurely with status: {}",
                    status
                ));
            }
        }

        let resp_res = ureq::get(&health_url)
            .timeout(Duration::from_millis(250))
            .call();

        match resp_res {
            Ok(resp) => {
                if resp.status() == 200 {
                    log::info!("[llama-server] Server is healthy and fully ready!");
                    return Ok(LlamaServerGuard {
                        app,
                        killed_on_drop: true,
                    });
                }
            }
            Err(_) => {
                // Not responding yet, sleep and retry
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    Err("Timeout waiting for background `llama-server` to become ready. Ensure your Mac has sufficient memory.".to_string())
}

/// Forcefully terminates the managed background llama-server process if active.
pub fn terminate_llama_server(app: &AppHandle) {
    if let Some(state) = app.try_state::<LlamaServerState>() {
        if let Ok(mut lock) = state.child.lock() {
            if let Some(mut child) = lock.take() {
                log::info!("[llama-server] Killing background llama-server child process...");
                let _ = child.kill();
                let _ = child.wait();
                log::info!("[llama-server] Child process reaped successfully.");
            }
        }
    }
}
