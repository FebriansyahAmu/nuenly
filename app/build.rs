fn main() {
    // Command aplikasi didaftarkan di manifest agar hanya bisa dipanggil jendela yang diberi izin
    // `allow-<command>` di `capabilities/`.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_quit",
        "clipboard_write_text",
        "onboarding_accept",
        "onboarding_finish",
        "onboarding_ollama",
        "onboarding_open_download",
        "open_settings",
        "status_get",
        "status_pause",
        "status_resume",
        "status_set_mode",
        "toast_layout",
    ]);
    if let Err(error) =
        tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
    {
        panic!("tauri-build gagal: {error:#}");
    }
}
