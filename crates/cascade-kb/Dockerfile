FROM rust:1.71-bullseye as builder

WORKDIR /usr/src/cascade

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    apt-transport-https \
    ca-certificates \
    gnupg \
    lsb-release \
    && rm -rf /var/lib/apt/lists/*

# Install Neo4j command-line tools
RUN wget -O - https://debian.neo4j.com/neotechnology.gpg.key | apt-key add -
RUN echo 'deb https://debian.neo4j.com stable latest' | tee -a /etc/apt/sources.list.d/neo4j.list
RUN apt-get update && apt-get install -y cypher-shell \
    && rm -rf /var/lib/apt/lists/*

# Install cargo dependencies
RUN cargo install cargo-make

# Copy cargo config
COPY Cargo.toml .
COPY Cargo.lock .

# Copy only the cascade-kb crate
COPY crates/cascade-kb /usr/src/cascade/crates/cascade-kb

# Build dependencies to cache them
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -f src/main.rs

# Run the tests with docker-compose
CMD ["/bin/bash", "-c", "cd crates/cascade-kb && ./run-docker-tests.sh"] 