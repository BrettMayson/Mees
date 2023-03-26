use std::{future::Future, io::Cursor};

use internals::{Message, Payload, RequestAsk, RequestResponse};
use rmp_serde::{Deserializer, Serializer};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub use async_trait;
pub use mees_proc::requests;
pub use serde;

mod client;
pub use client::Client;

pub mod internals;

mod responder;
pub use responder::Responder;

#[async_trait::async_trait]
pub trait Requestable: Sized + DeserializeOwned + Serialize {
    type Response: Serialize + DeserializeOwned;
    fn path() -> &'static str;
    async fn handle_local(&self, responder: &Responder) -> Result<Self::Response, String> {
        let response = responder.handle(self.to_message(0)).await;
        Ok(Deserialize::deserialize(&mut Deserializer::new(Cursor::new(response.data))).unwrap())
    }
    async fn request(&self, client: &Client) -> Result<Self::Response, String> {
        let response = client.request(self).await;
        Ok(Deserialize::deserialize(&mut Deserializer::new(Cursor::new(response.data))).unwrap())
    }
    fn from_request(request: RequestAsk) -> Self {
        Deserialize::deserialize(&mut Deserializer::new(Cursor::new(request.data))).unwrap()
    }
    fn to_message(&self, id: u32) -> Message {
        Message {
            payload: Payload::RequestAsk(RequestAsk {
                id,
                data: {
                    let mut buf = Vec::new();
                    self.serialize(&mut Serializer::new(&mut buf)).unwrap();
                    buf
                },
                path: Self::path().to_string(),
            }),
        }
    }
    fn handler<F, Fut>(handler: F) -> responder::Handler<Self>
    where
        Self: Send,
        F: Fn(Self) -> Fut + Copy + Send + Sync + 'static,
        Fut: Future<Output = Self::Response> + Send + Sync + 'static,
    {
        responder::Handler {
            handler: Box::new(move |request| {
                Box::pin(async move {
                    let id = request.id;
                    let mut buf = Vec::new();
                    handler(Self::from_request(request))
                        .await
                        .serialize(&mut Serializer::new(&mut buf))
                        .unwrap();
                    RequestResponse { id, data: buf }
                })
            }),
            phantom: std::marker::PhantomData,
        }
    }
}
