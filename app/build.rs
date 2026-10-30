fn main() {
    // Command aplikasi didaftarkan di manifest agar hanya bisa dipanggil jendela yang diberi izin
    // `allow-<command>` di `capabilities/`.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_quit",
        "ask_send",
        "clipboard_write_text",
        "ollama_open_download",
        "ollama_status",
        "onboarding_accept",
        "onboarding_finish",
        "open_settings",
        "settings_get",
        "settings_set_model",
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
