# Stage 1 - Builder
# PG 17
FROM postgres:17 AS builder

# Build Dependencies required for compiling C/C++ extensions
RUN apt-get update && apt-get install -y \
  build-essential \
  postgresql-server-dev-17 \
  git \
  curl \
  wget \
  unzip \
  libstdc++-12-dev

WORKDIR /build

# Clone DuckDB Foreign Data Wrapper repo
RUN git clone --depth 1 https://github.com/alitrack/duckdb_fdw.git .

# Fetch precompiled DuckDB C lib (libduckdb.so)
RUN bash ./download_libduckdb.sh

# Compile extension and install it to Postgres directories
RUN make USE_PGXS=1 && make install USE_PGXS=1

# Stage 2 - Final Engine
FROM postgres:17

# DuckDB requires the C++ standard lib to run
RUN apt-get update && apt-get install -y libstdc++6 postgresql-17-wal2json && rm -rf /var/lib/apt/lists/*

# Copy the compiled extension and SQL control files from bulder stage
COPY --from=builder /usr/lib/postgresql/17/lib/duckdb_fdw.so /usr/lib/postgresql/17/lib
COPY --from=builder /usr/share/postgresql/17/extension/duckdb_fdw* /usr/share/postgresql/17/extension

# Copy DuckDB shared lib into system lib path, so that Postgres can find it.
COPY --from=builder /build/libduckdb.so /usr/lib/libduckdb.so
