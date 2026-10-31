//! Jawaban Tanya Nuenly (F-22, US-1): perintah dan teks terpilih disusun `ContextAssembler`
//! (F-08), lalu jawaban Ollama (F-10) dialirkan ke jendela sesuai rencana `ResourceProfiler`.

use nuenly_context::{AssembleError, ContextAssembler, ModelPlan, Placement};
use nuenly_core::SensitiveText;
use nuenly_llm::ollama::OllamaModel;
use nuenly_llm::{ChatRequest, LlmError, LlmProvider, Message, OllamaProvider};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;
use tokio::sync::oneshot;

use crate::ask::AskState;
use crate::config::ConfigStore;
use crate::hardware::{HardwareInfo, HardwareState};
use crate::ollama;
use crate::status::AppStatus;

/// Batas panjang perintah, sama dengan `maxlength` kotak perintah di jendela.
const MAX_PROMPT_CHARS: usize = 2000;

/// Isi channel jawaban; bentuknya sama dengan `AnswerEvent` di `app/ui/src/windows/ask/tauri.ts`.
/// `End` dikirim setelah token terakhir, karena balasan command bisa tiba lebih dulu dari pesan
/// channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnswerEvent {
    Token { text: String },
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskReply {
    /// Teks terpilih terlalu panjang, jadi hanya bagian awalnya yang dipakai.
    pub context_truncated: bool,
}

/// Alasan gagal menjawab. Jendela menerjemahkannya menjadi pesan untuk pengguna; bentuknya sama
/// dengan `AskFailure` di `app/ui/src/windows/ask/tauri.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AskFailure {
    OllamaNotRunning,
    /// Belum ada model yang bisa dipakai untuk percakapan.
    NoModel,
    /// Model pilihan pengguna tidak ada di Ollama.
    ModelMissing,
    ModelTooLarge,
    /// Ollama gagal memuat model, biasanya karena memori GPU atau RAM sedang tidak cukup.
    ModelLoad,
    TooLong,
    Timeout,
    /// Jendela ditutup atau sesi baru dibuka sebelum jawaban selesai.
    Cancelled,
    Failed,
}

impl From<LlmError> for AskFailure {
    fn from(err: LlmError) -> Self {
        match err {
            LlmError::Unreachable => Self::OllamaNotRunning,
            LlmError::Timeout => Self::Timeout,
            LlmError::ModelNotFound | LlmError::RemoteModel => Self::ModelMissing,
            LlmError::ModelLoad => Self::ModelLoad,
            _ => Self::Failed,
        }
    }
}

/// Menjawab perintah dari jendela Tanya Nuenly dengan prompt sistem mode aktif. Token dikirim
/// lewat `channel` begitu tiba.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Setiap state Tauri adalah argumen tersendiri.
pub async fn ask_send(
    provider: State<'_, OllamaProvider>,
    hardware: State<'_, HardwareState>,
    store: State<'_, ConfigStore>,
    status: State<'_, AppStatus>,
    ask: State<'_, AskState>,
    prompt: String,
    use_context: bool,
    channel: Channel<AnswerEvent>,
) -> Result<AskReply, AskFailure> {
    let instruction = SensitiveText::new(prompt);
    let stopped = ask.begin_answer();
    let context = ask.context(use_context);
    if instruction.expose().trim().is_empty() {
        return Err(AskFailure::Failed);
    }
    if instruction.char_count() > MAX_PROMPT_CHARS {
        return Err(AskFailure::TooLong);
    }

    let models = ollama::local_models(&provider).await?;
    let preferred = store.get().model;
    let plan = choose(&hardware.get().await, &models, preferred.as_deref())?;
    let prompt = ContextAssembler::new(status.ask_prompt())
        .assemble(plan.num_ctx, &instruction, context.as_ref())
        .map_err(|err| match err {
            AssembleError::TooLong => AskFailure::TooLong,
            _ => AskFailure::Failed,
        })?;
    let context_truncated = prompt.context_truncated;
    let mut request = ChatRequest::new(
        plan.model,
        plan.num_ctx,
        vec![Message::system(prompt.system), Message::user(prompt.user)],
    );
    request.keep_alive = Some(plan.keep_alive);

    let mut on_token = |token: &str| {
        let _ = channel.send(AnswerEvent::Token {
            text: token.to_owned(),
        });
    };
    stream(&*provider, &request, &mut on_token, stopped).await?;
    let _ = channel.send(AnswerEvent::End);
    Ok(AskReply { context_truncated })
}

