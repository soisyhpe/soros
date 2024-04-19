use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    ops::Deref,
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccessManagerError {
    #[error("Could not grant access request for proc: {0}, with key: {1}")]
    RequestAccess(ProcId, KeyId),
    #[error("Could not release access for proc: {0}, with key: {1}")]
    ReleaseAccess(ProcId, KeyId),
    #[error("Key already exist: {0}")]
    KeyExists(KeyId),
    #[error("Key not found: {0}")]
    KeyNotFound(KeyId),
}

/// Unique identifier for a process.
type ProcId = usize;
/// Unique identifier for a key.
type KeyId = usize;

#[derive(Debug, PartialEq, Eq)]
pub enum RequestType {
    Writer,
    Reader,
}

/// Closure called when access is granted to a process.
pub type OnAccessGranted = Box<dyn Fn(ProcId, KeyId, RequestType)>;

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
    on_access_granted: OnAccessGranted,
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
        if !self.key_states.contains_key(&key_id) {
            return Err(AccessManagerError::KeyNotFound(key_id));
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
    fn handle_requesting(&mut self, key_id: KeyId) {
        let key_state = self.key_states.get_mut(&key_id).unwrap();
        let Some((proc_id, req_type)) = key_state.pending_request.front()
        else {
            return;
        };
        match req_type {
            RequestType::Writer => {
                self.on_access_granted.deref()(
                    *proc_id,
                    key_id,
                    RequestType::Writer,
                );
                key_state.register_writer(*proc_id);
                key_state.pending_request.pop_front();
            }
            RequestType::Reader => {
                while let Some((proc_id, RequestType::Reader)) =
                    key_state.pending_request.front()
                {
                    self.on_access_granted.deref()(
                        *proc_id,
                        key_id,
                        RequestType::Reader,
                    );
                    key_state.register_reader(*proc_id);
                    key_state.pending_request.pop_front();
                }
            }
        }
    }

    pub fn new(on_grant: OnAccessGranted) -> AccessManager {
        AccessManager {
            on_access_granted: on_grant,
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
            self.handle_requesting(key_id);
        } else if reader_release {
            key_state.readers.remove(&proc_id);
            if key_state.readers.is_empty() {
                self.handle_requesting(key_id);
            }
        }

        Ok(())
    }

    /// Request read access for a process.
    /// Multiple read are possible at the same time.
    pub fn request_read(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        let key_state = self.get_key_state_mut(key_id)?;

        let requesting_writer = key_state
            .pending_request
            .front()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if key_state.writer.is_some() || requesting_writer {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Reader));
            return Err(AccessManagerError::RequestAccess(proc_id, key_id));
        }

        key_state.register_reader(proc_id);
        Ok(())
    }

    /// Reqest write access for a process.
    /// Only one write access at a time.
    pub fn request_write(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), AccessManagerError> {
        let key_state = self.get_key_state_mut(key_id)?;

        if !key_state.readers.is_empty() || key_state.writer.is_some() {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Writer));
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
    use std::sync::mpsc::{channel, Receiver, Sender};

    fn create_manager() -> AccessManager {
        AccessManager::new(Box::new(|_, _, _| {}))
    }

    fn create_manager_with_grant(
    ) -> (AccessManager, Sender<OnAccessGranted>, Receiver<bool>) {
        let (fn_tx, fn_rx) = channel::<OnAccessGranted>();
        let (call_tx, call_rx) = channel();
        (
            AccessManager::new(Box::new(move |proc_id, key_id, req_type| {
                fn_rx.try_recv().unwrap()(proc_id, key_id, req_type);
                call_tx.send(true).unwrap();
            })),
            fn_tx,
            call_rx,
        )
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
    fn test_request_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.request_read(1, 0).is_err());
        manager.create(0, 0)?;
        manager.request_read(1, 0)?;
        manager.request_read(2, 0)?;
        manager.request_read(3, 0)?;
        assert_eq!(manager.get_key_state(0)?.pending_request, vec![]);
        assert_eq!(manager.get_key_state(0)?.readers, HashSet::from([1, 2, 3]));
        Ok(())
    }

    #[test]
    fn test_request_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        assert!(manager.request_write(1, 0).is_err());
        manager.create(0, 0)?;
        manager.request_write(1, 0)?;
        assert!(manager.request_write(2, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Writer)]
        );
        assert_eq!(manager.get_key_state(0)?.writer, Some(1));
        Ok(())
    }

    #[test]
    fn test_request_read_before_write() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.request_read(1, 0)?;
        manager.request_read(2, 0)?;
        assert!(manager.request_write(3, 0).is_err());
        assert!(manager.request_read(4, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(3, RequestType::Writer), (4, RequestType::Reader)]
        );
        Ok(())
    }

    #[test]
    fn test_request_write_before_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        manager.request_write(1, 0)?;
        assert!(manager.request_read(2, 0).is_err());
        assert!(manager.request_write(3, 0).is_err());
        assert_eq!(
            manager.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Reader), (3, RequestType::Writer)]
        );
        Ok(())
    }

    #[test]
    fn test_release_read() -> Result<(), AccessManagerError> {
        let mut manager = create_manager();
        manager.create(0, 0)?;
        assert!(manager.release(1, 0).is_err());
        manager.request_read(1, 0)?;
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
        manager.request_write(1, 0)?;
        assert!(manager.release(2, 0).is_err());
        manager.release(1, 0)?;
        assert_eq!(manager.get_key_state(0)?.writer, None);
        Ok(())
    }

    macro_rules! assert_on_grant {
        ($tx:expr, $proc:expr, $key:expr, $req:expr) => {
            $tx.send(Box::new(move |proc_id, key_id, req_type| {
                assert_eq!((proc_id, key_id, req_type), ($proc, $key, $req));
            }))
            .unwrap();
        };
    }

    #[test]
    fn test_handling_read_before_write() -> Result<(), AccessManagerError> {
        let (mut manager, fn_tx, call_rx) = create_manager_with_grant();
        manager.create(0, 0)?;
        manager.request_read(1, 0)?;
        assert!(manager.request_write(2, 0).is_err());
        assert!(manager.request_read(3, 0).is_err());

        assert_on_grant!(fn_tx, 2, 0, RequestType::Writer);
        manager.release(1, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        assert_on_grant!(fn_tx, 3, 0, RequestType::Reader);
        manager.release(2, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    #[test]
    fn test_handling_write_before_read() -> Result<(), AccessManagerError> {
        let (mut manager, fn_tx, call_rx) = create_manager_with_grant();
        manager.create(0, 0)?;
        manager.request_write(1, 0)?;
        assert!(manager.request_read(2, 0).is_err());
        assert!(manager.request_read(3, 0).is_err());
        assert!(manager.request_read(4, 0).is_err());
        assert!(manager.request_write(5, 0).is_err());

        for i in 2..=4 {
            assert_on_grant!(fn_tx, i, 0, RequestType::Reader);
        }
        manager.release(1, 0)?;
        assert_eq!(call_rx.try_iter().count(), 3);

        assert_on_grant!(fn_tx, 5, 0, RequestType::Writer);
        manager.release(2, 0)?;
        manager.release(3, 0)?;
        assert_eq!(call_rx.try_iter().count(), 0);
        manager.release(4, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    // correspond to the report fairness diagram
    #[test]
    fn test_fairness() -> Result<(), AccessManagerError> {
        let (mut manager, fn_tx, call_rx) = create_manager_with_grant();
        let (x, a, b, c, d) = (0, 1, 2, 3, 4);
        manager.create(a, x)?;
        assert_eq!(manager.get_key_state(x)?.creator, a);

        manager.request_read(a, x)?;
        assert!(manager.request_write(c, x).is_err());
        assert!(manager.request_read(b, x).is_err());
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(c, RequestType::Writer), (b, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, c, x, RequestType::Writer);
        manager.release(a, x)?;
        assert_eq!(call_rx.try_iter().count(), 1);
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Reader)]
        );

        assert!(manager.request_read(d, x).is_err());
        assert_eq!(
            manager.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Reader), (d, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, b, x, RequestType::Reader);
        assert_on_grant!(fn_tx, d, x, RequestType::Reader);
        manager.release(c, 0)?;
        assert_eq!(call_rx.try_iter().count(), 2);

        assert!(manager.get_key_state(x)?.pending_request.is_empty());
        assert_eq!(manager.get_key_state(x)?.readers, HashSet::from([b, d]));
        assert!(manager.get_key_state(x)?.writer.is_none());

        Ok(())
    }
}
