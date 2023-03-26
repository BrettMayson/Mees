#[tokio::main]
async fn main() {
    let addr = std::env::var("MEES_ADDR").unwrap_or_else(|_| "localhost:6454".to_string());
    mees_bin::run(addr.parse().unwrap()).await;
}
