#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    hearthline_api::serve().await
}
