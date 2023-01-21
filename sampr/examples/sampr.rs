use sampr::{async_trait, Actor, Addr, Context, Error, Handler, Message};

#[derive(Default)]
struct Writer {
    count: u32,
}

impl Actor for Writer {
    type Context = Context<Self>;

    fn started(&mut self) {
        log::info!("Writer has started");
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

#[derive(Default)]
struct Generator(Option<String>);

impl Actor for Generator {
    type Context = Context<Self>;

    fn started(&mut self) {
        log::info!("Generator has started");
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
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    log::info!("hello world");

    let generator = Generator::default().start();
    let writer = Writer::default().start();

    let awriter = writer.address();
    let awriter2: Addr<Writer> = awriter.clone();
    let agenerator = generator.address();

    log::info!(
        "writer wrote {} times",
        awriter
            .send(OutputMsg(String::from("hello")))
            .await
            .unwrap()
    );
    log::info!(
        "writer wrote {} times",
        awriter
            .send(OutputMsg(String::from("world")))
            .await
            .unwrap()
    );
    log::info!(
        "writer wrote {} times",
        awriter.send(OutputMsg(String::from("!"))).await.unwrap()
    );
    agenerator.send(AddrMsg(awriter2)).await.unwrap().unwrap();
    log::info!(
        "writer wrote {} times",
        awriter
            .send(OutputMsg(String::from("I'm alive")))
            .await
            .unwrap()
    );

    drop(awriter);
    drop(agenerator);

    log::info!("all messages sent, waiting for shutdown");
    generator.wait_for_shutdown().await;
    writer.wait_for_shutdown().await;

    log::info!("bye world");
}
