use tokio::sync::{mpsc, oneshot};

use crate::{
    context::Context,
    message::{Envelope, Handler, Message},
};

pub struct Addr<A: Actor> {
    msg_tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Addr<A> {
    pub async fn send<M>(&self, msg: M)
    where
        A: Handler<M>,
        M: Message + Send,
    {
        self.msg_tx.send(Envelope::pack(msg)).await.unwrap();
    }
}

pub struct ActorHandle<A: Actor> {
    shutdown_rx: oneshot::Receiver<()>,
    msg_tx: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> ActorHandle<A> {
    pub async fn wait_for_shutdown(self) {
        let ActorHandle {
            shutdown_rx,
            msg_tx,
        } = self;
        drop(msg_tx);
        shutdown_rx.await.unwrap();
    }

    pub fn address(&self) -> Addr<A> {
        Addr {
            msg_tx: self.msg_tx.clone(),
        }
    }
}

pub trait Actor: Sized + Send + 'static {
    fn started(&mut self);
    fn stopped(&mut self);

    fn start(self) -> ActorHandle<Self> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (msg_tx, msg_rx) = mpsc::channel(10);

        let ctx = Context::new(self, msg_rx);
        ctx.spawn(shutdown_tx);

        ActorHandle {
            shutdown_rx,
            msg_tx,
        }
    }
}
