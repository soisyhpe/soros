use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Unique identifier for a process.
pub type ProcId = usize;
/// Unique identifier for a key.
pub type KeyId = usize;

/// Represents different types of messages exchanged in the protocol.
#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    /// Represent request / response related to the registry.
    Registry(RegistryMessage),
    /// Represent request / response related to peer-to-peer data.
    Data(DataMessage),
}

impl Message {
    pub fn to_vec(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut data = serde_json::to_vec(self)?;
        data.extend_from_slice(b"\n");
        Ok(data)
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

/// Represents different kind of responses from the registry.
#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryResponse {
    Success(KeyId),
    /// The request cannot be fullfilled yet
    Wait(KeyId),
    /// Return a process id using the data, can be used for the peer-to-peer protocol.
    Holder(KeyId, SocketAddr),
    Error(String),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum RequestType {
    Create,
    Delete,
    Read,
    Write,
    Release,
}

/// Represent a message related to the registry
#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryMessage {
    /// Unique proc id given by the registry server during connection
    Connection(ProcId),
    /// Request for accessing registry data.
    Request {
        request_type: RequestType,
        key_id: KeyId,
    },
    /// Response containing registry data.
    Response(RegistryResponse),
    /// Used for gracefully shutdown in the tests
    StopRequest,
}

/// Represent a message related to the peer-to-peer data exchange protocol
#[derive(Serialize, Deserialize, Debug)]
pub enum DataMessage {
    /// Request for peer-to-peer data content.
    Request { _data: u32 },
    /// Response containing peer-to-peer data content.
    Response { _data: u32 },
}

#[macro_export]
macro_rules! registry_request {
    ($key_id: expr, $request_type: ident) => {
        Message::Registry(RegistryMessage::Request {
            key_id: $key_id,
            request_type: $request_type,
        })
    };
}

#[macro_export]
macro_rules! registry_connection {
    ($proc_id: expr) => {
        Message::Registry(RegistryMessage::Connection($proc_id))
    };
}

#[macro_export]
macro_rules! registry_response {
    ($resp: expr) => {
        Message::Registry(RegistryMessage::Response($resp))
    };
}

#[macro_export]
macro_rules! registry_stop {
    () => {
        Message::Registry(RegistryMessage::StopRequest)
    };
}
