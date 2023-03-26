use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use mees::internals::Message;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    sync::RwLock,
};

mod action;
mod id;
mod registry;

pub async fn run(addr: SocketAddr) {
    let listener = TcpListener::bind(addr).await.unwrap();

    let registry = Arc::new(registry::Registry::new());
    let conn_counter = std::sync::atomic::AtomicU32::new(0);
    let connections = Arc::new(RwLock::new(HashMap::new()));

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(32);
        let conn_id = loop {
            let conn_id =
                id::ConnectionID(conn_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
            if let Entry::Vacant(e) = connections.write().await.entry(conn_id) {
                e.insert(tx);
                break conn_id;
            }
        };
        let registry = registry.clone();
        let connections = connections.clone();

        tokio::spawn(async move {
            // Wait for either a message from the socket or a message from the registry
            let (read, write) = socket.split();
            let mut read = BufReader::new(read);
            let mut write = BufWriter::new(write);
            let mut buffer = [0; 1024];
            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        let msg = msg.to_bytes();
                        write.write_u32(msg.len() as u32).await.unwrap();
                        write.write_all(&msg).await.unwrap();
                        write.flush().await.unwrap();
                    }
                    res = read.read_u32() => {
                        let n = res.unwrap();
                        let buf = &mut buffer[..n as usize];
                        if n == 0 {
                            break;
                        }
                        let _ = read.read_exact(buf).await.unwrap();
                        let msg = Message::from_bytes(buf);
                        let action = registry.handle_message(conn_id, msg).await;
                        match action {
                            action::Action::Ok=>{}
                            action::Action::Send(connection, message) => {
                                if let Some(sender) = connections.read().await.get(&connection) {
                                    sender.send(message).await.unwrap();
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
