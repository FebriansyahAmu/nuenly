//! Antarmuka penyedia model dan output terstruktur.

use std::future::Future;
use std::pin::Pin;

use nuenly_core::SensitiveText;
use serde::de::DeserializeOwned;

use crate::{ChatRequest, LlmError, OutputSchema, Usage};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Berapa kali output terstruktur diminta sebelum menyerah: sekali, lalu ulang sekali.
const STRUCTURED_ATTEMPTS: usize = 2;

/// Penyedia model. Object-safe agar aplikasi bisa memilih penyedia saat berjalan.
pub trait LlmProvider: Send + Sync {
    /// Jawaban streaming. Setiap potongan teks diteruskan ke `on_token` begitu tiba;
    /// membuang (drop) future-nya membatalkan permintaan.
    fn chat<'a>(
        &'a self,
        request: &'a ChatRequest,
        on_token: &'a mut (dyn FnMut(&str) + Send),
    ) -> BoxFuture<'a, Result<Usage, LlmError>>;

    /// Satu jawaban utuh yang dibatasi `schema` di sisi server. Belum divalidasi; pakai
    /// [`structured`].
    fn complete_json<'a>(
        &'a self,
        request: &'a ChatRequest,
        schema: &'a OutputSchema,
    ) -> BoxFuture<'a, Result<SensitiveText, LlmError>>;
}

/// Output terstruktur yang lolos `schema`, lalu dibaca sebagai `T`.
///
/// # Errors
///
/// [`LlmError::SchemaMismatch`] bila jawaban tetap tidak lolos setelah diulang sekali.
/// Kegagalan lain (server mati, model tidak ada) langsung dikembalikan tanpa diulang.
pub async fn structured<T: DeserializeOwned>(
    provider: &dyn LlmProvider,
    request: &ChatRequest,
    schema: &OutputSchema,
) -> Result<T, LlmError> {
    for _ in 0..STRUCTURED_ATTEMPTS {
        let answer = provider.complete_json(request, schema).await?;
        if let Some(output) = parse(&answer, schema) {
            return Ok(output);
        }
    }
    Err(LlmError::SchemaMismatch)
}

fn parse<T: DeserializeOwned>(answer: &SensitiveText, schema: &OutputSchema) -> Option<T> {
    let value: serde_json::Value = serde_json::from_str(answer.expose()).ok()?;
    if !schema.accepts(&value) {
        return None;
    }
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::num::NonZeroU32;
    use std::sync::Mutex;

    use nuenly_core::SensitiveText;
    use serde::Deserialize;
    use serde_json::json;

    use super::{BoxFuture, LlmProvider, structured};
    use crate::{ChatRequest, LlmError, OutputSchema, Usage};

    /// Penyedia palsu yang menjawab dari daftar jawaban berurutan.
    struct Scripted {
        answers: Mutex<VecDeque<Result<&'static str, LlmError>>>,
        calls: Mutex<usize>,
    }

    impl Scripted {
        fn new(answers: Vec<Result<&'static str, LlmError>>) -> Self {
            Self {
                answers: Mutex::new(answers.into()),
                calls: Mutex::new(0),
            }
        }

        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl LlmProvider for Scripted {
        fn chat<'a>(
            &'a self,
            _request: &'a ChatRequest,
            _on_token: &'a mut (dyn FnMut(&str) + Send),
        ) -> BoxFuture<'a, Result<Usage, LlmError>> {
            Box::pin(async { Ok(Usage::default()) })
        }

        fn complete_json<'a>(
            &'a self,
            _request: &'a ChatRequest,
            _schema: &'a OutputSchema,
        ) -> BoxFuture<'a, Result<SensitiveText, LlmError>> {
            *self.calls.lock().unwrap() += 1;
            let answer = self.answers.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { answer.map(SensitiveText::new) })
        }
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Suggestion {
        title: String,
    }

    fn schema() -> OutputSchema {
        OutputSchema::new(json!({
            "type": "object",
            "properties": { "title": { "type": "string" } },
            "required": ["title"],
            "additionalProperties": false
        }))
        .unwrap()
    }

    fn request() -> ChatRequest {
        ChatRequest::new("gemma3:4b", NonZeroU32::MIN, Vec::new())
    }

    #[tokio::test]
    async fn returns_the_first_valid_answer() {
        let provider = Scripted::new(vec![Ok(r#"{"title":"Ringkas"}"#)]);
        let output: Suggestion = structured(&provider, &request(), &schema()).await.unwrap();
        assert_eq!(output.title, "Ringkas");
        assert_eq!(provider.calls(), 1);
    }

    #[tokio::test]
    async fn retries_once_after_an_invalid_answer() {
        let provider = Scripted::new(vec![Ok("bukan json"), Ok(r#"{"title":"Ringkas"}"#)]);
        let output: Suggestion = structured(&provider, &request(), &schema()).await.unwrap();
        assert_eq!(output.title, "Ringkas");
        assert_eq!(provider.calls(), 2);
    }

    #[tokio::test]
    async fn gives_up_after_the_retry() {
        let provider = Scripted::new(vec![
            Ok(r#"{"title":"x","url":"https://contoh.invalid"}"#),
            Ok(r#"{"judul":"x"}"#),
            Ok(r#"{"title":"tidak sempat diminta"}"#),
        ]);
        let result = structured::<Suggestion>(&provider, &request(), &schema()).await;
        assert_eq!(result.unwrap_err(), LlmError::SchemaMismatch);
        assert_eq!(provider.calls(), 2);
    }

    #[tokio::test]
    async fn does_not_retry_transport_failures() {
        let provider = Scripted::new(vec![Err(LlmError::Unreachable), Ok(r#"{"title":"x"}"#)]);
        let result = structured::<Suggestion>(&provider, &request(), &schema()).await;
        assert_eq!(result.unwrap_err(), LlmError::Unreachable);
        assert_eq!(provider.calls(), 1);
    }
}
