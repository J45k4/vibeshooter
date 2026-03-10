mod protocol;
mod server;
mod sim;
mod world;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server::run().await
}
