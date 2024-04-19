use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    ops::Deref,
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RwQueueError {
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

pub struct RwQueue {
    on_access_granted: OnAccessGranted,
    key_states: HashMap<KeyId, KeyState>,
}

impl fmt::Debug for RwQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwQueue")
            .field("on_access_granted", &"OnAccessGranted")
            .field("key_states", &self.key_states)
            .finish()
    }
}

impl RwQueue {
    /// Create a new key if it does not already exist.
    pub fn create(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), RwQueueError> {
        if self.key_states.contains_key(&key_id) {
            return Err(RwQueueError::KeyExists(key_id));
        }

        self.key_states.insert(key_id, KeyState::new(proc_id));
        Ok(())
    }

    pub fn delete(&mut self, key_id: KeyId) -> Result<(), RwQueueError> {
        if !self.key_states.contains_key(&key_id) {
            return Err(RwQueueError::KeyNotFound(key_id));
        }

        self.key_states.remove(&key_id);
        Ok(())
    }

    pub fn get_key_state(
        &self,
        key_id: KeyId,
    ) -> Result<&KeyState, RwQueueError> {
        self.key_states
            .get(&key_id)
            .ok_or(RwQueueError::KeyNotFound(key_id))
    }

    fn get_key_state_mut(
        &mut self,
        key_id: KeyId,
    ) -> Result<&mut KeyState, RwQueueError> {
        self.key_states
            .get_mut(&key_id)
            .ok_or(RwQueueError::KeyNotFound(key_id))
    }

    /// Handle requesting queue for processus we could not grant access
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

    pub fn new(on_grant: OnAccessGranted) -> RwQueue {
        RwQueue {
            on_access_granted: on_grant,
            key_states: HashMap::new(),
        }
    }

