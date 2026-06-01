use deep_cuts_lib::commands::download::{download_models, DownloadState, DownloadProgressEvent};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::Manager;
use tauri::Listener;

// Custom local TCP mock server that can support Accept-Ranges, 206 Partial Content, and dropped streams.
fn start_mock_resumable_server(
    drop_first_n_bytes: Option<usize>,
    corrupt_data: bool,
) -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_signal_clone = shutdown_signal.clone();

    let server_handle = thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        
        let drop_triggered = Arc::new(AtomicBool::new(false));

        while !shutdown_signal_clone.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0; 2048];
                    
                    // Use standard read timeout instead of full non-blocking mode to ensure write reliability
                    stream.set_read_timeout(Some(Duration::from_millis(500))).ok();
                    stream.set_write_timeout(Some(Duration::from_millis(500))).ok();
                    
                    let bytes_read = match stream.read(&mut buffer) {
                        Ok(n) => n,
                        Err(_) => 0,
                    };
                    
                    let req_str = String::from_utf8_lossy(&buffer[..bytes_read]);
                    println!("[mock_server] Received request: {}", req_str);
                    let is_range_req = req_str.contains("Range: bytes=");
                    let range_start = if is_range_req {
                        if let Some(range_line) = req_str.lines().find(|l| l.starts_with("Range: bytes=")) {
                            let bytes_part = range_line.trim_start_matches("Range: bytes=");
                            let start_str = bytes_part.split('-').next().unwrap();
                            start_str.parse::<usize>().unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    };

                    // Let's use a 16-byte payload for predictable testing.
                    // Expected SHA256 of b"0123456789abcdef" (16 bytes):
                    // sha256("0123456789abcdef") = "6238bc32512f46210f2b2db8ebbf577b311746f33cfcdbb13f3e8fcdcf6574f3"
                    let mut mock_content = b"0123456789abcdef".to_vec();
                    if corrupt_data {
                        // Corrupt the data deliberately by modifying a byte
                        mock_content[0] = b'X';
                    }
                    let total_len = mock_content.len();

                    if is_range_req {
                        let slice = &mock_content[range_start..];
                        let response = format!(
                            "HTTP/1.1 206 Partial Content\r\n\
                             Accept-Ranges: bytes\r\n\
                             Content-Length: {}\r\n\
                             Content-Range: bytes {}-{}/{}\r\n\
                             Connection: close\r\n\r\n",
                            slice.len(), range_start, total_len - 1, total_len
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(slice);
                        let _ = stream.flush();
                        let _ = stream.shutdown(std::net::Shutdown::Write);
                    } else {
                        // If we are simulating a dropped stream on the first download try
                        if let Some(drop_limit) = drop_first_n_bytes {
                            if !drop_triggered.load(Ordering::SeqCst) {
                                drop_triggered.store(true, Ordering::SeqCst);
                                let slice = &mock_content[0..drop_limit];
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                     Accept-Ranges: bytes\r\n\
                                     Content-Length: {}\r\n\
                                     Connection: close\r\n\r\n",
                                     total_len
                                );
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.write_all(slice);
                                let _ = stream.flush();
                                let _ = stream.shutdown(std::net::Shutdown::Write);
                                // Purposely closing stream to drop connection midway
                                continue;
                            }
                        }

                        // Otherwise stream full content
                        let response = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Accept-Ranges: bytes\r\n\
                             Content-Length: {}\r\n\
                             Connection: close\r\n\r\n",
                            total_len
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(&mock_content);
                        let _ = stream.flush();
                        let _ = stream.shutdown(std::net::Shutdown::Write);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    (format!("http://127.0.0.1:{}", port), shutdown_signal, server_handle)
}

fn create_test_manifest() -> String {
    // 16 bytes size schema, matches the sha256 checksum of b"0123456789abcdef"
    r#"{
        "manifest_version": 1,
        "min_app_version": "0.1.0",
        "update_notice": null,
        "models": {
            "test_model": {
                "label": "Test Mock Model Group",
                "files": [
                    {
                        "filename": "mock_model.bin",
                        "url": "http://example.com/mock_model.bin",
                        "sha256": "9f9f5111f7b27a781f1f1ddde5ebc2dd2b796bfc7365c9c28b548e564176929f",
                        "size_bytes": 16
                    }
                ]
            }
        }
    }"#.to_string()
}

