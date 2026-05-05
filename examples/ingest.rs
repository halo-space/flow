#[path = "common/demo.rs"]
mod shared;

#[tokio::main]
async fn main() -> rag::Result<()> {
    let chunk_count = shared::ingest_all_texts().await?;
    shared::wait_for_refresh().await;
    println!("indexed {chunk_count} chunks");
    Ok(())
}
