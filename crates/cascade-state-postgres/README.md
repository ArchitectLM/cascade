# Cascade PostgreSQL State Store

This crate provides a PostgreSQL implementation of the state store for the Cascade Platform.

## Features

- Persistent storage of flow definitions and instances
- Transaction support for atomic operations
- Efficient querying and indexing
- Automatic migrations

## Usage

```rust
use cascade_state_postgres::{PostgresStateStore, PostgresConnection};

// Create a connection string for your PostgreSQL database
let connection_string = "postgres://user:password@localhost:5432/cascade";

// Create a new state store
let state_store = PostgresStateStore::new(connection_string).await?;

// Get the repositories
let flow_instance_repo = state_store.flow_instance_repository();
let flow_definition_repo = state_store.flow_definition_repository();
let component_state_repo = state_store.component_state_repository();
let timer_repo = state_store.timer_repository();

// Use the repositories to interact with the state store
let flow_instance = flow_instance_repo.find_by_id(&instance_id).await?;
```

## Development Setup

### SQLx and Offline Mode

This crate uses SQLx for database access, which by default requires a live database connection at compile time to validate SQL queries. To work around this for development, we use SQLx's offline mode:

1. Set up a local PostgreSQL database for testing:

```bash
# Start a PostgreSQL database in Docker
docker run --name cascade-postgres -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=cascade -p 5432:5432 -d postgres:latest
```

2. Create an .env file in the crate root with your database connection string:

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/cascade
```

3. Generate the sqlx-data.json file when making changes to SQL queries:

```bash
# Install the SQLx CLI if you haven't already
cargo install sqlx-cli

# Generate the sqlx-data.json file
cargo sqlx prepare --database-url postgres://postgres:postgres@localhost:5432/cascade -- --lib
```

This will allow you to compile the crate without a database connection using the prepared SQL data.

## Tables

The PostgreSQL implementation creates the following tables:

- `flow_definitions`: Stores flow definitions
- `flow_instances`: Stores flow instances
- `component_states`: Stores component state data
- `correlations`: Stores correlation IDs for event correlation
- `timers`: Stores timers for scheduling

## Notes on Testing

When testing this crate, you have two options:

1. Use a real PostgreSQL database (recommended for integration tests)
2. Use the mock implementations for unit tests

### Integration Testing with PostgreSQL

For integration tests, it's best to use a real PostgreSQL database. You can set up a temporary database for testing:

```rust
// Set up a test database
let connection_string = "postgres://postgres:postgres@localhost:5432/cascade_test";
let state_store = PostgresStateStore::new(connection_string).await?;
```

### Unit Testing with Mocks

For unit tests, you can use the mock implementations:

```rust
use cascade_state_postgres::connection::MockPostgresConnection;

// Create a mock connection
let mock_conn = MockPostgresConnection::new().await;
let conn = PostgresConnection::from(mock_conn);

// Create repositories with the mock connection
let flow_instance_repo = PostgresFlowInstanceRepository::new(conn.clone());
```

## Migration Error Handling

If there are issues with migrations, you might need to rebuild the database. For development purposes, you can drop and recreate the database:

```bash
dropdb -U postgres cascade
createdb -U postgres cascade
```

Then run your application again to apply the migrations. 