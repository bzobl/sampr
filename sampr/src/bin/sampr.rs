use async_trait::async_trait;
use sampr::{Actor, Addr, Handler, Message};

struct Writer;

impl Actor for Writer {
    fn started(&mut self) {
        log::info!("Writer has started");
    }
    fn stopped(&mut self) {
        log::info!("Writer has stopped");
    }
}

#[async_trait]
impl Handler<OutputMsg> for Writer {
    async fn handle(&mut self, msg: OutputMsg) {
        log::info!("writer says: {}", msg.0);
    }
}

struct Generator;

impl Actor for Generator {
    fn started(&mut self) {
        log::info!("Generator has started");
    }

    fn stopped(&mut self) {
        log::info!("Generator has stopped");
    }
}

#[async_trait]
impl Handler<AddrMsg> for Generator {
    async fn handle(&mut self, msg: AddrMsg) {
        log::info!("send ping");
        msg.0.send(OutputMsg(String::from("PING"))).await;
    }
}

struct OutputMsg(String);
impl Message for OutputMsg {}

struct AddrMsg(Addr<Writer>);
impl Message for AddrMsg {}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    log::info!("hello world");

    let generator = Generator.start();
    let writer = Writer.start();

    let awriter = writer.address();
    let awriter2: Addr<Writer> = awriter.clone();
    let agenerator = generator.address();

    awriter.send(OutputMsg(String::from("hello"))).await;
    awriter.send(OutputMsg(String::from("world"))).await;
    awriter.send(OutputMsg(String::from("!"))).await;
    agenerator.send(AddrMsg(awriter2)).await;
    awriter.send(OutputMsg(String::from("I'm alive"))).await;

    drop(awriter);
    drop(agenerator);

    log::info!("all messages sent, waiting for shutdown");
    generator.wait_for_shutdown().await;
    writer.wait_for_shutdown().await;

    log::info!("bye world");
}
