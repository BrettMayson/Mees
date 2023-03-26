use std::net::{SocketAddr, ToSocketAddrs};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
};

use super::{Connect, Control, Message};

pub struct Connection {
    addrs: Vec<SocketAddr>,
    write: Mutex<BufWriter<OwnedWriteHalf>>,
    read: Mutex<BufReader<OwnedReadHalf>>,
    id: u32,
}

impl Connection {
    pub async fn new<A>(address: A) -> Result<Self, Box<dyn std::error::Error>>
    where
        A: ToSocketAddrs,
    {
        let addrs: Vec<_> = address.to_socket_addrs()?.collect();
        let (read, write) = TcpStream::connect(&*addrs).await?.into_split();
        let write = BufWriter::new(write);
        let read = BufReader::new(read);
        Self::connect(addrs, write, read, Connect::New).await
    }

    pub async fn new_with_id<A>(address: A, id: u32) -> Result<Self, Box<dyn std::error::Error>>
    where
        A: ToSocketAddrs,
    {
        let addrs: Vec<_> = address.to_socket_addrs()?.collect();
        let (read, write) = TcpStream::connect(&*addrs).await?.into_split();
        let write = BufWriter::new(write);
        let read = BufReader::new(read);
        Self::connect(addrs, write, read, Connect::Existing(id)).await
    }

    async fn reconnect(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (read, write) = TcpStream::connect(&*self.addrs).await?.into_split();
        let write = BufWriter::new(write);
        let read = BufReader::new(read);
        let new =
            Self::connect(self.addrs.clone(), write, read, Connect::Existing(self.id)).await?;
        *self.write.lock().await = new.write.into_inner();
        *self.read.lock().await = new.read.into_inner();
        Ok(())
    }

    async fn connect(
        addrs: Vec<SocketAddr>,
        mut write: BufWriter<OwnedWriteHalf>,
        mut read: BufReader<OwnedReadHalf>,
        id: Connect,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Write version
        write.write_u32(1).await?;
        // Read version
        let version = read.read_u32().await?;
        if version != 1 {
            return Err("Version mismatch".into());
        }

        // Write id
        let bytes = Connect::New.into_message().to_bytes();
        write.write_u32(bytes.len() as u32).await?;
        write.write_all(&bytes).await?;

        // Read id
        let n = read.read_u32().await.unwrap();
        if n == 0 {
            return Err("No id".into());
        }
        let buf = &mut [0; 1024];
        read.read_exact(buf).await.unwrap();
        let message = Message::from_bytes(buf);
        if let Message {
            payload: super::Payload::Control(super::Control::ConnectAck(super::ConnectAck(id))),
        } = message
        {
            Ok(Self {
                addrs,
                write: Mutex::new(write),
                read: Mutex::new(read),
                id,
            })
        } else {
            Err("No id".into())
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

    pub async fn send(&self, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let mut sleep = std::time::Duration::from_millis(2);
        loop {
            match self.write(message).await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tokio::time::sleep(sleep).await;
                    if self.reconnect().await.is_err() {
                        sleep *= 2;
                    }
                }
            }
        }
    }

    async fn write(&self, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let mut write = self.write.lock().await;
        let bytes = message.to_bytes();
        write.write_u32(bytes.len() as u32).await?;
        write.write_all(&bytes).await?;
        write.flush().await?;
        Ok(())
    }
}
