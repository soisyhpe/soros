use crate::protocol::{KeyId, ProcId, RequestType};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    sync::mpsc::{channel, Receiver, SendError, Sender},
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccessManagerError {
    #[error("Send error: {0}")]
    SendError(#[from] SendError<AccessGranted>),
    #[error("Could not grant access request for proc: {0}, with key: {1}")]
    RequestAccess(ProcId, KeyId),
    #[error("Could not release access for proc: {0}, with key: {1}")]
    ReleaseAccess(ProcId, KeyId),
    #[error("Key currently accessed")]
    KeyAccessed,
    #[error("Key already exist: {0}")]
    KeyExists(KeyId),
    #[error("Key not found: {0}")]
    KeyNotFound(KeyId),
}

/// Data sent to the channel when an access is granted to a process.
pub type AccessGranted = (ProcId, KeyId, RequestType);

/// Current state of a specific key, tracking current readers and writer.
/// Pending requests are processed alternately to ensure fair access.
/// `creator` indicates the holder of the resource in case of no readers / writer.
#[derive(Debug)]
pub struct KeyState {
    pending_request: VecDeque<(ProcId, RequestType)>,
    readers: HashSet<ProcId>,
    writer: Option<ProcId>,
    pub creator: ProcId,
}

impl KeyState {
    pub fn new(creator: ProcId) -> Self {
        Self {
            pending_request: VecDeque::new(),
            readers: HashSet::new(),
            writer: None,
            creator,
        }
    }

    fn register_reader(&mut self, proc_id: ProcId) {
        self.readers.insert(proc_id);
    }

    fn register_writer(&mut self, proc_id: ProcId) {
        self.writer = Some(proc_id);
    }
}

pub struct AccessManager {
    pub access_granted_rx: Receiver<AccessGranted>,
    access_granted_tx: Sender<AccessGranted>,
    key_states: HashMap<KeyId, KeyState>,
}

impl fmt::Debug for AccessManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessManager")
            .field("on_access_granted", &"OnAccessGranted")
            .field("key_states", &self.key_states)
            .finish()
    }
}

impl Default for AccessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessManager {
    /// Create a new key if it does not already exist.
    pub fn create(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        if self.key_states.contains_key(&key_id) {
            return Err(AccessManagerError::KeyExists(key_id));
        }

        self.key_states.insert(key_id, KeyState::new(proc_id));
        Ok(())
    }

    pub fn delete(&mut self, key_id: KeyId) -> Result<(), AccessManagerError> {
        let key_state = self.get_key_state(key_id)?;
        if !key_state.readers.is_empty() || key_state.writer.is_some() {
            return Err(AccessManagerError::KeyAccessed);
        }

        self.key_states.remove(&key_id);
        Ok(())
    }

    pub fn get_key_state(
        &self,
        key_id: KeyId,
    ) -> Result<&KeyState, AccessManagerError> {
        self.key_states
            .get(&key_id)
            .ok_or(AccessManagerError::KeyNotFound(key_id))
    }

    fn get_key_state_mut(
        &mut self,
        key_id: KeyId,
    ) -> Result<&mut KeyState, AccessManagerError> {
        self.key_states
            .get_mut(&key_id)
            .ok_or(AccessManagerError::KeyNotFound(key_id))
    }

    /// Handle pending request queue for a given key
    fn handle_requesting(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        let key_state = self.key_states.get_mut(&key_id).unwrap();
        let Some((proc_id, req_type)) = key_state.pending_request.front()
        else {
            return Ok(());
        };
        match req_type {
            RequestType::Write => {
                self.access_granted_tx.send((
                    *proc_id,
                    key_id,
                    RequestType::Write,
                ))?;
                key_state.register_writer(*proc_id);
                key_state.pending_request.pop_front();
                Ok(())
            }
            RequestType::Read => {
                while let Some((proc_id, RequestType::Read)) =
                    key_state.pending_request.front()
                {
                    self.access_granted_tx.send((
                        *proc_id,
                        key_id,
                        RequestType::Read,
                    ))?;
                    key_state.register_reader(*proc_id);
                    key_state.pending_request.pop_front();
                }
                Ok(())
            }
            _ => {
                panic!("Unexpected request type")
            }
        }
    }

    pub fn new() -> AccessManager {
        let (access_tx, access_rx) = channel::<AccessGranted>();
        AccessManager {
            access_granted_tx: access_tx,
            access_granted_rx: access_rx,
            key_states: HashMap::new(),
        }
    }

    /// Release write / read access for a process.
    pub fn release(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        let key_state = self.get_key_state_mut(key_id)?;

        let writer_release =
            key_state.writer.is_some_and(|writer| writer == proc_id);
        let reader_release = key_state.readers.contains(&proc_id);

        if !writer_release && !reader_release {
            return Err(AccessManagerError::ReleaseAccess(proc_id, key_id));
        }

        if writer_release {
            key_state.writer = None;
            self.handle_requesting(key_id)?;
        } else if reader_release {
            key_state.readers.remove(&proc_id);
            if key_state.readers.is_empty() {
                self.handle_requesting(key_id)?;
            }
        }

        Ok(())
    }

    /// Request read access for a process.
    /// Multiple read are possible at the same time.
    pub fn read(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<ProcId, AccessManagerError> {
        let key_state = self.get_key_state_mut(key_id)?;

        let requesting_writer = key_state
            .pending_request
            .front()
            .is_some_and(|req| matches!(req.1, RequestType::Write));

        if key_state.writer.is_some() || requesting_writer {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Read));
            return Err(AccessManagerError::RequestAccess(proc_id, key_id));
        }

