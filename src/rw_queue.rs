use std::collections::HashSet;
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RwQueueError {
    #[error("Could not grant read access request for proc: {0}")]
    ReqRead(ProcId),
    #[error("Could not grant write access request for proc: {0}")]
    ReqWrite(ProcId),
    #[error("Could not release read access for proc: {0}")]
    RelRead(ProcId),
    #[error("Could not release write access for proc: {0}")]
    RelWrite(ProcId),
}

type ProcId = usize;

#[derive(Debug, PartialEq, Eq)]
enum RequestType {
    Writer,
    Reader,
}

#[derive(Debug)]
pub struct RwQueue {
    readers: HashSet<ProcId>,
    writer: Option<ProcId>,
    requesting: Vec<(ProcId, RequestType)>,
}

impl RwQueue {
    pub fn new() -> RwQueue {
        RwQueue {
            readers: HashSet::new(),
            writer: None,
            requesting: Vec::new(),
        }
    }

    /// Handle requesting queue for processus we could not grant access
    fn handle_requesting(&mut self) {}

    /// Release read access by a proc.
    pub fn rel_read(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        if !self.readers.contains(&proc) {
            return Err(RwQueueError::RelRead(proc));
        }

        self.readers.remove(&proc);
        Ok(())
    }

    /// Release write access by a proc.
    pub fn rel_write(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        if !self.writer.is_some_and(|writer| writer == proc) {
            return Err(RwQueueError::RelWrite(proc));
        }

        self.writer = None;
        self.handle_requesting();
        Ok(())
    }

    /// Request read access by a proc.
    /// Multiple read are possible at the same time.
    pub fn req_read(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        let requesting_writer = self
            .requesting
            .first()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if self.writer.is_some() || requesting_writer {
            self.requesting.push((proc, RequestType::Reader));
            return Err(RwQueueError::ReqRead(proc));
        }

        self.readers.insert(proc);
        Ok(())
    }

    /// Reqest write access by a proc.
    /// Only one write access at a time.
    pub fn req_write(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        if !self.readers.is_empty() || self.writer.is_some() {
            self.requesting.push((proc, RequestType::Writer));
            return Err(RwQueueError::ReqWrite(proc));
        }

        self.writer = Some(proc);
        Ok(())
    }
}

impl Default for RwQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_req_read() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_read(1)?;
        queue.req_read(2)?;
        queue.req_read(3)?;
        assert_eq!(queue.requesting, vec![]);
        assert_eq!(queue.readers, HashSet::from([1, 2, 3]));
        Ok(())
    }

    #[test]
    fn test_req_write() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_write(1)?;
        assert!(queue.req_write(2).is_err());
        assert_eq!(queue.requesting, vec![(2, RequestType::Writer)]);
        assert_eq!(queue.writer, Some(1));
        Ok(())
    }

    #[test]
    fn test_req_read_before_write() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_read(1)?;
        queue.req_read(2)?;
        assert!(queue.req_write(3).is_err());
        assert!(queue.req_read(4).is_err());
        assert_eq!(
            queue.requesting,
            vec![(3, RequestType::Writer), (4, RequestType::Reader)]
        );
        Ok(())
    }

    #[test]
    fn test_req_write_before_read() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_write(1)?;
        assert!(queue.req_read(2).is_err());
        assert!(queue.req_write(3).is_err());
        assert_eq!(
            queue.requesting,
            vec![(2, RequestType::Reader), (3, RequestType::Writer)]
        );
        Ok(())
    }

    #[test]
    fn test_rel_read() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        assert!(queue.rel_read(1).is_err());
        queue.req_read(1)?;
        assert!(queue.rel_read(2).is_err());
        queue.rel_read(1)?;
        assert!(queue.readers.is_empty());
        Ok(())
    }

    #[test]
    fn test_rel_write() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        assert!(queue.rel_write(1).is_err());
        queue.req_write(1)?;
        assert!(queue.rel_write(2).is_err());
        queue.rel_write(1)?;
        assert_eq!(queue.writer, None);
        Ok(())
    }
}
