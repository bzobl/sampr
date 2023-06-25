use async_trait::async_trait;
use futures::{
    ready,
    stream::{SelectAll, Stream, StreamExt},
    task::{self, Poll},
    Future, FutureExt,
};
use std::pin::Pin;
use tokio::sync::mpsc;

use crate::{message::Envelope, Actor, Addr, Handler, Message};

/// Context for interacting with an [Actor's](Actor) task.
pub struct Context<A: Actor> {
    addr: Addr<A>,
    tasks: Vec<Box<dyn ActorTask<A>>>,
}

impl<A> Context<A>
where
    A: Actor,
{
    /// Start an actor, moving it into its own `tokio::task`.
    ///
    /// The caller can move the [Actor] back to their task using [Addr::stop()].
    pub(crate) fn start(actor: A, addr: Addr<A>, msg_rx: mpsc::Receiver<Envelope<A>>) {
        tokio::task::spawn(async move {
            let worker = Worker {
                ctx: Context {
                    addr,
                    tasks: vec![],
                },
                actor,
                msg_rx,
            };

            worker.run().await;
        });
    }

    fn drain_tasks(&mut self, async_items: &mut SelectAll<Box<dyn ActorTask<A>>>) {
        for item in self.tasks.drain(..) {
            async_items.push(item);
        }
    }

    /// Spawn a future to be worked on in the background. Once `future` resolves `callback` will be
    /// called with the [Actor] and its [Context] as parameters.
    pub fn spawn<F, C, O>(&mut self, future: F, callback: C)
    where
        F: Future<Output = O> + Send + 'static,
        C: FnOnce(O, &mut A, &mut Self) + Send + 'static,
        O: Send + 'static,
    {
        self.tasks.push(Box::new(Spawned {
            future: Box::pin(future),
            callback: Some(Box::new(callback)),
        }));
    }

    /// Add a stream to be polled to the [Actor's](Actor) [Context]. Every time `stream` produces
    /// an item the [Actor's](Actor) [Handler<Option<_>>] implementation is called.
    pub fn add_stream<S>(&mut self, stream: S)
    where
        S: Stream + Send + Unpin + 'static,
        S::Item: Message,
        A: Handler<Option<<S as Stream>::Item>>,
    {
        self.tasks.push(Box::new(ActorStream {
            stream: Some(stream),
            addr: self.addr.clone(),
        }));
    }
}

/// An future that is polled in the [Actor's](Actor) [Context] after being added through
/// [Context::spawn()] or [Context::add_stream()].
///
/// The indirection via this trait is necessary as the concrete [Item] has is generic over
/// `Output`.
///
/// Types implementing [ActorTask] will have to implement [Stream] producing a [TaskOutput] which
/// will be used by [Worker] to call user-provided callbacks or the [Actor's](Actor)
/// [StreamHandler::handle()] function for the stream's output type.
trait ActorTask<A: Actor>: Stream<Item = Box<dyn TaskOutput<A>>> + Send + Unpin {}

/// Callback called to enter user-defined code after a [ActorTask] has produced output.
///
/// The indirection via this trait is necessary as the concrete output structs are generic over
/// their `Output`.
#[async_trait]
trait TaskOutput<A: Actor>: Send {
    async fn call(&mut self, actor: &mut A, ctx: &mut Context<A>);
}

/// An [ActorTask] created for async tasks spawned through [Context::spawn()].
///
/// Once `future` completes, the [Worker] will call the user-provided `callback` in the
/// [Actor's](Actor) [Context].
struct Spawned<A: Actor, O: Send> {
    future: Pin<Box<dyn Future<Output = O> + Send>>,
    callback: Option<Box<dyn FnOnce(O, &mut A, &mut Context<A>) + Send>>,
}

impl<A: Actor, O: Send + 'static> ActorTask<A> for Spawned<A, O> {}

impl<A: Actor, O: Send + 'static> Stream for Spawned<A, O> {
    type Item = Box<dyn TaskOutput<A>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        if self.callback.is_none() {
            return Poll::Ready(None);
        }

        let mut s = self.as_mut();

        let result = ready!(s.future.poll_unpin(cx));

        Poll::Ready(Some(Box::new(SpawnedOutput::<A, O> {
            result: Some(result),
            callback: s.callback.take(),
        })))
    }
}

