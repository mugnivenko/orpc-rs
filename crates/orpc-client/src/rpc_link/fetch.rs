use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::Stream;
use js_sys::{Promise, Uint8Array};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

use super::RpcLink;
use crate::envelope::RpcEnvelope;
use crate::error::ClientError;
use crate::link::{Link, ValueStream};
use crate::sse;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = fetch)]
    fn global_fetch(request: &Request) -> Promise;
}

#[derive(Debug)]
pub struct FetchError {
    message: String,
}

impl FetchError {
    fn from_js(value: &JsValue) -> Self {
        let message = value
            .dyn_ref::<js_sys::Error>()
            .map(|error| String::from(error.message()))
            .or_else(|| value.as_string())
            .unwrap_or_else(|| format!("{value:?}"));
        FetchError { message }
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FetchError {}

fn transport(value: JsValue) -> ClientError {
    ClientError::Transport(FetchError::from_js(&value))
}

async fn post(
    url: &str,
    body: &serde_json::Value,
    last_event_id: Option<u64>,
) -> Result<Response, ClientError> {
    let headers = Headers::new().map_err(transport)?;
    headers
        .set("content-type", "application/json")
        .map_err(transport)?;
    if let Some(id) = last_event_id {
        headers
            .set("last-event-id", &id.to_string())
            .map_err(transport)?;
    }

    let body = serde_json::to_string(body).map_err(ClientError::Serialize)?;
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(url, &init).map_err(transport)?;
    let response = JsFuture::from(global_fetch(&request))
        .await
        .map_err(transport)?;
    response.dyn_into::<Response>().map_err(transport)
}

async fn read_bytes(response: &Response) -> Result<Vec<u8>, ClientError> {
    let buffer = JsFuture::from(response.array_buffer().map_err(transport)?)
        .await
        .map_err(transport)?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

fn rpc_error(bytes: &[u8]) -> ClientError {
    match serde_json::from_slice::<RpcEnvelope<orpc::ORPCError>>(bytes) {
        Ok(envelope) => ClientError::Rpc(envelope.json),
        Err(error) => ClientError::Deserialize(error),
    }
}

struct ChunkStream {
    inner: wasm_streams::readable::IntoStream<'static>,
}

impl Stream for ChunkStream {
    type Item = Result<Bytes, FetchError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx).map(|chunk| {
            chunk.map(|chunk| {
                chunk
                    .map(|value| Bytes::from(Uint8Array::new(&value).to_vec()))
                    .map_err(|value| FetchError::from_js(&value))
            })
        })
    }
}

impl Link for RpcLink {
    async fn call(
        &self,
        path: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let response = post(&self.build_url(path), &Self::build_body(input), None).await?;
        let bytes = read_bytes(&response).await?;

        if response.ok() {
            let envelope: RpcEnvelope<serde_json::Value> =
                serde_json::from_slice(&bytes).map_err(ClientError::Deserialize)?;
            Ok(envelope.json)
        } else {
            Err(rpc_error(&bytes))
        }
    }

    async fn subscribe(
        &self,
        path: &str,
        input: serde_json::Value,
        last_event_id: Option<u64>,
    ) -> Result<ValueStream, ClientError> {
        let response = post(&self.build_url(path), &Self::build_body(input), last_event_id).await?;

        if !response.ok() {
            let bytes = read_bytes(&response).await?;
            return Err(rpc_error(&bytes));
        }

        let body = response
            .body()
            .ok_or_else(|| ClientError::Sse("response has no body".to_owned()))?;
        let chunks = ChunkStream {
            inner: wasm_streams::ReadableStream::from_raw(body.unchecked_into()).into_stream(),
        };
        Ok(sse::sse_to_values(chunks))
    }
}
