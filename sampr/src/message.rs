use crate::actor::Actor;

use async_trait::async_trait;
use std::fmt;
use tokio::sync::oneshot;

pub trait Message: Send {
    type Result: Send;
}

#[async_trait]
pub trait Handler<M: Message>
where
    Self: Actor,
{
    async fn handle(&mut self, msg: M, ctx: &mut <Self as Actor>::Context) -> M::Result;
}

#[async_trait]
pub trait Deliver<A: Actor> {
    // Deliver a message to its actor.
    //
    // Please note that this should consume self, but cannot do so
    // as the rust compiler cannot determine the size of [EnvelopeWithMessage]
    // due to it hiding in a box behind this trait.
    //
    async fn deliver(&mut self, actor: &mut A, ctx: &mut A::Context);
}

struct Inner<M>
where
    M: Message,
{
    msg: M,
    result_tx: oneshot::Sender<M::Result>,
}

struct EnvelopeWithMessage<M>(Option<Inner<M>>)
where
    M: Message + Send;

impl<M> EnvelopeWithMessage<M>
where
    M: Message + Send,
{
    fn new(msg: M, result_tx: oneshot::Sender<M::Result>) -> Self {
        EnvelopeWithMessage(Some(Inner { msg, result_tx }))
    }
}

#[async_trait]
impl<A, M> Deliver<A> for EnvelopeWithMessage<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn deliver(&mut self, actor: &mut A, ctx: &mut A::Context) {
        let Inner { msg, result_tx } = self.0.take().expect("envelope can only be delivered once");
        let res = <A as Handler<M>>::handle(actor, msg, ctx).await;
        if result_tx.send(res).is_err() {
            log::error!("cannot send result to sender shut down");
        }
    }
}

pub struct Envelope<A: Actor> {
    msg: Box<dyn Deliver<A> + Send>,
}

impl<A: Actor> fmt::Debug for Envelope<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope").finish()
    }
}

impl<A: Actor> Envelope<A> {
    pub fn pack<M>(msg: M, result_tx: oneshot::Sender<M::Result>) -> Self
    where
        A: Handler<M>,
        M: Message + Send + 'static,
    {
        Envelope {
            msg: Box::new(EnvelopeWithMessage::new(msg, result_tx)),
        }
    }
}

#[async_trait]
impl<A: Actor> Deliver<A> for Envelope<A> {
    async fn deliver(&mut self, actor: &mut A, ctx: &mut A::Context) {
        self.msg.deliver(actor, ctx).await;
    }
}
