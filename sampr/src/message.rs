use async_trait::async_trait;
use std::fmt;
use tokio::sync::oneshot;

use crate::{Actor, Context};

/// A message sent to an actor.
///
/// Implement this trait on types that should be sent to an actor. As the object may be moved
/// between threads the type has to be [Send]. The message will be handled asynchronously by the
/// [Actor's](Actor) [Handler<M: Message>] function.
pub trait Message: Send {
    /// The result of the [Actor's](Actor) [Handler<M: Message>] function.
    ///
    /// The result is returned to the sender.
    type Result: Send;
}

impl<T> Message for Option<T>
where
    T: Message,
{
    type Result = T::Result;
}

/// A trait implemented by an actor to handle a specific message.
///
/// Please note that only a single message at a time can be processed by an [Actor]. Awaiting in
/// [Handler::handle] will block the receiving end of the [Actor's](Actor) message queue.
#[async_trait]
pub trait Handler<M: Message>
where
    Self: Actor,
{
    /// Called whenever a type implementing [Message] was received in the [Actor's](Actor)
    /// context. The returned [Message::Result] will be moved to the sender's context
    /// asynchronously.
    async fn handle(&mut self, msg: M, ctx: &mut Context<Self>) -> M::Result;
}

/// Helper trait to encapsulate messages with arbitrary type into an [Envelope].
#[async_trait]
pub trait Deliver<A: Actor> {
    /// Deliver a message to its actor.
    ///
    /// Please note that this should consume self, but cannot do so
    /// as the rust compiler cannot determine the size of [MessageRsvp]
    /// due to it hiding in a box behind this trait.
    async fn deliver(&mut self, actor: &mut A, ctx: &mut Context<A>);
}

/// Inner data of [MessageRsvp].
struct Inner<M: Message> {
    msg: M,
    result_tx: oneshot::Sender<M::Result>,
}

/// A [Message] with return channel to put into an [Envelope].
///
/// As [Deliver::deliver()] does not consume `self`, we take [Inner] out of the `Option`, expecting
/// that the [Envelope] is only delivered once.
struct MessageRsvp<M: Message>(Option<Inner<M>>);

#[async_trait]
impl<A, M> Deliver<A> for MessageRsvp<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn deliver(&mut self, actor: &mut A, ctx: &mut Context<A>) {
        let Inner { msg, result_tx } = self.0.take().expect("Envelope can only be delivered once");
        let res = <A as Handler<M>>::handle(actor, msg, ctx).await;
        if result_tx.send(res).is_err() {
            log::error!("cannot send result to sender shut down");
        }
    }
}

/// A [Message] with no return channel to put into an [Envelope].
///
/// As [Deliver::deliver()] does not consume `self`, we hold an `Option` of a [Message], taking
/// it out on delivery, expecting that the [Envelope] is only delivered once.
struct MessageOneway<M: Message>(Option<M>);

#[async_trait]
impl<A, M> Deliver<A> for MessageOneway<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn deliver(&mut self, actor: &mut A, ctx: &mut Context<A>) {
        let msg = self.0.take().expect("Envelope can only be delivered once");
        <A as Handler<M>>::handle(actor, msg, ctx).await;
    }
}

/// An envelope containing some kind of message to an Actor.
pub enum Envelope<A: Actor> {
    /// Some [Message] sent to the [Actor].
    Message(Box<dyn Deliver<A> + Send>),
    /// Signal for the [Actor] to stop.
    Stop(oneshot::Sender<A>),
}

impl<A: Actor> fmt::Debug for Envelope<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Envelope::*;
        match self {
            Message(_) => write!(f, "Envelope::Message"),
            Stop(_) => write!(f, "Envelope::Stop"),
        }
    }
}

impl<A: Actor> Envelope<A> {
    /// Pack a [Message] and a return channel into an envelope.
    pub fn message_rsvp<M>(msg: M, result_tx: oneshot::Sender<M::Result>) -> Self
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        Envelope::Message(Box::new(MessageRsvp(Some(Inner { msg, result_tx }))))
    }

    /// Pack a [Message] into an envelope.
    pub fn message<M>(msg: M) -> Self
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        Envelope::Message(Box::new(MessageOneway(Some(msg))))
    }

    /// Construct a [Envelope::Stop].
    pub fn stop(result_tx: oneshot::Sender<A>) -> Self {
        Envelope::Stop(result_tx)
    }

    /// Deliver the encapsulated [Message] to the [Actor], calling the respective
    /// [Handler<M: Message] trait.
    ///
    /// # Panic
    ///
    /// This function will panic if called on a [Envelope::Stop] variant.
    pub async fn deliver(&mut self, actor: &mut A, ctx: &mut Context<A>) {
        use Envelope::*;
        match self {
            Message(msg) => msg.deliver(actor, ctx).await,
            Stop(_) => unreachable!("context::Worker will not call deliver on Stop"),
        }
    }
}