/// Concrete type for [TaskOutput] for [Spawned], a task spawned in an
/// [Actor's](Actor) context.
///
/// `result` and `callback` are [Options](Option) because the
/// [TaskOutput] trait cannot be [Sized], hence we cannot take consume
/// `self` in the trait function, hence we move `result` and `callback`
/// out of [SpawnedOutput] when [TaskOutput::call()] is called. We rely
/// that this function is only called once.
struct SpawnedOutput<A: Actor, O: Send> {
    result: Option<O>,
    callback: Option<Box<dyn FnOnce(O, &mut A, &mut Context<A>) + Send>>,
}

#[async_trait]
impl<A: Actor, O: Send> TaskOutput<A> for SpawnedOutput<A, O> {
    async fn call(&mut self, actor: &mut A, ctx: &mut Context<A>) {
        // This function should not await in here.
        // Doing so would starve all of the actor's context's futures.
        let callback = self.callback.take().expect("call() is only called once");
        (callback)(
            self.result.take().expect("call() is only called once"),
            actor,
            ctx,
        )
    }
}

struct ActorStream<A, S>
where
    A: Actor,
    A: Handler<Option<<S as Stream>::Item>>,
    S: Stream + Send + Unpin + 'static,
    S::Item: Message,
{
    stream: Option<S>,
    addr: Addr<A>,
}

impl<A, S> ActorTask<A> for ActorStream<A, S>
where
    A: Actor,
    A: Handler<Option<<S as Stream>::Item>>,
    S: Stream + Send + Unpin + 'static,
    S::Item: Message,
{
}

impl<A, S> Stream for ActorStream<A, S>
where
    A: Actor,
    A: Handler<Option<<S as Stream>::Item>>,
    S: Stream + Send + Unpin + 'static,
    S::Item: Message,
{
    type Item = Box<dyn TaskOutput<A>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        if self.stream.is_none() {
            return Poll::Ready(None);
        }

        let mut s = self.as_mut();
        let item = ready!(s
            .stream
            .as_mut()
            .expect("checked above")
            .poll_next_unpin(cx));
        if item.is_none() {
            s.stream = None;
        }

        Poll::Ready(Some(Box::new(StreamOutput {
            item,
            addr: self.addr.clone(),
        })))
    }
}

struct StreamOutput<A: Actor, O: Send + Message + 'static> {
    item: Option<O>,
    addr: Addr<A>,
}

#[async_trait]
impl<A: Actor + Handler<Option<O>>, O: Send + Message + 'static> TaskOutput<A>
    for StreamOutput<A, O>
{
    async fn call(&mut self, _actor: &mut A, _ctx: &mut Context<A>) {
        if let Err(e) = self.addr.send_and_forget(self.item.take()).await {
            log::warn!("discarding item from stream: {e}");
        }
    }
}

struct Worker<A: Actor> {
    ctx: Context<A>,
    actor: A,
    msg_rx: mpsc::Receiver<Envelope<A>>,
}

impl<A> Worker<A>
where
    A: Actor,
{
    async fn run(mut self) {
        let mut async_items = SelectAll::new();

        self.actor.started(&mut self.ctx);

        let mut mailbox_closed = false;
        loop {
            if mailbox_closed && async_items.is_empty() {
                break;
            }

            self.ctx.drain_tasks(&mut async_items);

            let mut envelope = tokio::select! {
                res = async_items.next(), if !async_items.is_empty() => {
                    if let Some(mut task_output) = res {
                        task_output.call(&mut self.actor, &mut self.ctx).await;
                        continue;
                    } else {
                        log::warn!("some stream finished in outer loop?");
                        continue;
                    }
                },
                res = self.msg_rx.recv(), if !mailbox_closed => {
                    match res {
                        Some(envelope) => envelope,
                        None => {
                            mailbox_closed = true;
                            continue;
                        },
                    }
                }
            };

            if let Envelope::Stop(tx) = envelope {
                self.actor.stopped();
                if tx.send(self.actor).is_err() {
                    log::warn!("sending actor back to stop failed");
                }
                return;
            }

            // This is split in two loops as an Actor can only process
            // on Message at a time). Therefore, msg_rx is not polled while
            // delivering a message. Nevertheless, all async_items have
            // to be polled, hence the duplication.
            loop {
                self.ctx.drain_tasks(&mut async_items);

                tokio::select! {
                    res = async_items.next(), if !async_items.is_empty() => {
                        if let Some(mut task_output) = res {
                            task_output.call(&mut self.actor, &mut self.ctx).await;
                        } else {
                            log::warn!("some stream finished in inner loop?");
                        }
                    },
                    _ = envelope.deliver(&mut self.actor, &mut self.ctx) => {
                        break;
                    }
                }
            }
        }

        self.actor.stopped();
    }
}
