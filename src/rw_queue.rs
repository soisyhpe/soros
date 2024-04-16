use std::{
    collections::{HashSet, VecDeque},
    fmt,
    ops::Deref,
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RwQueueError {
    #[error("Could not grant access request for proc: {0}")]
    RequestAccess(ProcId),
    #[error("Could not release access for proc: {0}")]
    ReleaseAccess(ProcId),
}

type ProcId = usize;

#[derive(Debug, PartialEq, Eq)]
pub enum RequestType {
    Writer,
    Reader,
}

pub type OnAccessGranted = Box<dyn Fn(ProcId, RequestType)>;

pub struct RwQueue {
    on_access_granted: OnAccessGranted,
    pending_request: VecDeque<(ProcId, RequestType)>,
    readers: HashSet<ProcId>,
    writer: Option<ProcId>,
}

impl fmt::Debug for RwQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwQueue")
            .field("on_access_granted", &"OnAccessGranted")
            .field("pending_request", &self.pending_request)
            .field("readers", &self.readers)
            .field("writer", &self.writer)
            .finish()
    }
}

impl RwQueue {
    /// Handle requesting queue for processus we could not grant access
    fn handle_requesting(&mut self) {
        let Some((proc_id, req_type)) = self.pending_request.front() else {
            return;
        };
        match req_type {
            RequestType::Writer => {
                self.on_access_granted.deref()(*proc_id, RequestType::Writer);
                self.register_writer(*proc_id);
                self.pending_request.pop_front();
            }
            RequestType::Reader => {
                while let Some((proc_id, RequestType::Reader)) =
                    self.pending_request.front()
                {
                    self.on_access_granted.deref()(
                        *proc_id,
                        RequestType::Reader,
                    );
                    self.register_reader(*proc_id);
                    self.pending_request.pop_front();
                }
            }
        }
    }

    pub fn new(on_grant: OnAccessGranted) -> RwQueue {
        RwQueue {
            readers: HashSet::new(),
            writer: None,
            pending_request: VecDeque::new(),
            on_access_granted: on_grant,
        }
    }

    /// Release write / read access by a proc.
    pub fn release(&mut self, proc_id: ProcId) -> Result<(), RwQueueError> {
        let writer_release =
            self.writer.is_some_and(|writer| writer == proc_id);
        let reader_release = self.readers.contains(&proc_id);

        if !writer_release && !reader_release {
            return Err(RwQueueError::ReleaseAccess(proc_id));
        }

        if writer_release {
            self.writer = None;
            self.handle_requesting();
        } else if reader_release {
            self.readers.remove(&proc_id);
            if self.readers.is_empty() {
                self.handle_requesting();
            }
        }

        Ok(())
    }

    /// Request read access by a proc.
    /// Multiple read are possible at the same time.
    pub fn request_read(
        &mut self,
        proc_id: ProcId,
    ) -> Result<(), RwQueueError> {
        let requesting_writer = self
            .pending_request
            .front()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if self.writer.is_some() || requesting_writer {
            self.pending_request
                .push_back((proc_id, RequestType::Reader));
            return Err(RwQueueError::RequestAccess(proc_id));
        }

        self.register_reader(proc_id);
        Ok(())
    }

    /// Reqest write access by a proc.
    /// Only one write access at a time.
    pub fn request_write(
        &mut self,
        proc_id: ProcId,
    ) -> Result<(), RwQueueError> {
        if !self.readers.is_empty() || self.writer.is_some() {
            self.pending_request
                .push_back((proc_id, RequestType::Writer));
            return Err(RwQueueError::RequestAccess(proc_id));
        }

        self.register_writer(proc_id);
        Ok(())
    }

    fn register_reader(&mut self, proc_id: ProcId) {
        self.readers.insert(proc_id);
    }

    fn register_writer(&mut self, proc_id: ProcId) {
        self.writer = Some(proc_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::mpsc::{channel, Receiver, Sender};

    fn create_queue() -> RwQueue {
        RwQueue::new(Box::new(|_, _| {}))
    }

    fn create_queue_with_grant(
    ) -> (RwQueue, Sender<OnAccessGranted>, Receiver<bool>) {
        let (fn_tx, fn_rx) = channel::<OnAccessGranted>();
        let (call_tx, call_rx) = channel();
        (
            RwQueue::new(Box::new(move |proc_id, req_type| {
                fn_rx.try_recv().unwrap()(proc_id, req_type);
                call_tx.send(true).unwrap();
            })),
            fn_tx,
            call_rx,
        )
    }

    #[test]
    fn test_request_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.request_read(1)?;
        queue.request_read(2)?;
        queue.request_read(3)?;
        assert_eq!(queue.pending_request, vec![]);
        assert_eq!(queue.readers, HashSet::from([1, 2, 3]));
        Ok(())
    }

    #[test]
    fn test_request_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.request_write(1)?;
        assert!(queue.request_write(2).is_err());
        assert_eq!(queue.pending_request, vec![(2, RequestType::Writer)]);
        assert_eq!(queue.writer, Some(1));
        Ok(())
    }

