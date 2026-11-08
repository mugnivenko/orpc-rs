use super::RpcLink;
use crate::envelope::RpcEnvelope;
use crate::error::ClientError;
use crate::link::{Link, ValueStream};
use crate::sse;

impl Link for RpcLink {
    async fn call(
        &self,
        path: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let url = self.build_url(path);
        let body = Self::build_body(input);

        let response = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let bytes = response.bytes().await?;

        if status.is_success() {
            let envelope: RpcEnvelope<serde_json::Value> =
                serde_json::from_slice(&bytes).map_err(ClientError::Deserialize)?;
            Ok(envelope.json)
        } else {
            let envelope: RpcEnvelope<orpc::ORPCError> =
                serde_json::from_slice(&bytes).map_err(ClientError::Deserialize)?;
            Err(ClientError::Rpc(envelope.json))
        }
    }

    async fn subscribe(
        &self,
        path: &str,
        input: serde_json::Value,
        last_event_id: Option<u64>,
    ) -> Result<ValueStream, ClientError> {
        let url = self.build_url(path);
        let body = Self::build_body(input);

        let mut request = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .json(&body);

        if let Some(id) = last_event_id {
            request = request.header("last-event-id", id.to_string());
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let bytes = response.bytes().await?;
            let envelope: RpcEnvelope<orpc::ORPCError> =
                serde_json::from_slice(&bytes).map_err(ClientError::Deserialize)?;
            return Err(ClientError::Rpc(envelope.json));
        }

        let byte_stream = response.bytes_stream();
        Ok(sse::sse_to_values(byte_stream))
    }
}
