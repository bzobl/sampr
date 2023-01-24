use thiserror::Error;

#[derive(Error, Debug)]
pub enum SamprError {
    #[error("receiver has stopped")]
    ReceiverShutdown,
}
