use tokio::sync::{mpsc, oneshot};

use crate::{
    context::Context,
    message::{Envelope, Handler, Message},
    Error,
};

/// Address of an actor.
#[derive(Debug)]
pub struct Addr<A: Actor> {
    msg_tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Addr {
            msg_tx: self.msg_tx.clone(),
        }
    }
}

impl<A: Actor> Addr<A> {
    /// Send a message to the actor this address is pointing to.
    ///
    /// The message's result is returned when awaiting this function after the receiving
    /// actor's handler has processed the message.
    ///
    /// # Errors
    ///
    /// [Error::ReceiverShutdown] if the receiving actor has stopped.
    pub async fn send<M>(&self, msg: M) -> Result<M::Result, Error>
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        let (result_tx, result_rx) = oneshot::channel();
        self.msg_tx
            .send(Envelope::pack(msg, result_tx))
            .await
            .map_err(|_| Error::ReceiverShutdown)?;
        Ok(result_rx.await.map_err(|_| Error::ReceiverShutdown)?)
    }

    pub(crate) async fn send_nowait<M>(&self, msg: M) -> Result<(), Error>
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        let (result_tx, _result_rx) = oneshot::channel();
        self.msg_tx
            .send(Envelope::pack(msg, result_tx))
            .await
            .map_err(|_| Error::ReceiverShutdown)
    }

    /// Stop the actor
    ///
    /// This will remove the actor from its background task and moves it back to the caller's
    /// task.
    ///
    /// Any attempt to send messages to this Actor using [Addr::send()] will fail with
    /// [Error::ReceiverShutdown].
    pub async fn stop(self) -> Result<A, Error> {
        let (tx, rx) = oneshot::channel();
        self.msg_tx
            .send(Envelope::stop(tx))
            .await
            .map_err(|_| Error::ReceiverShutdown)?;
        Ok(rx.await.map_err(|_| Error::ReceiverShutdown)?)
    }
}

/// An asynchronous actor.
///
/// An arbitrary type implementing the [Actor] trait will become a _sampr_ actor. Use
/// [Actor::start()] to start receiving messages for the actor. Once started, the actor
/// object will be moved to a [tokio::task](https://docs.rs/tokio/latest/tokio/task/index.html).
///
/// The application should then use messages to interact with this actor. Messages can be sent
/// using this actor's [Addr]. Access to the actor while processing those messages will be granted
/// through a mutable reference in the respective handler functions and callbacks. Additionally,
/// those will have a reference to the actor's [Context] at hand to interact with the actor
/// and the actor's [tokio::task].
///
/// # Example
///
/// ```
/// struct MyActor;
///
/// impl sampr::Actor for MyActor {}
/// ```
pub trait Actor: Sized + Send + 'static {
    /// Called in the context of the actor's `tokio::task` when the actor is started .
    ///
    /// The default implementation of this function does nothing.
    fn started(&mut self, _ctx: &mut Context<Self>) {}

    /// Called in the context of the actor's `tokio::task` when the actor is stopped.
    ///
    /// The default implementation of this function does nothing.
    fn stopped(&mut self) {}

    /// Start the actor.
    ///
    /// By starting, the actor object is moved to its own `tokio::task` and starts receiving
    /// messages.
    ///
    /// **Please note**: The underlying actor's message queue (i.e., a [mpsc::channel()]) is
    /// currently limited to 10 messages. This, however, is not a limiting factor as sending a
    /// message through [Addr::send()] will await the message's result anyways, hence the sending
    /// actor will wait regardless of whether the receiver's message queue has reached its capacity.
    fn start(self) -> Addr<Self> {
        let (msg_tx, msg_rx) = mpsc::channel(10);

        let addr = Addr { msg_tx };

        // TODO by cloning msg_tx into Context, the actor will not stop automatically
        Context::start(self, addr.clone(), msg_rx);

        addr
    }
}
