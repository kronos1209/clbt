use std::time::Duration;

mod bot;
mod context;
mod manager;

async fn test() {
    let a = tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            println!("t")
        }
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            println!("t")
        }
    };
}