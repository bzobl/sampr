use sampr::{async_trait, Actor, Addr, Context, Error, Handler, Message};

use futures::stream;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Default)]
struct Writer {
    count: u32,
}

impl Actor for Writer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        log::info!("Writer has started");
        let v: Vec<StreamMsg> = vec!["I".into(), "have".into(), "a".into(), "stream".into()];
        let stream = stream::iter(v);

        ctx.add_stream(stream);
    }

    fn stopped(&mut self) {
        log::info!("Writer has stopped. Sent {} messages in total", self.count);
    }
}

#[async_trait]
impl Handler<OutputMsg> for Writer {
    async fn handle(&mut self, msg: OutputMsg, _ctx: &mut Context<Self>) -> u32 {
        log::info!("writer says (count={}): {}", self.count, msg.0);
        self.count += 1;
        self.count
    }
}

#[async_trait]
impl Handler<Option<StreamMsg>> for Writer {
    async fn handle(&mut self, msg: Option<StreamMsg>, _ctx: &mut Context<Self>) {
        if let Some(StreamMsg(msg)) = msg {
            log::info!("writer got a String: {}", msg);
        } else {
            log::info!("and now my stream has ended");
        }
    }
}

#[derive(Default)]
struct Generator(Option<String>);

impl Actor for Generator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        log::info!("Generator has started");
        ctx.spawn(
            async {
                for i in 0..10 {
                    log::info!("Generator sleep #{i}");
                    sleep(Duration::from_secs(1)).await;
                }
            },
            |_, _, _| log::info!("Generator's sleep tasks is done"),
        )
    }

    fn stopped(&mut self) {
        log::info!("Generator has stopped: {:?}", self.0);
    }
}

#[async_trait]
impl Handler<AddrMsg> for Generator {
    async fn handle(&mut self, msg: AddrMsg, ctx: &mut Context<Self>) -> Result<(), Error> {
        log::info!("got AddrMsg");

        ctx.spawn(
            async move {
                log::info!("SLEEPING");
                sleep(Duration::from_secs(1)).await;
                log::info!("SLEEPING DONE");

                "this is a str slice"
            },
            |result, actor, ctx| {
                log::info!("SLEEP HAS ENDED: result is '{result}'");
                actor.0 = Some(result.to_string());

                ctx.spawn(async move {}, |_, act, _| {
                    act.0 = Some(String::from("second spawn"))
                });
            },
        );

        log::info!("waiting for 5 seconds before sending Ping");
        sleep(Duration::from_secs(5)).await;
        log::info!("send ping");
        if let Err(e) = msg.0.send(OutputMsg(String::from("PING"))).await {
            Err(e)
        } else {
            Ok(())
        }
    }
}

struct OutputMsg(String);
impl Message for OutputMsg {
    type Result = u32;
}

struct AddrMsg(Addr<Writer>);
impl Message for AddrMsg {
    type Result = Result<(), Error>;
}

struct StreamMsg(String);
impl Message for StreamMsg {
    type Result = ();
}

impl From<&str> for StreamMsg {
    fn from(msg: &str) -> Self {
        StreamMsg(msg.into())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    log::info!("hello world");

    let generator = Generator::default().start();
    let writer = Writer::default().start();

    let awriter2: Addr<Writer> = writer.clone();

    log::info!(
        "writer wrote {} times",
        writer.send(OutputMsg(String::from("hello"))).await.unwrap()
    );
    log::info!(
        "writer wrote {} times",
        writer.send(OutputMsg(String::from("world"))).await.unwrap()
    );
    log::info!(
        "writer wrote {} times",
        writer.send(OutputMsg(String::from("!"))).await.unwrap()
    );
    generator.send(AddrMsg(awriter2)).await.unwrap().unwrap();
    log::info!(
        "writer wrote {} times",
        writer
            .send(OutputMsg(String::from("I'm alive")))
            .await
            .unwrap()
    );

    log::info!("all messages sent, waiting for shutdown");

    drop(writer);
    drop(generator);
    tokio::signal::ctrl_c().await.unwrap();

    log::info!("bye world");
}
