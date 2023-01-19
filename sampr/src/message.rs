use crate::actor::Actor;

use std::fmt;

pub trait Message: Send + 'static {}

pub trait Handler<M: Message> {
    fn handle(&mut self, msg: M);
}

pub trait Unpackable<A: Actor> {
    fn handle_message(&mut self, actor: &mut A);
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

impl<A, M> Unpackable<A> for InnerEnvelope<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle_message(&mut self, actor: &mut A) {
        let msg = self.msg.take().unwrap();
        <A as Handler<M>>::handle(actor, msg);
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

impl<A: Actor> Unpackable<A> for Envelope<A> {
    fn handle_message(&mut self, actor: &mut A) {
        log::info!("handling message in inner envelope");
        self.msg.handle_message(actor)
    }
}