        key_state.register_reader(proc_id);

        // Find a processus that knows the key's data
        // The writer has the priority, because he can modify the data
        let mut data_user = key_state.creator;
        if let Some(writer) = key_state.writer {
            data_user = writer;
        }

        Ok(data_user)
    }

    /// Reqest write access for a process.
    /// Only one write access at a time.
    pub fn write(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        let key_state = self.get_key_state_mut(key_id)?;

        if !key_state.readers.is_empty() || key_state.writer.is_some() {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Write));
            return Err(AccessManagerError::RequestAccess(proc_id, key_id));
        }

        key_state.register_writer(proc_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_manager() -> AccessManager {
        AccessManager::new()
    }

    #[test]
    fn test_create() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.get_key_state(0).is_err());
        manager.create(2, 0)?;
        assert_eq!(manager.get_key_state(0)?.creator, 2);
        assert!(manager.create(0, 0).is_err());
        Ok(())
    }

    #[test]
    fn test_delete() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.delete(0).is_err());
        manager.create(0, 0)?;
        manager.delete(0)?;
        assert!(manager.get_key_state(0).is_err());
        Ok(())
    }

    #[test]
    fn test_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.read(1, 0).is_err());
        manager.create(0, 0)?;
        manager.read(1, 0)?;
        manager.read(2, 0)?;
        manager.read(3, 0)?;
        assert!(manager.delete(0).is_err());
        assert_eq!(manager.get_key_state(0)?.pending_request, vec![]);
        assert_eq!(manager.get_key_state(0)?.readers, HashSet::from([1, 2, 3]));
        Ok(())
    }

    #[test]
    fn test_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.write(1, 0).is_err());
        manager.create(0, 0)?;
        manager.write(1, 0)?;
        assert!(manager.delete(0).is_err());
        assert!(manager.write(2, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Write)]
        );
        assert_eq!(manager.get_key_state(0)?.writer, Some(1));
        Ok(())
    }

    #[test]
    fn test_read_before_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.read(1, 0)?;
        manager.read(2, 0)?;
        assert!(manager.write(3, 0).is_err());
        assert!(manager.read(4, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(3, RequestType::Write), (4, RequestType::Read)]
        );
        Ok(())
    }

    #[test]
    fn test_write_before_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.write(1, 0)?;
        assert!(manager.read(2, 0).is_err());
        assert!(manager.write(3, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Read), (3, RequestType::Write)]
        );
        Ok(())
    }

    #[test]
    fn test_release_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        assert!(manager.release(1, 0).is_err());
        manager.read(1, 0)?;
        assert!(manager.release(2, 0).is_err());
        manager.release(1, 0)?;
        assert!(manager.get_key_state(0)?.readers.is_empty());
        Ok(())
    }

    #[test]
    fn test_release_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        assert!(manager.release(1, 0).is_err());
        manager.write(1, 0)?;
        assert!(manager.release(2, 0).is_err());
        manager.release(1, 0)?;
        assert_eq!(manager.get_key_state(0)?.writer, None);
        Ok(())
    }

    macro_rules! assert_grant {
        ($manager:expr, $proc:expr, $key:expr, $req:expr) => {
            let data = $manager.access_granted_rx.try_recv().unwrap();
            assert_eq!(data, ($proc, $key, $req));
        };
    }

    #[test]
    fn test_handling_read_before_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.read(1, 0)?;
        assert!(manager.write(2, 0).is_err());
        assert!(manager.read(3, 0).is_err());

        manager.release(1, 0)?;
        assert_grant!(manager, 2, 0, RequestType::Write);

        manager.release(2, 0)?;
        assert_grant!(manager, 3, 0, RequestType::Read);

        Ok(())
    }

    #[test]
    fn test_handling_write_before_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.write(1, 0)?;
        assert!(manager.read(2, 0).is_err());
        assert!(manager.read(3, 0).is_err());
        assert!(manager.read(4, 0).is_err());
        assert!(manager.write(5, 0).is_err());

        manager.release(1, 0)?;
        assert_grant!(manager, 2, 0, RequestType::Read);
        assert_grant!(manager, 3, 0, RequestType::Read);
        assert_grant!(manager, 4, 0, RequestType::Read);

        manager.release(2, 0)?;
        manager.release(3, 0)?;
        assert_eq!(manager.access_granted_rx.try_iter().count(), 0);
        manager.release(4, 0)?;
        assert_grant!(manager, 5, 0, RequestType::Write);

        Ok(())
    }

    // correspond to the report fairness diagram
    #[test]
    fn test_fairness() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        let (x, a, b, c, d) = (0, 1, 2, 3, 4);
        manager.create(a, x)?;
        assert_eq!(manager.get_key_state(x)?.creator, a);

        manager.read(a, x)?;
        assert!(manager.write(c, x).is_err());
        assert!(manager.read(b, x).is_err());
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(c, RequestType::Write), (b, RequestType::Read)]
        );

        manager.release(a, x)?;
        assert_grant!(manager, c, x, RequestType::Write);
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Read)]
        );

        assert!(manager.read(d, x).is_err());
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Read), (d, RequestType::Read)]
        );

        manager.release(c, 0)?;
        assert_grant!(manager, b, x, RequestType::Read);
        assert_grant!(manager, d, x, RequestType::Read);

        assert!(manager.get_key_state(x)?.pending_request.is_empty());
        assert_eq!(manager.get_key_state(x)?.readers, HashSet::from([b, d]));
        assert!(manager.get_key_state(x)?.writer.is_none());

        Ok(())
    }
}
