use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use mees::internals::{Connect, Control, Message, Payload};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, ToSocketAddrs},
    sync::RwLock,
};

mod action;
mod id;
mod registry;

pub async fn run<A>(addr: A)
where
    A: ToSocketAddrs,
{
    let listener = TcpListener::bind(addr).await.unwrap();

    let registry = Arc::new(registry::Registry::new());
    let conn_counter = std::sync::atomic::AtomicU32::new(0);
    let connections = Arc::new(RwLock::new(HashMap::new()));

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(32);

        // Wait for either a message from the socket or a message from the registry
        let (read, write) = socket.split();
        let mut read = BufReader::new(read);
        let mut write = BufWriter::new(write);
        let mut buffer = [0; 1024];

        // Version negotiation
        let version = read.read_u32().await.unwrap();
        write.write_u32(1).await.unwrap();
        write.flush().await.unwrap();

        if version != 1 {
            return;
        }

        // ID negotiation
        let size = read.read_u32().await.unwrap();
        let mut buf = vec![0; size as usize];
        read.read_exact(&mut buf).await.unwrap();
        let msg = Message::from_bytes(&buf);
        let Payload::Control(Control::Connect(connect)) = msg.payload else {
            return;
        };
        let conn_id = match connect {
            Connect::New => loop {
                let conn_id = id::ConnectionID(
                    conn_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                );
                if let Entry::Vacant(e) = connections.write().await.entry(conn_id) {
                    e.insert(tx);
                    break conn_id;
                }
            },
            Connect::Existing(id) => id,
        };
        let registry = registry.clone();
        let connections = connections.clone();

        tokio::spawn(async move {
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
                            // Connection closed
                            break;
                        }
                        let _ = read.read_exact(buf).await.unwrap();
                        let msg = Message::from_bytes(buf);
                        let action = registry.handle_message(conn_id, msg).await;
                        match action {
                            action::Action::Ok=>{}
                            action::Action::Send(connection, message) => {
                                tokio::spawn(async move {
                                    let mut i = 0;
                                    loop {
                                        if let Some(tx) = connections.read().await.get(&connection) {
                                            tx.send(message).await.unwrap();
                                            break;
                                        }
                                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                        i += 1;
                                        if i > 100 {
                                            break;
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        });
    }
}