    /// Release write / read access for a process.
    pub fn release(
        &mut self,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<(), RwQueueError> {
        let key_state = self.get_key_state_mut(key_id)?;

        let writer_release =
            key_state.writer.is_some_and(|writer| writer == proc_id);
        let reader_release = key_state.readers.contains(&proc_id);

        if !writer_release && !reader_release {
            return Err(RwQueueError::ReleaseAccess(proc_id, key_id));
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
    ) -> Result<(), RwQueueError> {
        let key_state = self.get_key_state_mut(key_id)?;

        let requesting_writer = key_state
            .pending_request
            .front()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if key_state.writer.is_some() || requesting_writer {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Reader));
            return Err(RwQueueError::RequestAccess(proc_id, key_id));
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
    ) -> Result<(), RwQueueError> {
        let key_state = self.get_key_state_mut(key_id)?;

        if !key_state.readers.is_empty() || key_state.writer.is_some() {
            key_state
                .pending_request
                .push_back((proc_id, RequestType::Writer));
            return Err(RwQueueError::RequestAccess(proc_id, key_id));
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

    fn create_queue() -> RwQueue {
        RwQueue::new(Box::new(|_, _, _| {}))
    }

    fn create_queue_with_grant(
    ) -> (RwQueue, Sender<OnAccessGranted>, Receiver<bool>) {
        let (fn_tx, fn_rx) = channel::<OnAccessGranted>();
        let (call_tx, call_rx) = channel();
        (
            RwQueue::new(Box::new(move |proc_id, key_id, req_type| {
                fn_rx.try_recv().unwrap()(proc_id, key_id, req_type);
                call_tx.send(true).unwrap();
            })),
            fn_tx,
            call_rx,
        )
    }

    #[test]
    fn test_create() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.get_key_state(0).is_err());
        queue.create(2, 0)?;
        assert_eq!(queue.get_key_state(0)?.creator, 2);
        assert!(queue.create(0, 0).is_err());
        Ok(())
    }

    #[test]
    fn test_delete() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.delete(0).is_err());
        queue.create(0, 0)?;
        queue.delete(0)?;
        assert!(queue.get_key_state(0).is_err());
        Ok(())
    }

    #[test]
    fn test_request_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.request_read(1, 0).is_err());
        queue.create(0, 0)?;
        queue.request_read(1, 0)?;
        queue.request_read(2, 0)?;
        queue.request_read(3, 0)?;
        assert_eq!(queue.get_key_state(0)?.pending_request, vec![]);
        assert_eq!(queue.get_key_state(0)?.readers, HashSet::from([1, 2, 3]));
        Ok(())
    }

    #[test]
    fn test_request_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.request_write(1, 0).is_err());
        queue.create(0, 0)?;
        queue.request_write(1, 0)?;
        assert!(queue.request_write(2, 0).is_err());
        assert_eq!(
            queue.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Writer)]
        );
        assert_eq!(queue.get_key_state(0)?.writer, Some(1));
        Ok(())
    }

    #[test]
    fn test_request_read_before_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.create(0, 0)?;
        queue.request_read(1, 0)?;
        queue.request_read(2, 0)?;
        assert!(queue.request_write(3, 0).is_err());
        assert!(queue.request_read(4, 0).is_err());
        assert_eq!(
            queue.get_key_state(0)?.pending_request,
            vec![(3, RequestType::Writer), (4, RequestType::Reader)]
        );
        Ok(())
    }

    #[test]
    fn test_request_write_before_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.create(0, 0)?;
        queue.request_write(1, 0)?;
        assert!(queue.request_read(2, 0).is_err());
        assert!(queue.request_write(3, 0).is_err());
        assert_eq!(
            queue.get_key_state(0)?.pending_request,
            vec![(2, RequestType::Reader), (3, RequestType::Writer)]
        );
        Ok(())
    }

    #[test]
    fn test_release_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.create(0, 0)?;
        assert!(queue.release(1, 0).is_err());
        queue.request_read(1, 0)?;
        assert!(queue.release(2, 0).is_err());
        queue.release(1, 0)?;
        assert!(queue.get_key_state(0)?.readers.is_empty());
        Ok(())
    }

    #[test]
    fn test_release_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.create(0, 0)?;
        assert!(queue.release(1, 0).is_err());
        queue.request_write(1, 0)?;
        assert!(queue.release(2, 0).is_err());
        queue.release(1, 0)?;
        assert_eq!(queue.get_key_state(0)?.writer, None);
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
    fn test_handling_read_before_write() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        queue.create(0, 0)?;
        queue.request_read(1, 0)?;
        assert!(queue.request_write(2, 0).is_err());
        assert!(queue.request_read(3, 0).is_err());

        assert_on_grant!(fn_tx, 2, 0, RequestType::Writer);
        queue.release(1, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        assert_on_grant!(fn_tx, 3, 0, RequestType::Reader);
        queue.release(2, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    #[test]
    fn test_handling_write_before_read() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        queue.create(0, 0)?;
        queue.request_write(1, 0)?;
        assert!(queue.request_read(2, 0).is_err());
        assert!(queue.request_read(3, 0).is_err());
        assert!(queue.request_read(4, 0).is_err());
        assert!(queue.request_write(5, 0).is_err());

        for i in 2..=4 {
            assert_on_grant!(fn_tx, i, 0, RequestType::Reader);
        }
        queue.release(1, 0)?;
        assert_eq!(call_rx.try_iter().count(), 3);

        assert_on_grant!(fn_tx, 5, 0, RequestType::Writer);
        queue.release(2, 0)?;
        queue.release(3, 0)?;
        assert_eq!(call_rx.try_iter().count(), 0);
        queue.release(4, 0)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    // correspond to the report fairness diagram
    #[test]
    fn test_fairness() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        let (x, a, b, c, d) = (0, 1, 2, 3, 4);
        queue.create(a, x)?;
        assert_eq!(queue.get_key_state(x)?.creator, a);

        queue.request_read(a, x)?;
        assert!(queue.request_write(c, x).is_err());
        assert!(queue.request_read(b, x).is_err());
        assert_eq!(
            queue.get_key_state(x)?.pending_request,
            vec![(c, RequestType::Writer), (b, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, c, x, RequestType::Writer);
        queue.release(a, x)?;
        assert_eq!(call_rx.try_iter().count(), 1);
        assert_eq!(
            queue.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Reader)]
        );

        assert!(queue.request_read(d, x).is_err());
        assert_eq!(
            queue.get_key_state(x)?.pending_request,
            vec![(b, RequestType::Reader), (d, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, b, x, RequestType::Reader);
        assert_on_grant!(fn_tx, d, x, RequestType::Reader);
        queue.release(c, 0)?;
        assert_eq!(call_rx.try_iter().count(), 2);

        assert!(queue.get_key_state(x)?.pending_request.is_empty());
        assert_eq!(queue.get_key_state(x)?.readers, HashSet::from([b, d]));
        assert!(queue.get_key_state(x)?.writer.is_none());

        Ok(())
    }
}
