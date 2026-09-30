# Zero-ETL HTAP Database Engine

> Link PostgreSQL and DuckDB through a Hybrid Transactional/Analytical Processing without the overhead of an Extract-Transform-Load pipeline.

## Objective

A custom data engineering architecture that transforms PostgreSQL into a Hybrid Transactional/Analytical Processing (HTAP) system. This project eliminates fragile external ETL pipelines by capturing logical Write-Ahead Log (WAL) events from PostgreSQL in real time, maintaining a synchronised columnar data lake on disk, and intercepting the query planner to dynamically route OLAP workloads to an embedded DuckDB engine.

## System Architecture

The project aims to bridge the gap between transactional row-based storage and analytical columnar storage without moving data across a network to a separate warehouse.

+ **Transactional layer (OLTP)**: Standard PostgreSQL 17 handles all continuous, high-throughput ACID writes and point-lookups.

+ **Zero-ETL Synchronisation**: A Rust-based asynchronous consumer uses `tokio` to poll PostgreSQL's logical replication slots, capturing row-level `INSERT`, `UPDATE` and `DELETE` events the microsecond they commit.

+ **Analytical Layer (OLAP)**: The captured data is flushed to highly compressed Parquet (and eventually Apache Iceberg) files. A compiled C-extension (duckdb_fdw) embeds DuckDB directly into Postgres, allowing it to natively query these columnar files using vectorised execution.

## Performance Benchmark

The manual prototype was tested against the industry-standard **TPC-H (Scale Factor 1)** dataset consisting of approximately 1GB of data and 6 million rows in the `lineitem` table.

### Aggregation Query:

```SQL
SELECT l_returnflag, l_linestatus, sum(l_quantity) as sum_qty, sum(l_extendedprice) as sum_base_price
FROM lineitem
GROUP BY l_returnflag, l_linestatus;
```

+ **Native PostgreSQL Execution**: `605 ms` (As it is forced to load 16 columns per row into memory to calculate 4 columns).

+ **DuckDB Embedded Execution**: `51 ms` (Leveraged columnar pruning to read only the required 4 columns and utilised SIMD vectorised processing).

+ **Result**: A `12x` performance increase entirely contained within the PostgreSQL ecosystem.

## Development Roadmap

This system is being engineered iteratively across four distinct phases:

- **Phase 1: Manual Prototype (Completed)**
    - Master DuckDB and Parquet by converting large datasets and writing analytical queries against them.
    - Set up a standard PostgreSQL instance, load millions of rows of data, and export to CSV/Parquet on the local disk.
    - Install the `duckdb_fdw` extension to map DuckDB to Postgres, allowing standard `psql` queries to fetch data directly from Parquet files.
- **Phase 2: CDC to Iceberg (In Progress)**
    - Understand how Postgres broadcasts database changes (inserts/updates/deletes) to the outside world using logical decoding and replication slots.
    - Write an external Rust application that connects to Postgres, listens to the logical replication slot, and prints database changes to the terminal.
    - Modify the consumer to write data to Apache Iceberg, utilising a table format that allows ACID transactions on data lakes.
- **Phase 3: Native PostgreSQL Extension**
    - Learn `pgrx`, a framework for writing safer Postgres extensions in Rust, eliminating the need for any external infrastructure.
    - Port the external Phase 2 Rust consumer into a native Postgres Background Worker using `pgrx`.
    - Internalise the sync so that the database natively monitors its own WAL and writes columnar data to disk invisibly to the user.
- **Phase 4: Query Router**
    - Implement a planner hook in the `pgrx` extension to intercept the SQL query after Postgres parses it, but before execution.
    - Write heuristics to analyze the query's Abstract Syntax Tree (AST) to determine if it is a heavy OLAP aggregation or a simple OLTP select.
    - Rewrite the execution plan on the fly to force analytical workloads to bypass the local row-based storage and directly hit the embedded DuckDB engine instead.

## Tech Stack and Core Infrastructure
* **PostgreSQL 17:** The primary OLTP kernel, configured for logical replication (`wal_level = logical`).
* **DuckDB and Parquet:** The embedded OLAP engine and columnar storage format, enabling SIMD-accelerated vectorised execution on analytical queries.
* **Rust (tokio, tokio-postgres):** The high-performance async consumer built to poll the logical decoding slot and translate row-based WAL events into columnar data.
* **C (duckdb_fdw):** Compiled from source to create the Foreign Data Wrapper bridging the Postgres query planner to DuckDB.
* **Docker (Multi-stage builds):** Utilised to isolate the C/C++ build toolchains and bypass host OS linker limitations, producing a clean, deployment-ready database container.

## [WIP] How it compares to Industry Standards

* **vs. Traditional ETL (Fivetran, Airflow):** Traditional pipelines rely on scheduled batch jobs over a network, leading to stale data and high I/O overhead. This engine captures WAL events logically and continuously, meaning analytical data is synchronised in real-time with zero external network hops.
* **vs. Native pg_duckdb:** Existing extensions like `pg_duckdb` are heavily compute-centric, often pulling live row-oriented Postgres data into memory to perform vectorised math. This architecture is strictly storage centric. It asynchronously flushes WAL events to decoupled Parquet/Iceberg files, giving Postgres infinite, cheap analytical storage without fighting the primary database for live memory.

## Current Setup and Execution

The project utilises a multi-stage Docker build to bypass host OS linking constraints and compile the `duckdb_fdw` C-extension from source.

- **1. Boot the Database Environment**

```bash
# Build the C-extension and start PostgreSQL 17
docker-compose up -d --build

# Enter the interactive SQL shell
docker exec -it htap_postgres psql -U postgres -d htap_db
```

- **2. Run the Rust CDC Consumer**

The database is configured with `wal_level = logical`. The Rust consumer connects over TCP to stream the events captured in real time.

```bash
cd htap_cdc
cargo run
```
