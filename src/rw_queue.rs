use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RwQueueError {
    #[error("Could not grant read request for proc: {0}")]
    ReqRead(ProcId),
    #[error("Could not grant write request for proc: {0}")]
    ReqWrite(ProcId),
}

type ProcId = usize;

#[derive(Debug, PartialEq, Eq)]
enum RequestType {
    Writer,
    Reader,
}

#[derive(Debug)]
pub struct RwQueue {
    readers: u32,
    writer: Option<ProcId>,
    requesting: Vec<(ProcId, RequestType)>,
}

impl RwQueue {
    pub fn new() -> RwQueue {
        RwQueue {
            readers: 0,
            writer: None,
            requesting: Vec::new(),
        }
    }

    pub fn req_read(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        let requesting_writer = self
            .requesting
            .first()
            .is_some_and(|req| matches!(req.1, RequestType::Writer));

        if self.writer.is_some() || requesting_writer {
            self.requesting.push((proc, RequestType::Reader));
            return Err(RwQueueError::ReqRead(proc));
        }

        self.readers += 1;
        Ok(())
    }

    pub fn req_write(&mut self, proc: ProcId) -> Result<(), RwQueueError> {
        if self.readers != 0 || self.writer.is_some() {
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
    fn test_simple_read() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_read(1)?;
        queue.req_read(2)?;
        queue.req_read(3)?;
        assert_eq!(queue.requesting, vec![]);
        Ok(())
    }

    #[test]
    fn test_simple_write() -> Result<(), RwQueueError> {
        let mut queue = RwQueue::new();
        queue.req_write(1)?;
        assert!(queue.req_write(2).is_err());
        assert_eq!(queue.requesting, vec![(2, RequestType::Writer)]);
        Ok(())
    }

    #[test]
    fn test_read_before_write() -> Result<(), RwQueueError> {
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
    fn test_write_before_read() -> Result<(), RwQueueError> {
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
}
