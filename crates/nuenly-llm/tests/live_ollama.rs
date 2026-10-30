//! Uji terhadap Ollama sungguhan di PC pengembang. Tidak jalan di CI.
//! Jalankan manual: `cargo test -p nuenly-llm --test live_ollama -- --ignored --nocapture`

use nuenly_llm::OllamaProvider;

#[tokio::test]
#[ignore = "butuh Ollama yang berjalan di 127.0.0.1:11434"]
async fn reads_version_and_models_from_a_running_ollama() {
    let provider = OllamaProvider::new().unwrap();

    let version = provider.version().await.unwrap();
    let models = provider.models().await.unwrap();

    assert_ne!(version, "");
    println!("Ollama {version}, {} model:", models.len());
    for model in models {
        let place = if model.remote { "cloud" } else { "lokal" };
        println!("  {} ({} MB, {place})", model.name, model.size / 1_000_000);
    }
}
