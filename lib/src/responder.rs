use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
};

use crate::{
    internals::{Message, Payload, RequestAsk, RequestRegister, RequestResponse},
    Requestable,
};

type HandlerFut = Pin<Box<dyn Future<Output = RequestResponse> + Send>>;
type HandlerFunc = Box<dyn Fn(RequestAsk) -> HandlerFut + Send + Sync>;

pub struct Handler<T> {
    pub(crate) handler: HandlerFunc,
    pub(crate) phantom: PhantomData<T>,
}

impl<T> Handler<T> {
    pub fn consume(self) -> HandlerFunc {
        self.handler
    }
}

#[derive(Default)]
pub struct Responder {
    handlers: HashMap<String, HandlerFunc>,
}

impl Responder {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register<R>(&mut self, handler: Handler<R>)
    where
        R: Requestable + 'static,
    {
        self.handlers
            .insert(R::path().to_string(), handler.consume());
    }

    pub async fn handle(&self, message: Message) -> RequestResponse {
        if let Payload::RequestAsk(request) = message.payload {
            if let Some(handler) = self.handlers.get(&request.path) {
                return handler(request).await;
            }
        }
        unimplemented!()
    }

    pub async fn run(&self, address: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = TcpStream::connect(address).await?;
        let (read, write) = conn.split();
        let mut read = BufReader::new(read);
        let mut write = BufWriter::new(write);
        for handler in &self.handlers {
            let message = Message {
                payload: Payload::RequestRegister(RequestRegister {
                    path: handler.0.to_string(),
                }),
            };
            let bytes = &message.to_bytes();
            write.write_u32(bytes.len() as u32).await?;
            write.write_all(bytes).await?;
            write.flush().await?;
        }
        let mut buffer = [0; 1024];
        loop {
            let expect = read.read_u32().await?;
            if expect == 0 {
                break;
            }
            let buf = &mut buffer[..expect as usize];
            let _ = read.read_exact(buf).await?;
            let message = Message::from_bytes(buf);
            let response = self.handle(message).await;
            let message = Message {
                payload: Payload::RequestResponse(response),
            };
            let bytes = &message.to_bytes();
            write.write_u32(bytes.len() as u32).await?;
            write.write_all(bytes).await?;
            write.flush().await?;
        }
        Ok(())
    }
}
