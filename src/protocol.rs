use serde::{Deserialize, Serialize};

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

/// Represents different kind of responses from the registry.
#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryResponse {
    Success,
    /// Return a process id using the data, can be used for the peer-to-peer protocol.
    Holder(ProcId),
    Error(String),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum RequestType {
    Create,
    Delete,
    Read,
    Write,
}

/// Represent a message related to the registry
#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryMessage {
    /// Request for accessing registry data.
    Request {
        request_type: RequestType,
        proc_id: ProcId,
        key_id: KeyId,
    },
    /// Response containing registry data.
    Response(RegistryResponse),
}

/// Represent a message related to the peer-to-peer data exchange protocol
#[derive(Serialize, Deserialize, Debug)]
pub enum DataMessage {
    /// Request for peer-to-peer data content.
    Request { _data: u32 },
    /// Response containing peer-to-peer data content.
    Response { _data: u32 },
}
