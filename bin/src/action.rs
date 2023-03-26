use mees::internals::Message;

use crate::id::ConnectionID;

pub enum Action {
    Ok,
    Send(ConnectionID, Message),
}
