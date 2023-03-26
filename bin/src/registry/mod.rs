use std::{collections::HashMap, sync::atomic::AtomicU32};

use mees::internals::{Connect, Control, Message, Payload};
use tokio::sync::RwLock;

use crate::{action::Action, id::ConnectionID};

#[derive(Debug)]
pub struct Registry {
    events_subscribers: RwLock<HashMap<String, Vec<ConnectionID>>>,
    request_handlers: RwLock<HashMap<String, Vec<ConnectionID>>>,
    request_handlers_roundrobin: RwLock<HashMap<String, AtomicU32>>,
    request_pending: RwLock<HashMap<u32, (u32, ConnectionID)>>,
    request_count: AtomicU32,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            events_subscribers: RwLock::new(HashMap::new()),
            request_handlers: RwLock::new(HashMap::new()),
            request_handlers_roundrobin: RwLock::new(HashMap::new()),
            request_pending: RwLock::new(HashMap::new()),
            request_count: AtomicU32::new(0),
        }
    }

    pub async fn event_subscribe(&self, path: &str, id: ConnectionID) -> Result<(), String> {
        let selector = path.to_string();
        self.events_subscribers
            .write()
            .await
            .entry(selector)
            .or_insert_with(Vec::new)
            .push(id);
        Ok(())
    }

    pub async fn event_unsubscribe(&self, path: &str, id: ConnectionID) {
        let mut events_subscribers = self.events_subscribers.write().await;
        if let Some(entry) = events_subscribers.get_mut(path) {
            entry.retain(|&x| x != id);
        }
    }

    pub async fn event_subscribers(&self, path: &str) -> Vec<ConnectionID> {
        let events_subscribers = self.events_subscribers.read().await;
        events_subscribers.get(path).cloned().unwrap_or_default()
    }

    pub async fn request_subscribe(&self, path: String, id: ConnectionID) {
        self.request_handlers
            .write()
            .await
            .entry(path)
            .or_insert_with(Vec::new)
            .push(id);
    }

    pub async fn request_unsubscribe(&self, path: &str, id: ConnectionID) {
        let mut request_handlers = self.request_handlers.write().await;
        if let Some(entry) = request_handlers.get_mut(path) {
            entry.retain(|&x| x != id);
        }
    }

    pub async fn request_subscribers(&self, path: &str) -> Vec<ConnectionID> {
        let request_handlers = self.request_handlers.read().await;
        request_handlers.get(path).cloned().unwrap_or_default()
    }

    pub async fn is_active(&self, id: ConnectionID) -> bool {
        for (_, entry) in self.events_subscribers.read().await.iter() {
            if entry.contains(&id) {
                return true;
            }
        }
        for (_, entry) in self.request_handlers.read().await.iter() {
            if entry.contains(&id) {
                return true;
            }
        }
        false
    }

    pub async fn handle_message(&self, client: ConnectionID, msg: Message) -> Action {
        match msg.payload {
            Payload::Control(control) => match control {
                Control::Disconnect => {
                    let mut events_subscribers = self.events_subscribers.write().await;
                    for (_, entry) in events_subscribers.iter_mut() {
                        entry.retain(|&x| x != client);
                    }
                    let mut request_handlers = self.request_handlers.write().await;
                    for (_, entry) in request_handlers.iter_mut() {
                        entry.retain(|&x| x != client);
                    }
                    Action::Ok
                }
                _ => {
                    todo!()
                }
            },
            Payload::RequestRegister(register) => {
                println!("Registering request handler: {:?}", register.path);
                self.request_subscribe(register.path.clone(), client).await;
                Action::Ok
            }
            Payload::RequestAsk(request) => {
                let subscribers = self.request_subscribers(&request.path).await;
                if subscribers.is_empty() {
                    Action::Ok
                } else {
                    let handler = {
                        let mut request_handlers_roundrobin =
                            self.request_handlers_roundrobin.write().await;
                        let index = request_handlers_roundrobin
                            .entry(request.path.clone())
                            .or_insert_with(|| AtomicU32::new(0))
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        subscribers[index as usize % subscribers.len()]
                    };
                    {
                        let mut request_pending = self.request_pending.write().await;
                        let id = loop {
                            let id = self
                                .request_count
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if !request_pending.contains_key(&id) {
                                break id;
                            }
                        };
                        request_pending.insert(id, (request.id, client));
                        let mut request = request;
                        request.id = id;
                        Action::Send(
                            handler,
                            Message {
                                payload: Payload::RequestAsk(request),
                            },
                        )
                    }
                }
            }
            Payload::RequestResponse(response) => {
                let mut request_pending = self.request_pending.write().await;
                if let Some((id, client)) = request_pending.remove(&response.id) {
                    let mut response = response;
                    response.id = id;
                    Action::Send(
                        client,
                        Message {
                            payload: Payload::RequestResponse(response),
                        },
                    )
                } else {
                    todo!()
                }
            }
        }
    }
}
