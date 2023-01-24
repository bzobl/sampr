use tokio::sync::{mpsc, oneshot};

use crate::{
    context::{AsyncContext, Context},
    message::{Envelope, Handler, Message},
    Error,
};

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
    pub async fn send<M>(&self, msg: M) -> Result<M::Result, Error>
    where
        A: Handler<M>,
        M: Message + Send,
    {
        let (result_tx, result_rx) = oneshot::channel();
        self.msg_tx.send(Envelope::pack(msg, result_tx)).await?;
        Ok(result_rx.await?)
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
    type Context: AsyncContext;

    fn started(&mut self, _ctx: &mut Self::Context) {}

    fn stopped(&mut self) {}
    fn start(self) -> ActorHandle<Self>
    where
        Self: Actor<Context = Context<Self>>,
    {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (msg_tx, msg_rx) = mpsc::channel(10);

        Context::start(self, msg_rx, shutdown_tx);

        ActorHandle {
            shutdown_rx,
            msg_tx,
        }
    }
}
