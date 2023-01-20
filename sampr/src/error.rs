use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

#[derive(Error, Debug)]
pub enum SamprError {
    #[error("receiver has shut down")]
    ReceiverShutdown,
}

impl<T> From<mpsc::error::SendError<T>> for SamprError {
    fn from(_error: mpsc::error::SendError<T>) -> Self {
        SamprError::ReceiverShutdown
    }
}

impl From<oneshot::error::RecvError> for SamprError {
    fn from(_error: oneshot::error::RecvError) -> Self {
        SamprError::ReceiverShutdown
    }
}
