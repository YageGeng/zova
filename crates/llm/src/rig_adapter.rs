use std::sync::Arc;

use futures::StreamExt;
use rig::completion::{CompletionModel, Message as RigMessage};
use rig::http_client::{self, HttpClientExt, NoBody};
use rig::prelude::CompletionClient;
use rig::providers::openai;
use rig::streaming::StreamedAssistantContent;
use snafu::{ResultExt, ensure};
use tokio::sync::{mpsc, oneshot};

use super::model::{
    DEFAULT_OPENAI_MODEL, Model, ModelCache, ModelCatalog, default_openai_models, get_model_cache,
};
use super::provider::{
    BoxFuture, CompletionsFailedSnafu, EmptyMessageSetSnafu, HttpClientSnafu, LlmProvider,
    MissingApiKeySnafu, ModelFetchStatusSnafu, ModelPayloadParseSnafu, ProviderConfig,
    ProviderError, ProviderResult, ProviderStreamHandle, ProviderWorker, Role, StreamEventMapped,
    StreamEventPayload, StreamRequest, StreamTarget, make_event_stream,
};

pub const RIG_OPENAI_PROVIDER_ID: &str = "openai";

type RigStreamingResponse = rig::streaming::StreamingCompletionResponse<
    rig::providers::openai::responses_api::streaming::StreamingCompletionResponse,
>;

pub struct RigProviderAdapter {
    config: ProviderConfig,
    fallback_models: Vec<Model>,
    model_cache: Arc<ModelCache>,
}

impl RigProviderAdapter {
    pub fn new(config: ProviderConfig) -> ProviderResult<Self> {
        ensure!(
            !config.api_key.is_empty(),
            MissingApiKeySnafu {
                stage: "rig-adapter-new",
                provider_id: config.provider_id.clone(),
            }
        );

        Ok(Self {
            config,
            fallback_models: default_openai_models(),
            model_cache: get_model_cache(),
        })
    }

    fn build_client(config: &ProviderConfig) -> ProviderResult<openai::Client> {
        let mut builder = openai::Client::builder().api_key(config.api_key.as_str());
        if !config.endpoint.is_empty() {
            builder = builder.base_url(config.endpoint.as_str());
        }
        builder.build().context(HttpClientSnafu {
            stage: "build-client",
        })
    }

    async fn fetch_models_from_provider(&self) -> ProviderResult<Vec<Model>> {
        let client = Self::build_client(&self.config)?;
        let request = client
            .get("/models")
            .context(HttpClientSnafu {
                stage: "build-model-request",
            })?
            .body(NoBody)
            .map_err(|source| ProviderError::BuildHttpRequestBody {
                stage: "build-model-request-body",
                message: source.to_string(),
            })?;

        let response = client.send(request).await.context(HttpClientSnafu {
            stage: "send-model-request",
        })?;
        let status = response.status();
        let payload = http_client::text(response).await.context(HttpClientSnafu {
            stage: "read-model-response",
        })?;

        if !status.is_success() {
            return ModelFetchStatusSnafu {
                stage: "model-http-status",
                status: status.as_u16(),
                body: payload,
            }
            .fail();
        }

        let model_ids = Self::extract_model_ids(&payload);
        if model_ids.is_empty() {
            return ModelPayloadParseSnafu {
                stage: "parse-model-response",
                details: "no model identifiers found in provider response".to_string(),
            }
            .fail();
        }

        Ok(model_ids.into_iter().map(Model::from_id).collect())
    }

    fn extract_model_ids(payload: &str) -> Vec<String> {
        let mut ids = Vec::new();
        let mut cursor = payload;
        let needle = "\"id\":\"";

        // Keep the parser lightweight for MVP: extract every OpenAI-style `id` field.
        while let Some(start) = cursor.find(needle) {
            let tail = &cursor[start + needle.len()..];
            let Some(end) = tail.find('"') else {
                break;
            };

            let candidate = tail[..end].trim();
            if !candidate.is_empty() {
                ids.push(candidate.to_string());
            }
            cursor = &tail[end + 1..];
        }

        ids.sort();
        ids.dedup();
        ids
    }

