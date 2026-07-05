use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clickhouse::Client;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::time::sleep;

use super::traits::BatchInsert;

pub struct GenericBatcher<T>
where
    T: BatchInsert,
{
    clickhouse: Arc<Client>,
    rows: Arc<Mutex<Vec<T>>>,
    max_batch_size: usize,
    flush_interval: Duration,
}

impl<T> GenericBatcher<T>
where
    T: BatchInsert,
    for<'a> T::Value<'a>: Serialize + Send,
{
    pub fn create(
        clickhouse: Arc<Client>,
        max_batch_size: usize,
        flush_interval: Duration,
    ) -> Arc<Self> {
        let batcher = Arc::new(Self {
            clickhouse,
            rows: Arc::new(Mutex::new(Vec::new())),
            max_batch_size,
            flush_interval,
        });

        Self::start_flush_task(batcher.clone());

        batcher
    }

    pub async fn push(&self, row: T) -> Result<()> {
        let mut rows = self.rows.lock().await;

        rows.push(row);

        if rows.len() >= self.max_batch_size {
            let batch = rows.drain(..).collect::<Vec<_>>();
            drop(rows);
            self.flush(batch).await?;
        }

        Ok(())
    }

    pub async fn flush_all(&self) -> Result<()> {
        let batch = {
            let mut rows = self.rows.lock().await;
            rows.drain(..).collect::<Vec<_>>()
        };

        self.flush(batch).await
    }

    async fn flush(&self, batch: Vec<T>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let row_count = batch.len();
        let mut insert = self.clickhouse.insert::<T>(T::TABLE).await?;

        for row in &batch {
            let value = row.as_value();
            insert.write(&value).await?;
        }

        insert.end().await?;

        println!("[CLICKHOUSE][{}] inserted {} row(s)", T::TABLE, row_count);

        Ok(())
    }

    fn start_flush_task(batcher: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(batcher.flush_interval).await;

                let batch = {
                    let mut rows = batcher.rows.lock().await;

                    if rows.is_empty() {
                        continue;
                    }

                    rows.drain(..).collect::<Vec<_>>()
                };

                if let Err(err) = batcher.flush(batch).await {
                    eprintln!("[BATCHER ERROR][{}] {:?}", T::TABLE, err);
                }
            }
        });
    }
}
