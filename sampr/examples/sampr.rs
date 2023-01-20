use async_trait::async_trait;
use sampr::{Actor, Addr, Handler, Message};

#[derive(Default)]
struct Writer {
    count: u32,
}

impl Actor for Writer {
    fn started(&mut self) {
        log::info!("Writer has started");
    }
    fn stopped(&mut self) {
        log::info!("Writer has stopped. Sent {} messages in total", self.count);
    }
}

#[async_trait]
impl Handler<OutputMsg> for Writer {
    async fn handle(&mut self, msg: OutputMsg) -> u32 {
        log::info!("writer says (count={}): {}", self.count, msg.0);
        self.count += 1;
        self.count
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
impl Message for OutputMsg {
    type Result = u32;
}

struct AddrMsg(Addr<Writer>);
impl Message for AddrMsg {
    type Result = ();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    log::info!("hello world");

    let generator = Generator.start();
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
    agenerator.send(AddrMsg(awriter2)).await.unwrap();
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