    fn to_rig_message(message: &super::provider::ProviderMessage) -> Option<RigMessage> {
        match message.role {
            Role::System => None,
            Role::User => Some(RigMessage::user(message.content.clone())),
            Role::Assistant => Some(RigMessage::assistant(message.content.clone())),
        }
    }

    fn merged_preamble(request: &StreamRequest) -> Option<String> {
        let mut preamble_parts = Vec::new();

        if let Some(preamble) = &request.preamble
            && !preamble.trim().is_empty()
        {
            preamble_parts.push(preamble.clone());
        }

        // Rig exposes a single preamble field, so system-role messages are folded into it
        // to preserve caller intent while still sending user/assistant turns as chat messages.
        for message in &request.messages {
            if matches!(message.role, Role::System) && !message.content.trim().is_empty() {
                preamble_parts.push(message.content.clone());
            }
        }

        if preamble_parts.is_empty() {
            None
        } else {
            Some(preamble_parts.join("\n\n"))
        }
    }

    async fn open_stream(
        config: &ProviderConfig,
        request: &StreamRequest,
    ) -> ProviderResult<RigStreamingResponse> {
        let client = Self::build_client(config)?;
        let model = client.completion_model(request.model_id.clone());

        let mut messages = request
            .messages
            .iter()
            .filter_map(Self::to_rig_message)
            .collect::<Vec<_>>();

        let total_message_count = request.messages.len();
        let system_message_count = request
            .messages
            .iter()
            .filter(|message| matches!(message.role, Role::System))
            .count();

        if messages.is_empty() {
            tracing::warn!(
                target = ?request.target,
                model_id = %request.model_id,
                total_message_count,
                system_message_count,
                "cannot open stream because no user/assistant messages remain after filtering"
            );
            return EmptyMessageSetSnafu {
                stage: "open-stream-filter-messages",
                target: request.target,
            }
            .fail();
        }

        let Some(prompt) = messages.pop() else {
            tracing::error!(
                target = ?request.target,
                model_id = %request.model_id,
                "message list became empty before prompt extraction"
            );
            return EmptyMessageSetSnafu {
                stage: "open-stream-pop-prompt",
                target: request.target,
            }
            .fail();
        };
        let mut builder = model.completion_request(prompt).messages(messages);

        if let Some(preamble) = Self::merged_preamble(request) {
            builder = builder.preamble(preamble);
        }

        if let Some(temperature) = request.temperature {
            builder = builder.temperature(temperature);
        }

        if let Some(max_tokens) = request.max_tokens {
            builder = builder.max_tokens(max_tokens);
        }

        builder.stream().await.context(CompletionsFailedSnafu {
            stage: "open-stream",
        })
    }

    fn emit_error_event(
        event_tx: &mpsc::UnboundedSender<StreamEventMapped>,
        target: StreamTarget,
        error: ProviderError,
    ) {
        let _ = event_tx.send(StreamEventMapped {
            target,
            payload: StreamEventPayload::Error(error.to_string()),
        });
    }

    fn map_stream_item<R>(
        target: StreamTarget,
        item: StreamedAssistantContent<R>,
    ) -> Option<StreamEventMapped>
    where
        R: Clone + Unpin,
    {
        let payload = match item {
            StreamedAssistantContent::Text(text) => StreamEventPayload::Delta(text.text),
            StreamedAssistantContent::Reasoning(reasoning) => {
                // Rig can split reasoning into multiple fragments; flatten before forwarding.
                let text = reasoning.reasoning.join("");
                if text.is_empty() {
                    return None;
                }
                StreamEventPayload::ReasoningDelta(text)
            }
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                if reasoning.is_empty() {
                    return None;
                }
                StreamEventPayload::ReasoningDelta(reasoning)
            }
            StreamedAssistantContent::ToolCall { .. }
            | StreamedAssistantContent::ToolCallDelta { .. }
            | StreamedAssistantContent::Final(_) => return None,
        };

