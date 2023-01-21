use crate::{
    actor::Actor,
    message::{Deliver, Envelope},
};

use futures::{stream::FuturesUnordered, StreamExt};
use std::{future::Future, pin::Pin};
use tokio::sync::{mpsc, oneshot};

pub trait AsyncContext: Send {}

struct SpawnedCallback<A: Actor> {
    callback: Box<dyn FnOnce(&mut A, &mut A::Context) + Send>,
}

impl<A: Actor> SpawnedCallback<A> {
    fn call(self, actor: &mut A, ctx: &mut A::Context) {
        (self.callback)(actor, ctx)
    }
}

struct Item<A: Actor> {
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
    callback: SpawnedCallback<A>,
}

pub struct Context<A: Actor> {
    spawned: Vec<Item<A>>,
}

impl<A> Context<A>
where
    A: Actor<Context = Self>,
{
    pub(crate) fn start(
        actor: A,
        msg_rx: mpsc::Receiver<Envelope<A>>,
        shutdown_tx: oneshot::Sender<()>,
    ) {
        tokio::task::spawn(async move {
            let mut worker = Worker {
                ctx: Context { spawned: vec![] },
                actor,
                msg_rx,
            };

            worker.run().await;

            if let Err(_e) = shutdown_tx.send(()) {
                log::debug!("user dropped ActorHandle early");
            }
        });
    }
    pub fn spawn<F, C>(&mut self, future: F, callback: C)
    where
        F: Future<Output = ()> + Send + 'static,
        C: FnOnce(&mut A, &mut Self) + Send + 'static,
    {
        self.spawned.push(Item {
            future: Box::pin(future),
            callback: SpawnedCallback {
                callback: Box::new(callback),
            },
        });
    }
}

impl<A> AsyncContext for Context<A> where A: Actor<Context = Self> {}

struct Worker<A: Actor> {
    ctx: A::Context,
    actor: A,
    msg_rx: mpsc::Receiver<Envelope<A>>,
}

impl<A> Worker<A>
where
    A: Actor<Context = Context<A>>,
{
    async fn run(&mut self) {
        self.actor.started();

        let mut spawned = FuturesUnordered::new();

        loop {
            let mut mailbox_closed = false;

            tokio::select! {
                res = spawned.next(), if !spawned.is_empty() => {
                    let callback: SpawnedCallback<A> = match res {
                        Some(cb) => cb,
                        None => unreachable!("is only polled when not empty"),
                    };
                    callback.call(&mut self.actor, &mut self.ctx);
                },
                res = self.msg_rx.recv() => {
                    match res {
                        Some(mut envelope) => {
                            envelope.deliver(&mut self.actor, &mut self.ctx).await;
                            if !self.ctx.spawned.is_empty() {
                                for item in self.ctx.spawned.drain(..) {
                                    spawned.push(async move {
                                        item.future.await;
                                        item.callback
                                    })
                                }
                            }
                        }
                        None => mailbox_closed = true,
                    };
                }
            }

            if mailbox_closed && spawned.is_empty() {
                break;
            }
        }

        self.actor.stopped();
    }
}
