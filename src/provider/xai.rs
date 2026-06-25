use crate::models::{MessageRequest, MessageResponse};
use crate::provider::{ProviderClient, ProviderError, ProviderKind};
use crate::sse::SSEEvent;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

pub struct XAIClient {
    compat_client: super::openai_compat::OpenAICompatClient,
}

impl XAIClient {
    pub fn new() -> Self {
        Self {
            compat_client: super::openai_compat::OpenAICompatClient::with_custom(
                "https://api.x.ai/v1/chat/completions".to_string(),
                ProviderKind::XAI,
            ),
        }
    }
}

#[async_trait]
impl ProviderClient for XAIClient {
    async fn send_message(&self, request: &MessageRequest) -> Result<MessageResponse, ProviderError> {
        self.compat_client.send_message(request).await
    }

    async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SSEEvent, ProviderError>> + Send>>, ProviderError> {
        self.compat_client.stream_message(request).await
    }
}
