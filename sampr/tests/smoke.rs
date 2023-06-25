use futures::stream::{self, Stream};
use sampr::{async_trait, Actor, Context, Error, Handler, Message};
use std::{default::Default, pin::Pin};
use tokio::sync::{mpsc, oneshot};

#[derive(Default)]
struct TestActor {
    started: usize,
    stopped: usize,
    stream_done: Option<oneshot::Sender<()>>,
    stream_count: usize,
}

impl Actor for TestActor {
    fn started(&mut self, _ctx: &mut Context<Self>) {
        self.started += 1;
    }

    fn stopped(&mut self) {
        self.stopped += 1;
    }
}

#[async_trait]
impl Handler<Noop> for TestActor {
    async fn handle(&mut self, _msg: Noop, _ctx: &mut Context<Self>) {}
}

#[async_trait]
impl Handler<PlusOne> for TestActor {
    async fn handle(&mut self, msg: PlusOne, _ctx: &mut Context<Self>) -> usize {
        msg.0 + 1
    }
}

#[async_trait]
impl Handler<AddStream> for TestActor {
    async fn handle(&mut self, msg: AddStream, ctx: &mut Context<Self>) {
        self.stream_done = Some(msg.done);
        ctx.add_stream(msg.stream);
    }
}

#[async_trait]
impl Handler<Option<StreamItem>> for TestActor {
    async fn handle(&mut self, msg: Option<StreamItem>, _ctx: &mut Context<Self>) {
        if msg.is_some() {
            self.stream_count += 1
        } else {
            self.stream_done.take().unwrap().send(()).unwrap()
        }
    }
}

#[async_trait]
impl Handler<SpawnProducer> for TestActor {
    async fn handle(&mut self, msg: SpawnProducer, ctx: &mut Context<Self>) {
        let SpawnProducer { mut rx, tx, done } = msg;
        ctx.spawn(
            async move {
                while let Some(i) = rx.recv().await {
                    tx.send(i).await.unwrap();
                }
            },
            move |_, _, _| done.send(()).unwrap(),
        );
    }
}

struct Noop;

impl Message for Noop {
    type Result = ();
}

struct PlusOne(usize);

impl Message for PlusOne {
    type Result = usize;
}

struct AddStream {
    done: oneshot::Sender<()>,
    stream: Pin<Box<dyn Stream<Item = StreamItem> + Send>>,
}

impl Message for AddStream {
    type Result = ();
}

struct StreamItem;

impl Message for StreamItem {
    type Result = ();
}

struct SpawnProducer {
    rx: mpsc::Receiver<usize>,
    tx: mpsc::Sender<usize>,
    done: oneshot::Sender<()>,
}

impl Message for SpawnProducer {
    type Result = ();
}

#[tokio::test]
async fn test_actor_start_stop() {
    let addr = TestActor::default().start();

    let actor = addr.stop().await.unwrap();

    assert_eq!(actor.started, 1);
    assert_eq!(actor.stopped, 1);
}

#[tokio::test]
async fn test_actor_send_after_stop() {
    let addr = TestActor::default().start();
    let addr2 = addr.clone();

    assert!(addr2.send(Noop).await.is_ok());

    let actor = addr.stop().await.unwrap();

    let result = addr2.send(Noop).await;
    assert!(matches!(result, Err(Error::ReceiverShutdown)));

    assert_eq!(actor.started, 1);
    assert_eq!(actor.stopped, 1);
}

#[tokio::test]
async fn test_actor_message_result() {
    let addr = TestActor::default().start();

    assert_eq!(addr.send(PlusOne(1)).await.unwrap(), 2);
    assert_eq!(addr.send(PlusOne(2)).await.unwrap(), 3);
    assert_eq!(addr.send(PlusOne(3)).await.unwrap(), 4);
    assert_eq!(addr.send(PlusOne(4)).await.unwrap(), 5);

    addr.stop().await.unwrap();
}

#[tokio::test]
async fn test_actor_stream() {
    let addr = TestActor::default().start();

    let stream = stream::iter(vec![StreamItem, StreamItem, StreamItem, StreamItem]);
    let (done, done_rx) = oneshot::channel();

    addr.send(AddStream {
        done,
        stream: Box::pin(stream),
    })
    .await
    .unwrap();

    done_rx.await.unwrap();
    let actor = addr.stop().await.unwrap();
    assert_eq!(actor.stream_count, 4);
}

#[tokio::test]
async fn test_actor_spawn() {
    let addr = TestActor::default().start();

    let (out_tx, mut out_rx) = mpsc::channel(1);
    let (in_tx, in_rx) = mpsc::channel(1);
    let (done_tx, mut done_rx) = oneshot::channel();

    addr.send(SpawnProducer {
        rx: in_rx,
        tx: out_tx,
        done: done_tx,
    })
    .await
    .unwrap();

    in_tx.send(0).await.unwrap();
    assert_eq!(out_rx.recv().await.unwrap(), 0);

    // spawn should not block other messages
    assert_eq!(addr.send(PlusOne(0)).await.unwrap(), 1);
    assert_eq!(addr.send(PlusOne(0)).await.unwrap(), 1);
    assert_eq!(addr.send(PlusOne(0)).await.unwrap(), 1);

    for i in 0..10 {
        in_tx.send(i).await.unwrap();
        tokio::select! {
            res = out_rx.recv() => {
                assert_eq!(res.unwrap(), i);
            }
            _ = &mut done_rx => {
                panic!("done should not fire before spawned task has finished");
            }
        }
    }

    drop(in_tx);
    done_rx.await.unwrap();

    addr.stop().await.unwrap();
}
