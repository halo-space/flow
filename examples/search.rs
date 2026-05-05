#[path = "common/demo.rs"]
mod shared;

#[tokio::main]
async fn main() -> rag::Result<()> {
    shared::run_search().await
}
