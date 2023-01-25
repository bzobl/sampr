use crate::{
    actor::Actor,
    message::{Deliver, Envelope},
};

use async_trait::async_trait;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use std::{future::Future, pin::Pin};
use tokio::sync::mpsc;

pub trait AsyncContext: Send {}

pub struct Context<A: Actor> {
    spawned: Vec<Box<dyn AsyncItem<A>>>,
}

impl<A> Context<A>
where
    A: Actor<Context = Self>,
{
    pub(crate) fn start(actor: A, msg_rx: mpsc::Receiver<Envelope<A>>) {
        tokio::task::spawn(async move {
            let mut worker = Worker {
                ctx: Context { spawned: vec![] },
                actor,
                msg_rx,
            };

            worker.run().await;
        });
    }

    pub fn spawn<F, C, O>(&mut self, future: F, callback: C)
    where
        F: Future<Output = O> + Send + 'static,
        C: FnOnce(O, &mut A, &mut Self) + Send + 'static,
        O: Send + 'static,
    {
        self.spawned.push(Box::new(Item {
            future: Some(Box::pin(future)),
            callback: Some(Box::new(callback)),
        }));
    }
}

impl<A> AsyncContext for Context<A> where A: Actor<Context = Self> {}

#[async_trait]
trait AsyncItem<A: Actor>: Send {
    async fn work(&mut self) -> Box<dyn AsyncCallback<A>>;
}

struct Item<A: Actor, O: Send> {
    future: Option<Pin<Box<dyn Future<Output = O> + Send>>>,
    callback: Option<Box<dyn FnOnce(O, &mut A, &mut A::Context) + Send>>,
}

#[async_trait]
impl<A: Actor, O: Send + 'static> AsyncItem<A> for Item<A, O> {
    async fn work(&mut self) -> Box<dyn AsyncCallback<A>> {
        let result = self.future.take().unwrap().await;
        Box::new(SpawnedCallback {
            result: Some(result),
            callback: Some(self.callback.take().unwrap()),
        })
    }
}

trait AsyncCallback<A: Actor>: Send {
    fn call(&mut self, actor: &mut A, ctx: &mut A::Context);
}

struct SpawnedCallback<A: Actor, O: Send> {
    result: Option<O>,
    callback: Option<Box<dyn FnOnce(O, &mut A, &mut A::Context) + Send>>,
}

impl<A: Actor, O: Send> AsyncCallback<A> for SpawnedCallback<A, O> {
    fn call(&mut self, actor: &mut A, ctx: &mut A::Context) {
        let callback = self.callback.take().unwrap();
        (callback)(self.result.take().unwrap(), actor, ctx)
    }
}

enum Work<A: Actor> {
    Message(Envelope<A>),
    Callback(Box<dyn AsyncCallback<A>>),
}

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
        let mut spawned = FuturesUnordered::new();

        self.actor.started(&mut self.ctx);
        for mut item in self.ctx.spawned.drain(..) {
            spawned.push(async move { item.work().await }.boxed());
        }

        let mut mailbox_closed = false;
        loop {
            if mailbox_closed && spawned.is_empty() {
                break;
            }

            let work = tokio::select! {
                res = spawned.next(), if !spawned.is_empty() => {
                    Work::Callback(res.expect("is only polled when not empty"))
                },
                res = self.msg_rx.recv(), if !mailbox_closed => {
                    match res {
                        Some(envelope) => {
                            Work::Message(envelope)
                        }
                        None => {
                            mailbox_closed = true;
                            continue;
                        },
                    }
                }
            };

            let mut envelope = match work {
                Work::Callback(mut callback) => {
                    // TODO: should maybe be async
                    callback.call(&mut self.actor, &mut self.ctx);
                    for mut item in self.ctx.spawned.drain(..) {
                        spawned.push(async move { item.work().await }.boxed());
                    }
                    continue;
                }
                Work::Message(envelope) => envelope,
            };

            loop {
                tokio::select! {
                    res = spawned.next(), if !spawned.is_empty() => {
                        res.expect("is only polled when not empty")
                            .call(&mut self.actor, &mut self.ctx);
                        for mut item in self.ctx.spawned.drain(..) {
                            spawned.push(async move { item.work().await }.boxed());
                        }
                        continue;
                    },
                    _ = envelope.deliver(&mut self.actor, &mut self.ctx) => {
                        for mut item in self.ctx.spawned.drain(..) {
                            spawned.push(async move { item.work().await }.boxed());
                        }
                        break;
                    }
                }
            }
        }

        self.actor.stopped();
    }
}
