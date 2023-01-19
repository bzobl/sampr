use crate::actor::Actor;

use async_trait::async_trait;
use std::fmt;

pub trait Message: Send + 'static {}

#[async_trait]
pub trait Handler<M: Message> {
    async fn handle(&mut self, msg: M);
}

#[async_trait]
pub trait Unpackable<A: Actor> {
    async fn handle_message(&mut self, actor: &mut A);
}

struct InnerEnvelope<M>
where
    M: Message + Send,
{
    msg: Option<M>,
}

impl<M: Message + Send> InnerEnvelope<M> {
    fn new(msg: M) -> Self {
        InnerEnvelope { msg: Some(msg) }
    }
}

#[async_trait]
impl<A, M> Unpackable<A> for InnerEnvelope<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    async fn handle_message(&mut self, actor: &mut A) {
        let msg = self.msg.take().unwrap();
        <A as Handler<M>>::handle(actor, msg).await
    }
}

pub struct Envelope<A: Actor> {
    msg: Box<dyn Unpackable<A> + Send>,
}

impl<A: Actor> fmt::Debug for Envelope<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope").finish()
    }
}

impl<A: Actor> Envelope<A> {
    pub fn pack<M>(msg: M) -> Self
    where
        A: Handler<M>,
        M: Message + Send + 'static,
    {
        Envelope {
            msg: Box::new(InnerEnvelope::new(msg)),
        }
    }
}

#[async_trait]
impl<A: Actor> Unpackable<A> for Envelope<A> {
    async fn handle_message(&mut self, actor: &mut A) {
        self.msg.handle_message(actor).await
    }
}
