fn main() {
    if std::env::var("ACOUSTID_CLIENT_KEY").is_err() {
        println!("cargo:warning=ACOUSTID_CLIENT_KEY environment variable is not set! The build will fall back to the public testing API key.");
    }
    tauri_build::build()
}
