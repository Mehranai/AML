use anyhow::Result;
use arz_axum_for_services::router::build_router;

#[tokio::main]
async fn main() -> Result<()> {
    let bind_addr =
        std::env::var("TRON_GRAPH_API_ADDR").unwrap_or_else(|_| "127.0.0.1:4001".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    println!("TRON AML dashboard listening on http://{}", bind_addr);

    axum::serve(listener, build_router()).await?;

    Ok(())
}
