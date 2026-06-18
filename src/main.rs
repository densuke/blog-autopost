mod article;
mod config;
mod runner;
mod sns;
mod text;

use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting blog-autopost-rs...");

    // スケジューラの初期化
    let mut sched = JobScheduler::new().await?;

    // 例: 毎分0秒に実行されるダミーのジョブ
    // 本来はここに Runner のインスタンスを渡し、run_once を呼び出す
    sched.add(Job::new_async("0 * * * * *", |uuid, mut l| {
        Box::pin(async move {
            println!("Cron job triggered (UUID: {})", uuid);
            // runner.run_once("http://example.com/feed", "default").await;
        })
    })?).await?;

    sched.start().await?;
    
    // メインスレッドが終了しないように待機
    tokio::time::sleep(Duration::from_secs(60 * 60 * 24)).await;
    
    Ok(())
}

