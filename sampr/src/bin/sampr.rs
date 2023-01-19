use sampr::{Actor, Handler, Message};

struct Writer;

impl Actor for Writer {
    fn started(&mut self) {
        log::info!("Writer has started");
    }
    fn stopped(&mut self) {
        log::info!("Writer has stopped");
    }
}

impl Handler<OutputMsg> for Writer {
    fn handle(&mut self, msg: OutputMsg) {
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

struct OutputMsg(String);

impl Message for OutputMsg {}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    log::info!("hello world");

    let generator = Generator.start();
    let writer = Writer.start();

    let awriter = writer.address();
    awriter.send(OutputMsg(String::from("hello"))).await;
    awriter.send(OutputMsg(String::from("world"))).await;
    awriter.send(OutputMsg(String::from("!"))).await;
    awriter.send(OutputMsg(String::from("I'm alive"))).await;

    drop(awriter);

    log::info!("all messages sent, waiting for shutdown");
    generator.wait_for_shutdown().await;
    writer.wait_for_shutdown().await;

    log::info!("bye world");
}
