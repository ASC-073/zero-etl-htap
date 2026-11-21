use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tokio_postgres::{Error, NoTls};

// Structs to handle wal2json outputs
#[derive(Deserialize, Debug)]
pub struct Wal2JsonPayload {
    pub change: Vec<TableChange>,
}

// Serde must read "kind" field in JSON to match to correct enum variant
#[derive(Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum TableChange {
    #[serde(rename = "insert")]
    Insert {
        schema: String,
        table: String,
        columnnames: Vec<String>,
        columnvalues: Vec<Value>,
    },

    #[serde(rename = "update")]
    Update {
        schema: String,
        table: String,
        columnnames: Vec<String>,
        columnvalues: Vec<Value>,
        // oldkeys only present if REPLICA IDENTITY is configured on the PG table.
        // Hence here, it is optional.
        oldkeys: Option<OldKeys>,
    }

    #[serde(rename = "delete")]
    Delete {
        schema: String,
        table: String,
        oldkeys: OldKeys,
    }

    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct OldKeys {
    pub keynames: Vec<String>,
    pub keyvalues: Vec<Value>,
    pub keytypes: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    const REPLICATION_SLOT_NAME: &str = "json_slot";

    // Connect to the local Postgres Container.
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=postgres password=postgres dbname=htap_db", NoTls)
        .await
        .context("Failed to connect to PostgreSQL server")?;
    // Hand off connection to tokio - now PG connection runs in background.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    println!("Listening for CDC events on '{}'...", REPLICATION_SLOT_NAME);

    // CDC polling loop
    loop {
        // Query logical decoding slot for unread changes
        // Lock the scema to format-version '1' so that our struct logic can use wal2json's outputs consistently.
        let slot_query = format!("SELECT data FROM pg_logical_slot_get_changes('{}', NULL, NULL, 'format-version', '1');", REPLICATION_SLOT_NAME);
        let rows = client
            .query(&slot_query, &[])
            .await?;
        for row in rows {
            let raw_json: &str = row.get("data");
            // Utilise wal2json's structured output
            // Automatically convert serde_json::Error to anyhow::Error
            let payload = serde_json::from_str(raw_json)
                .context("Failed to deserialise WAL payload");
            
            for change in payload.change {
                match change {
                    TableChange::Insert { table, columnnames, columnvalues, .. } => {
                        println!("Inserting into {}: {:?}", table, columnvalues);
                    }
                    TableChange::Update { table, columnnames, columnvalues, oldkeys, .. } => {
                        println!("Updating {} where keys {:?} to {:?}", table, oldkeys, columnvalues);
                    }
                    TableChange::Delete { table, oldkeys, .. } => {
                        println!("Deleting from {} where keys {:?}", table, oldkeys);
                    }
                    TableChange::Unknown => {
                        // Ignore safely
                    }
                }
            }
        }

        sleep(Duration::from_millis(2000)).await;
    }
}
