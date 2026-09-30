use std::time::Duration;
use tokio::time::sleep;
use tokio_postgres::{Error, NoTls};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Connect to the local Postgres Container.
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=postgres password=postgres dbname=htap_db", NoTls).await?;
    // Hand off connection to tokio - now PG connection runs in background.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    println!("Listening for CDC events on 'test_slot'...");

    // CDC polling loop
    loop {
        // Query logical decoding slot for unread changes
        let rows = client
            .query("SELECT data FROM pg_logical_slot_get_changes('test_slot', NULL, NULL);", &[])
            .await?;
        for row in rows {
            // 'test_decoding' outputs changes as simple plaintext strings
            let change: &str = row.get("data");
            println!("Captured event: {}", change);
        }

        sleep(Duration::from_millis(2000)).await;
    }
}
