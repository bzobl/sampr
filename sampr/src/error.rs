use thiserror::Error;

#[derive(Error, Debug)]
/// Error type for the _sampr_ crate.
pub enum SamprError {
    /// Receiving actor has stopped before a sent message has returned a result.
    ///
    /// The receiving actor might or might not have started to process the message.
    #[error("receiver has stopped")]
    ReceiverShutdown,
}
