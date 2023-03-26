use rmp_serde::Deserializer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestRegister {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestAsk {
    pub id: u32,
    pub path: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestResponse {
    pub id: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Control {
    Ping,
    Pong,
    AuthPass(String),
    Disconnect,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Payload {
    Control(Control),
    RequestRegister(RequestRegister),
    RequestAsk(RequestAsk),
    RequestResponse(RequestResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub payload: Payload,
}

impl Message {
    pub fn new(payload: Payload) -> Self {
        Self { payload }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Deserialize::deserialize(&mut Deserializer::new(bytes)).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut rmp_serde::Serializer::new(&mut buf))
            .unwrap();
        buf
    }
}