        Some(StreamEventMapped { target, payload })
    }

    async fn run_stream_worker(
        config: ProviderConfig,
        request: StreamRequest,
        event_tx: mpsc::UnboundedSender<StreamEventMapped>,
        mut cancel_rx: oneshot::Receiver<()>,
    ) {
        let target = request.target;
        let mut stream = match Self::open_stream(&config, &request).await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::error!(
                    target = ?target,
                    provider_id = %config.provider_id,
                    model_id = %request.model_id,
                    error = %error,
                    "failed to open provider stream"
                );
                Self::emit_error_event(&event_tx, target, error);
                return;
            }
        };

        let mut cancelled = false;
        let mut stream_failed = false;

        loop {
            tokio::select! {
                _ = &mut cancel_rx => {
                    cancelled = true;
                    // Cancel the upstream Rig stream so provider IO stops promptly.
                    tracing::debug!(target = ?target, "provider stream cancelled");
                    stream.cancel();
                    break;
                }
                next_item = stream.next() => {
                    match next_item {
                        Some(Ok(item)) => {
                            if let Some(mapped) = Self::map_stream_item(target, item)
                                && event_tx.send(mapped).is_err()
                            {
                                return;
                            }
                        }
                        Some(Err(source)) => {
                            stream_failed = true;
                            tracing::warn!(
                                target = ?target,
                                error = %source,
                                "provider stream emitted an error chunk"
                            );
                            let error = ProviderError::CompletionsFailed {
                                stage: "stream-chunk",
                                source,
                            };
                            Self::emit_error_event(&event_tx, target, error);
                            break;
                        }
                        None => break,
                    }
                }
            }
        }

        if !cancelled && !stream_failed {
            let _ = event_tx.send(StreamEventMapped {
                target,
                payload: StreamEventPayload::Done,
            });
        }
    }
}

impl LlmProvider for RigProviderAdapter {
    fn id(&self) -> &str {
        &self.config.provider_id
    }

    fn name(&self) -> &str {
        "Rig OpenAI"
    }

    fn default_model(&self) -> &str {
        DEFAULT_OPENAI_MODEL
    }

    fn fallback_models(&self) -> &[Model] {
        &self.fallback_models
    }

    fn fetch_models<'a>(&'a self) -> BoxFuture<'a, ProviderResult<ModelCatalog>> {
        Box::pin(async move {
            if let Some(models) = self.model_cache.get_fresh(self.id()).await {
                return Ok(ModelCatalog::from_cache_fresh(models));
            }

            // Fallback order intentionally prefers availability over strict freshness:
            // provider API first, then stale cache, then static defaults.
            match self.fetch_models_from_provider().await {
                Ok(models) => {
                    self.model_cache.set(self.id(), models.clone()).await;
                    Ok(ModelCatalog::from_provider_api(models))
                }
                Err(error) => {
                    let error_message = error.to_string();

                    if let Some(models) = self.model_cache.get_any(self.id()).await {
                        tracing::warn!(
                            provider_id = %self.id(),
                            cached_model_count = models.len(),
                            error = %error_message,
                            "model fetch failed; serving stale cached models"
                        );
                        return Ok(ModelCatalog::from_cache_stale(models, error_message));
                    }

                    tracing::warn!(
                        provider_id = %self.id(),
                        fallback_model_count = self.fallback_models.len(),
                        error = %error_message,
                        "model fetch failed without cache; serving static fallback models"
                    );

                    Ok(ModelCatalog::from_static_fallback(
                        self.fallback_models.clone(),
                        error_message,
                    ))
                }
            }
        })
    }

    fn stream_chat(&self, request: StreamRequest) -> ProviderResult<ProviderStreamHandle> {
        ensure!(
            !request.messages.is_empty(),
            EmptyMessageSetSnafu {
                stage: "stream-chat",
                target: request.target,
            }
        );

        let (event_tx, stream, cancel_rx) = make_event_stream(request.target);
        let worker: ProviderWorker = Box::pin(Self::run_stream_worker(
            self.config.clone(),
            request,
            event_tx,
            cancel_rx,
        ));

        Ok(ProviderStreamHandle { stream, worker })
    }
}