    #[test]
    fn test_request_read_before_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.request_read(1)?;
        queue.request_read(2)?;
        assert!(queue.request_write(3).is_err());
        assert!(queue.request_read(4).is_err());
        assert_eq!(
            queue.pending_request,
            vec![(3, RequestType::Writer), (4, RequestType::Reader)]
        );
        Ok(())
    }

    #[test]
    fn test_request_write_before_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        queue.request_write(1)?;
        assert!(queue.request_read(2).is_err());
        assert!(queue.request_write(3).is_err());
        assert_eq!(
            queue.pending_request,
            vec![(2, RequestType::Reader), (3, RequestType::Writer)]
        );
        Ok(())
    }

    #[test]
    fn test_release_read() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.release(1).is_err());
        queue.request_read(1)?;
        assert!(queue.release(2).is_err());
        queue.release(1)?;
        assert!(queue.readers.is_empty());
        Ok(())
    }

    #[test]
    fn test_release_write() -> Result<(), RwQueueError> {
        let mut queue = create_queue();
        assert!(queue.release(1).is_err());
        queue.request_write(1)?;
        assert!(queue.release(2).is_err());
        queue.release(1)?;
        assert_eq!(queue.writer, None);
        Ok(())
    }

    macro_rules! assert_on_grant {
        ($tx:expr, $proc:expr, $req:expr) => {
            $tx.send(Box::new(move |proc_id, req_type| {
                assert_eq!((proc_id, req_type), ($proc, $req));
            }))
            .unwrap();
        };
    }

    #[test]
    fn test_handling_read_before_write() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        queue.request_read(1)?;
        assert!(queue.request_write(2).is_err());
        assert!(queue.request_read(3).is_err());

        assert_on_grant!(fn_tx, 2, RequestType::Writer);
        queue.release(1)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        assert_on_grant!(fn_tx, 3, RequestType::Reader);
        queue.release(2)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    #[test]
    fn test_handling_write_before_read() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        queue.request_write(1)?;
        assert!(queue.request_read(2).is_err());
        assert!(queue.request_read(3).is_err());
        assert!(queue.request_read(4).is_err());
        assert!(queue.request_write(5).is_err());

        for i in 2..=4 {
            assert_on_grant!(fn_tx, i, RequestType::Reader);
        }
        queue.release(1)?;
        assert_eq!(call_rx.try_iter().count(), 3);

        assert_on_grant!(fn_tx, 5, RequestType::Writer);
        queue.release(2)?;
        queue.release(3)?;
        assert_eq!(call_rx.try_iter().count(), 0);
        queue.release(4)?;
        assert_eq!(call_rx.try_iter().count(), 1);

        Ok(())
    }

    // correspond to the report fairness diagram
    #[test]
    fn test_fairness() -> Result<(), RwQueueError> {
        let (mut queue, fn_tx, call_rx) = create_queue_with_grant();
        let (a, b, c, d) = (1, 2, 3, 4);

        queue.request_read(a)?;
        assert!(queue.request_write(c).is_err());
        assert!(queue.request_read(b).is_err());
        assert_eq!(
            queue.pending_request,
            vec![(c, RequestType::Writer), (b, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, c, RequestType::Writer);
        queue.release(a)?;
        assert_eq!(call_rx.try_iter().count(), 1);
        assert_eq!(queue.pending_request, vec![(b, RequestType::Reader)]);

        assert!(queue.request_read(d).is_err());
        assert_eq!(
            queue.pending_request,
            vec![(b, RequestType::Reader), (d, RequestType::Reader)]
        );

        assert_on_grant!(fn_tx, b, RequestType::Reader);
        assert_on_grant!(fn_tx, d, RequestType::Reader);
        queue.release(c)?;
        assert_eq!(call_rx.try_iter().count(), 2);

        assert!(queue.pending_request.is_empty());
        assert_eq!(queue.readers, HashSet::from([b, d]));
        assert!(queue.writer.is_none());

        Ok(())
    }
}
