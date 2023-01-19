use crate::{
    actor::Actor,
    message::{Envelope, Unpackable},
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

            shutdown_tx.send(());
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
            envelope.handle_message(&mut self.actor);
        }
    }
}
