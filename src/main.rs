use std::error::Error;

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    wapp_simple_stats_rust::run().await
}
