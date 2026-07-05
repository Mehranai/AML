use clickhouse::Client;

pub async fn run_sql(client: &Client, sql: &str) -> anyhow::Result<()> {
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if has_executable_sql(stmt) {
            // println!("Inja reside");
            client
                .query(stmt)
                .execute()
                .await
                .map_err(|err| anyhow::anyhow!("failed SQL statement:\n{}\n\n{}", stmt, err))?;
        }
    }
    Ok(())
}

fn has_executable_sql(stmt: &str) -> bool {
    stmt.lines().any(|line| {
        let line = line.trim();

        !line.is_empty() && !line.starts_with("--")
    })
}
