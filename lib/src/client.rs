use std::{
    collections::HashMap,
    fmt::Display,
    net::SocketAddr,
    sync::{atomic::AtomicU32, Arc},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{oneshot::Sender, Mutex, RwLock},
};

use crate::{internals::Control, Message, RequestResponse, Requestable};

type RequestPending = Arc<RwLock<HashMap<u32, Sender<RequestResponse>>>>;

pub struct Client {
    write: Mutex<BufWriter<OwnedWriteHalf>>,
    request_pending: RequestPending,
    request_pending_counter: AtomicU32,
}

impl Client {
    pub async fn new(address: SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let (read, write) = TcpStream::connect(address).await?.into_split();
        let request_pending = Arc::new(RwLock::new(HashMap::new()));
        tokio::spawn(Self::run(read, request_pending.clone()));
        Ok(Self {
            write: Mutex::new(BufWriter::new(write)),
            request_pending,
            request_pending_counter: AtomicU32::new(0),
        })
    }

    pub async fn request(&self, request: &impl Requestable) -> RequestResponse {
        let id = loop {
            let id = self
                .request_pending_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if !self.request_pending.read().await.contains_key(&id) {
                break id;
            }
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.request_pending.write().await.insert(id, tx);
        let message = request.to_message(id);
        let bytes = message.to_bytes();
        let count = bytes.len();
        {
            let mut write = self.write.lock().await;
            write.write_u32(count as u32).await.unwrap();
            write.write_all(&bytes).await.unwrap();
            write.flush().await.unwrap();
        }
        rx.await.unwrap()
    }

    pub async fn run(read: OwnedReadHalf, request_pending: RequestPending) {
        let mut read = BufReader::new(read);
        let mut buffer = [0; 1024];
        loop {
            let n = read.read_u32().await.unwrap();
            if n == 0 {
                break;
            }
            let buf = &mut buffer[..n as usize];
            read.read_exact(buf).await.unwrap();
            let message = Message::from_bytes(buf);
            match message.payload {
                crate::Payload::Control(_) => todo!(),
                crate::Payload::RequestRegister(_) => todo!(),
                crate::Payload::RequestAsk(_) => todo!(),
                crate::Payload::RequestResponse(response) => {
                    let mut request_pending = request_pending.write().await;
                    if let Some(tx) = request_pending.remove(&response.id) {
                        tx.send(response).unwrap();
                    } else {
                        todo!()
                    }
                }
            }
        }
    }

    pub async fn disconnect(self) {
        let mut write = self.write.lock().await;
        let message = Message {
            payload: crate::Payload::Control(Control::Disconnect),
        };
        let bytes = message.to_bytes();
        write.write_u32(bytes.len() as u32).await.unwrap();
        write.write_all(&bytes).await.unwrap();
        write.flush().await.unwrap();
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client")
    }
}