/// Model untuk menjawab: pilihan pengguna, atau pilihan `ResourceProfiler` bila otomatis.
fn choose(
    hardware: &HardwareInfo,
    models: &[OllamaModel],
    preferred: Option<&str>,
) -> Result<ModelPlan, AskFailure> {
    let Some(plan) = ollama::plan(hardware, models, preferred) else {
        return Err(match preferred {
            Some(_) if !models.is_empty() => AskFailure::ModelMissing,
            _ => AskFailure::NoModel,
        });
    };
    // Model yang melebihi memori bisa membuat seluruh PC tersendat.
    if plan.placement == Placement::TooLarge {
        return Err(AskFailure::ModelTooLarge);
    }
    Ok(plan)
}

/// Mengalirkan jawaban `request` ke `on_token` sampai selesai, atau sampai `stopped` selesai.
/// Berhenti berarti future permintaannya di-drop, jadi koneksi ke Ollama ikut ditutup dan model
/// berhenti menulis.
async fn stream(
    provider: &dyn LlmProvider,
    request: &ChatRequest,
    on_token: &mut (dyn FnMut(&str) + Send),
    stopped: oneshot::Receiver<()>,
) -> Result<(), AskFailure> {
    tokio::select! {
        biased;
        _ = stopped => Err(AskFailure::Cancelled),
        result = provider.chat(request, on_token) => result.map(|_| ()).map_err(AskFailure::from),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use nuenly_context::{ContextAssembler, TAG_PREFIX};
    use nuenly_core::{AppIdentity, CaptureKind, ContextSnapshot, ProcessName, SensitiveText};
    use nuenly_llm::ollama::OllamaModel;
    use nuenly_llm::{BoxFuture, ChatRequest, LlmError, LlmProvider, Message, OutputSchema, Usage};
    use nuenly_modes::ModeManager;
    use tokio::sync::oneshot;

    use super::{AnswerEvent, AskFailure, choose, stream};
    use crate::hardware::HardwareInfo;

    const GIB: u64 = 1024 * 1024 * 1024;

    // Ollama dan model sungguhan di PC pengembang: memuat model ke memori, jadi jalankan hanya
    // saat PC sedang longgar. Teksnya sintetis.
    // Jalankan manual: cargo test -p nuenly --bin nuenly answers_with_ollama -- --ignored --nocapture
    #[test]
    #[ignore = "butuh Ollama dan model; memuat model ke memori"]
    fn answers_with_ollama() {
        let provider = nuenly_llm::OllamaProvider::new().unwrap();
        let hardware = crate::hardware::detect();
        let context = ContextSnapshot {
            app: AppIdentity {
                process: ProcessName::new("notepad.exe").unwrap(),
                display_name: "Notepad".to_owned(),
            },
            window_title: SensitiveText::default(),
            kind: CaptureKind::SelectedText,
            field: None,
            text: SensitiveText::new(
                "jadi gini, rapat minggu depan digeser ke kamis aja soalnya ruangan dipake \
                 dan materi dari tim keuangan belum lengkap. tolong kabarin bisa ga",
            ),
        };
        let (_stop, stopped) = oneshot::channel();

        tauri::async_runtime::block_on(async {
            let models = crate::ollama::local_models(&provider).await.unwrap();
            let plan = choose(&hardware, &models, None).unwrap();
            let umum = ModeManager::official().unwrap();
            let prompt = ContextAssembler::new(umum.active().ask_prompt())
                .assemble(
                    plan.num_ctx,
                    &SensitiveText::new("Rapikan jadi formal dan singkat"),
                    Some(&context),
                )
                .unwrap();
            let mut request = ChatRequest::new(
                plan.model.clone(),
                plan.num_ctx,
                vec![Message::system(prompt.system), Message::user(prompt.user)],
            );
            request.keep_alive = Some(plan.keep_alive);

            let started = Instant::now();
            let mut first_token = None;
            let mut on_token = |token: &str| {
                first_token.get_or_insert_with(|| started.elapsed());
                print!("{token}");
            };
            stream(&provider, &request, &mut on_token, stopped)
                .await
                .unwrap();
            println!(
                "\n\nmodel {} · num_ctx {} · token pertama {first_token:?} · total {:?}",
                plan.model,
                plan.num_ctx,
                started.elapsed()
            );
        });
    }

    /// Menjawab dengan token yang sudah ditentukan.
    struct Scripted(&'static [&'static str]);

    /// Tidak pernah selesai; menandai `dropped` saat permintaannya dibatalkan.
    struct Endless(Arc<AtomicBool>);

    struct SetOnDrop(Arc<AtomicBool>);

    impl Drop for SetOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    impl LlmProvider for Scripted {
        fn chat<'a>(
            &'a self,
            _request: &'a ChatRequest,
            on_token: &'a mut (dyn FnMut(&str) + Send),
        ) -> BoxFuture<'a, Result<Usage, LlmError>> {
            Box::pin(async move {
                for token in self.0 {
                    on_token(token);
                }
                Ok(Usage::default())
            })
        }

        fn complete_json<'a>(
            &'a self,
            _request: &'a ChatRequest,
            _schema: &'a OutputSchema,
        ) -> BoxFuture<'a, Result<SensitiveText, LlmError>> {
            Box::pin(async { Err(LlmError::Protocol) })
        }
    }

    impl LlmProvider for Endless {
        fn chat<'a>(
            &'a self,
            _request: &'a ChatRequest,
            _on_token: &'a mut (dyn FnMut(&str) + Send),
        ) -> BoxFuture<'a, Result<Usage, LlmError>> {
            let dropped = SetOnDrop(Arc::clone(&self.0));
            Box::pin(async move {
                let _dropped = dropped;
                std::future::pending::<()>().await;
                Ok(Usage::default())
            })
        }

        fn complete_json<'a>(
            &'a self,
            _request: &'a ChatRequest,
            _schema: &'a OutputSchema,
        ) -> BoxFuture<'a, Result<SensitiveText, LlmError>> {
            Box::pin(async { Err(LlmError::Protocol) })
        }
    }

    fn request() -> ChatRequest {
        ChatRequest::new("model-sintetis", NonZeroU32::MIN, Vec::new())
    }

    fn model(name: &str, size: u64) -> OllamaModel {
        OllamaModel {
            name: name.to_owned(),
            size,
            remote: false,
        }
    }

    /// PC pengembang: RAM 7,7 GB, VRAM 6 GB.
    fn hardware() -> HardwareInfo {
        HardwareInfo {
            ram_bytes: 8_279_416_832,
            vram_bytes: 6 * GIB,
            gpu: None,
        }
    }

    #[test]
    fn tokens_are_forwarded_in_order() {
        let (_stop, stopped) = oneshot::channel();
        let mut tokens = Vec::new();
        let mut on_token = |token: &str| tokens.push(token.to_owned());

        let result = tauri::async_runtime::block_on(stream(
            &Scripted(&["Halo ", "semua"]),
            &request(),
            &mut on_token,
            stopped,
        ));

        assert_eq!(result, Ok(()));
        assert_eq!(tokens, ["Halo ", "semua"]);
    }

    #[test]
    fn closing_the_window_cancels_the_request() {
        let dropped = Arc::new(AtomicBool::new(false));
        let (stop, stopped) = oneshot::channel::<()>();
        // Jendela ditutup saat model sedang menulis.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            drop(stop);
        });
        let mut on_token = |_: &str| {};

        let result = tauri::async_runtime::block_on(stream(
            &Endless(Arc::clone(&dropped)),
            &request(),
            &mut on_token,
            stopped,
        ));

        assert_eq!(result, Err(AskFailure::Cancelled));
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn model_choice_explains_what_is_missing() {
        let hardware = hardware();
        assert_eq!(choose(&hardware, &[], None), Err(AskFailure::NoModel));
        assert_eq!(
            choose(
                &hardware,
                &[model("gemma3:4b", 3 * GIB)],
                Some("tidak-ada:1b")
            ),
            Err(AskFailure::ModelMissing)
        );
        assert_eq!(
            choose(&hardware, &[model("raksasa:70b", 40 * GIB)], None),
            Err(AskFailure::ModelTooLarge)
        );
        let plan = choose(&hardware, &[model("gemma3:4b", 3 * GIB)], None).unwrap();
        assert_eq!(plan.model, "gemma3:4b");
    }

    #[test]
    fn ollama_errors_become_failures_without_details() {
        assert_eq!(
            AskFailure::from(LlmError::Unreachable),
            AskFailure::OllamaNotRunning
        );
        assert_eq!(
            AskFailure::from(LlmError::RemoteModel),
            AskFailure::ModelMissing
        );
        assert_eq!(AskFailure::from(LlmError::ModelLoad), AskFailure::ModelLoad);
        assert_eq!(AskFailure::from(LlmError::Status(500)), AskFailure::Failed);
        assert_eq!(
            serde_json::to_value(AskFailure::OllamaNotRunning).unwrap(),
            "ollamaNotRunning"
        );
    }

    #[test]
    fn channel_events_have_a_kind() {
        let token = AnswerEvent::Token {
            text: "Halo".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(token).unwrap(),
            serde_json::json!({ "kind": "token", "text": "Halo" })
        );
        assert_eq!(
            serde_json::to_value(AnswerEvent::End).unwrap(),
            serde_json::json!({ "kind": "end" })
        );
    }

    #[test]
    fn every_mode_prompt_names_the_tag_used_by_the_assembler() {
        let tag = format!("\"{TAG_PREFIX}\"");
        for pack in ModeManager::official().unwrap().packs() {
            assert!(pack.ask_prompt().contains(&tag), "{}", pack.id().as_str());
        }
    }
}
