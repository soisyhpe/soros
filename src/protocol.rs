/// Represents the type of p2p message
///
/// Only two type of messages are allowed: Request and Data
pub enum MessageType {
    /// Indicates that you're asking for a resource
    Request,
    /// Indicates that you're answering a resource request
    Data,
}

/// Represents the communication between two clients
pub struct Message {
    /// Who is sending the message (useful for the answer)
    sender: u32,
    /// Type of message
    message_type: MessageType,
    /// Requested data
    data_key: String,
}

impl Message {
    pub fn new(sender: u32, message_type: MessageType, data_key: String) -> Message {
        Message {
            sender,
            message_type,
            data_key,
        }
    }
}

#[cfg(test)]
mod tests {}