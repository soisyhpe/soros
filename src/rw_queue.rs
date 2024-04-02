use std::collections::HashSet;
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
enum RequestType {
    Writer,
    Reader,
}

type OnAccessGranted = fn(ProcId);

#[derive(Debug)]
pub struct RwQueue {
    readers: HashSet<ProcId>,
    writer: Option<ProcId>,
    pending_request: Vec<(ProcId, RequestType)>,
    on_access_granted: OnAccessGranted,
}

impl RwQueue {
    pub fn new(on_grant: OnAccessGranted) -> RwQueue {
        RwQueue {
            readers: HashSet::new(),
            writer: None,
            pending_request: Vec::new(),
            on_access_granted: on_grant,
        }
    }

    /// Handle requesting queue for processus we could not grant access
    fn handle_requesting(&mut self) {}

    /// Release write / read access by a proc.
    pub fn release(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        let writer_release = self.writer.is_some_and(|writer| writer == proc);
        let reader_release = self.readers.contains(&proc);

        if !writer_release && !reader_release {
            return Err(RwQueueError::ReleaseAccess(proc));
        }

        if writer_release {
            self.writer = None;
        } else if reader_release {
            self.readers.remove(&proc);
        }

        self.handle_requesting();
        Ok(())
    }

    /// Request read access by a proc.
    /// Multiple read are possible at the same time.
    pub fn request_read(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        let requesting_writer = self
            .pending_request
            .first()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if self.writer.is_some() || requesting_writer {
            self.pending_request.push((proc, RequestType::Reader));
            return Err(RwQueueError::RequestAccess(proc));
        }

        self.readers.insert(proc);
        Ok(())
    }

    /// Reqest write access by a proc.
    /// Only one write access at a time.
    pub fn request_write(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        if !self.readers.is_empty() || self.writer.is_some() {
            self.pending_request.push((proc, RequestType::Writer));
            return Err(RwQueueError::RequestAccess(proc));
        }

        self.writer = Some(proc);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_queue() -> RwQueue {
        RwQueue::new(|_| {})
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
}