#[test]
fn test_resumable_downloader_integration() {
    let app = tauri::test::mock_app();
    
    let models_dir = std::env::temp_dir().join(format!("deep_cuts_test_resumable_{}", std::process::id()));
    let _ = fs::remove_dir_all(&models_dir); // clean start
    fs::create_dir_all(&models_dir).unwrap();

    app.manage(deep_cuts_lib::commands::download::ModelDirectoryOverride(models_dir.clone()));
    app.manage(DownloadState::default());

    let target_file = models_dir.join("mock_model.bin");
    let part_file = models_dir.join("mock_model.bin.part");

    // ── Scenario A: Interrupted Download (Resume Check) ──
    // Server drops stream after sending first 6 bytes
    let (url_base, server_shutdown, server_thread) = start_mock_resumable_server(Some(6), false);

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let progress_events_clone = progress_events.clone();
    app.listen("model-download-progress", move |event| {
        if let Ok(progress) = serde_json::from_str::<DownloadProgressEvent>(event.payload()) {
            progress_events_clone.lock().unwrap().push(progress);
        }
    });

    let complete_signal = Arc::new(AtomicBool::new(false));
    let error_signal = Arc::new(AtomicBool::new(false));

    let complete_signal_clone = complete_signal.clone();
    app.listen("model-download-all-complete", move |_| {
        complete_signal_clone.store(true, Ordering::SeqCst);
    });

    let error_signal_clone = error_signal.clone();
    app.listen("model-download-all-error", move |event| {
        println!("[download_tests] Download error received: {:?}", event.payload());
        error_signal_clone.store(true, Ordering::SeqCst);
    });

    let download_state = app.state::<DownloadState>();

    // Trigger first download (which will drop after 6 bytes)
    let custom_manifest = Some(create_test_manifest());
    let res = download_models(
        app.handle().clone(),
        download_state.clone(),
        vec!["test_model".to_string()],
        Some(url_base.clone()),
        custom_manifest.clone(),
    );
    assert!(res.is_ok());

    // Wait for worker to finish (fails with network/EOF error)
    for _ in 0..100 {
        if error_signal.load(Ordering::SeqCst) || complete_signal.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Verify first run dropped and left 6 bytes inside part file
    assert!(part_file.exists());
    let part_meta = fs::metadata(&part_file).unwrap();
    assert_eq!(part_meta.len(), 6);
    assert!(!target_file.exists());
    assert!(error_signal.load(Ordering::SeqCst));

    // Reset signals for the resume run
    error_signal.store(false, Ordering::SeqCst);
    complete_signal.store(false, Ordering::SeqCst);

    println!("[download_tests] is_running before Scenario B: {}", download_state.is_running.load(Ordering::SeqCst));
    println!("[download_tests] cancel_flag before Scenario B: {}", download_state.cancel_flag.load(Ordering::SeqCst));

    // ── Scenario B: Resuming the Download ──
    // Now trigger download again. It should read offset 6, send Range request, append rest (10 bytes), verify sha256 and complete!
    let res = download_models(
        app.handle().clone(),
        download_state.clone(),
        vec!["test_model".to_string()],
        Some(url_base.clone()),
        custom_manifest.clone(),
    );
    assert!(res.is_ok());

    // Wait for worker to complete successfully
    for _ in 0..100 {
        if error_signal.load(Ordering::SeqCst) || complete_signal.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    println!("[download_tests] target_file exists: {}", target_file.exists());
    println!("[download_tests] part_file exists: {}", part_file.exists());
    if part_file.exists() {
        if let Ok(meta) = fs::metadata(&part_file) {
            println!("[download_tests] part_file size: {}", meta.len());
        }
    }

    // Verify it completed successfully and renamed to mock_model.bin
    assert!(complete_signal.load(Ordering::SeqCst));
    assert!(!error_signal.load(Ordering::SeqCst));
    assert!(!part_file.exists());
    assert!(target_file.exists());

    // Check final content matches
    let final_content = fs::read_to_string(&target_file).unwrap();
    assert_eq!(final_content, "0123456789abcdef");

    // Shutdown mock server
    server_shutdown.store(true, Ordering::SeqCst);
    let _ = server_thread.join();
    let _ = fs::remove_dir_all(&models_dir);
}

#[test]
fn test_download_checksum_corruption_handling() {
    let app = tauri::test::mock_app();
    
    let models_dir = std::env::temp_dir().join(format!("deep_cuts_test_corruption_{}", std::process::id()));
    let _ = fs::remove_dir_all(&models_dir); // clean start
    fs::create_dir_all(&models_dir).unwrap();

    app.manage(deep_cuts_lib::commands::download::ModelDirectoryOverride(models_dir.clone()));
    app.manage(DownloadState::default());

    let target_file = models_dir.join("mock_model.bin");
    let part_file = models_dir.join("mock_model.bin.part");

    // Boot mock server that deliberately sends corrupted bytes
    let (url_base, server_shutdown, server_thread) = start_mock_resumable_server(None, true);

    let error_signal = Arc::new(AtomicBool::new(false));
    let error_signal_clone = error_signal.clone();
    app.listen("model-download-all-error", move |_| {
        error_signal_clone.store(true, Ordering::SeqCst);
    });

    let download_state = app.state::<DownloadState>();
    let custom_manifest = Some(create_test_manifest());

    let res = download_models(
        app.handle().clone(),
        download_state.clone(),
        vec!["test_model".to_string()],
        Some(url_base),
        custom_manifest,
    );
    assert!(res.is_ok());

    // Wait for verification failure
    for _ in 0..100 {
        if error_signal.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Corrupted part file must be deleted and final file should not exist
    assert!(error_signal.load(Ordering::SeqCst));
    assert!(!part_file.exists());
    assert!(!target_file.exists());

    server_shutdown.store(true, Ordering::SeqCst);
    let _ = server_thread.join();
    let _ = fs::remove_dir_all(&models_dir);
}
