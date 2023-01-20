use crate::{
    actor::Actor,
    message::{Deliver, Envelope},
};

use tokio::sync::{mpsc, oneshot};

pub struct Context<A: Actor> {
    actor: A,
    msg_rx: mpsc::Receiver<Envelope<A>>,
}

impl<A: Actor> Context<A> {
    pub fn new(actor: A, msg_rx: mpsc::Receiver<Envelope<A>>) -> Self {
        Context { actor, msg_rx }
    }

    pub fn spawn(mut self, shutdown_tx: oneshot::Sender<()>) {
        tokio::task::spawn(async move {
            self.actor.started();

            self.run().await;

            self.actor.stopped();

            if let Err(_e) = shutdown_tx.send(()) {
                log::debug!("user dropped ActorHandle early");
            }
        });
    }

    async fn run(&mut self) {
        loop {
            let mut envelope = match self.msg_rx.recv().await {
                Some(envelope) => envelope,
                None => {
                    break;
                }
            };
            envelope.deliver(&mut self.actor).await;
        }
    }
}
