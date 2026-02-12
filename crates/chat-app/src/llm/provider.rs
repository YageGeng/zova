use std::future::Future;
use std::pin::Pin;

use snafu::Snafu;
use tokio::sync::{mpsc, oneshot};

use crate::chat::{Role, StreamEventMapped, StreamTarget};

use super::model::{Model, ModelCatalog};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_id: String,
    pub api_key: String,
    pub base_url: String,
    pub default_model: Option<String>,
}

impl ProviderConfig {
    pub fn new(
        provider_id: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        default_model: Option<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into().trim().to_string(),
            api_key: api_key.into().trim().to_string(),
            base_url: base_url.into().trim().to_string(),
            default_model,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMessage {
    pub role: Role,
    pub content: String,
}

impl ProviderMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamRequest {
    pub target: StreamTarget,
    pub model_id: String,
    pub messages: Vec<ProviderMessage>,
    pub preamble: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
}

impl StreamRequest {
    pub fn new(
        target: StreamTarget,
        model_id: impl Into<String>,
        messages: Vec<ProviderMessage>,
    ) -> Self {
        Self {
            target,
            model_id: model_id.into(),
            messages,
            preamble: None,
            temperature: None,
            max_tokens: None,
        }
    }

    pub fn with_preamble(mut self, preamble: impl Into<String>) -> Self {
        self.preamble = Some(preamble.into());
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type ProviderWorker = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum ProviderError {
    #[snafu(display("missing API key for provider '{provider_id}'"))]
    MissingApiKey {
        stage: &'static str,
        provider_id: String,
    },
    #[snafu(display("provider '{provider_id}' is not supported"))]
    UnsupportedProvider {
        stage: &'static str,
        provider_id: String,
    },
    #[snafu(display("stream request for {target:?} has no messages"))]
    EmptyMessageSet {
        stage: &'static str,
        target: StreamTarget,
    },
    #[snafu(display("http client failed on `{stage}`, {source}"))]
    HttpClient {
        stage: &'static str,
        source: rig::http_client::Error,
    },
    #[snafu(display("failed to finalize HTTP request body: {message}"))]
    BuildHttpRequestBody {
        stage: &'static str,
        message: String,
    },
    #[snafu(display("provider model endpoint returned status {status}: {body}"))]
    ModelFetchStatus {
        stage: &'static str,
        status: u16,
        body: String,
    },
    #[snafu(display("failed to parse provider model list: {details}"))]
    ModelPayloadParse {
        stage: &'static str,
        details: String,
    },
    #[snafu(display("completions failed on `{stage}`, {source}"))]
    CompletionsFailed {
        stage: &'static str,
        source: rig::completion::CompletionError,
    },
}

pub struct ProviderEventStream {
    target: StreamTarget,
    events: mpsc::UnboundedReceiver<StreamEventMapped>,
    cancel_tx: Option<oneshot::Sender<()>>,
}

pub struct ProviderStreamHandle {
    pub stream: ProviderEventStream,
    pub worker: ProviderWorker,
}

impl ProviderEventStream {
    pub(crate) fn new(
        target: StreamTarget,
        events: mpsc::UnboundedReceiver<StreamEventMapped>,
        cancel_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            target,
            events,
            cancel_tx: Some(cancel_tx),
        }
    }

    pub fn target(&self) -> StreamTarget {
        self.target
    }

    pub async fn recv(&mut self) -> Option<StreamEventMapped> {
        self.events.recv().await
    }

    pub fn try_recv(&mut self) -> Option<StreamEventMapped> {
        self.events.try_recv().ok()
    }

    pub fn cancel(&mut self) -> bool {
        self.cancel_tx
            .take()
            .map(|tx| tx.send(()).is_ok())
            .unwrap_or(false)
    }
}

impl Drop for ProviderEventStream {
    fn drop(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
    }
}

pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn default_model(&self) -> &str;
    fn fallback_models(&self) -> &[Model];
    fn fetch_models<'a>(&'a self) -> BoxFuture<'a, ProviderResult<ModelCatalog>>;
    fn stream_chat(&self, request: StreamRequest) -> ProviderResult<ProviderStreamHandle>;
}

pub(crate) fn make_event_stream(
    target: StreamTarget,
) -> (
    mpsc::UnboundedSender<StreamEventMapped>,
    ProviderEventStream,
    oneshot::Receiver<()>,
) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    (
        event_tx,
        ProviderEventStream::new(target, event_rx, cancel_tx),
        cancel_rx,
    )
}
