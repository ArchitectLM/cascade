/// Generate SQL migrations for PostgreSQL state store
/// 
/// These migrations create the necessary tables and indexes for the Cascade Core
/// state store in PostgreSQL.
pub fn generate_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        // Initial migration - Create core tables
        (
            "20240401000000_initial_schema",
            r#"
            -- Create flow definitions table
            CREATE TABLE IF NOT EXISTS flow_definitions (
                id TEXT PRIMARY KEY,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            
            -- Create flow instances table
            CREATE TABLE IF NOT EXISTS flow_instances (
                id TEXT PRIMARY KEY,
                flow_id TEXT NOT NULL,
                status TEXT NOT NULL,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL,
                CONSTRAINT fk_flow_id FOREIGN KEY (flow_id) REFERENCES flow_definitions(id) ON DELETE CASCADE
            );
            
            -- Create index on flow_id for quick lookups
            CREATE INDEX IF NOT EXISTS idx_flow_instances_flow_id ON flow_instances(flow_id);
            
            -- Create index on status for filtering
            CREATE INDEX IF NOT EXISTS idx_flow_instances_status ON flow_instances(status);
            
            -- Create component state table
            CREATE TABLE IF NOT EXISTS component_states (
                flow_instance_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                state JSONB NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (flow_instance_id, step_id),
                CONSTRAINT fk_flow_instance_id FOREIGN KEY (flow_instance_id) REFERENCES flow_instances(id) ON DELETE CASCADE
            );
            
            -- Create correlations table for event correlation
            CREATE TABLE IF NOT EXISTS correlations (
                correlation_id TEXT NOT NULL,
                flow_instance_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (correlation_id, flow_instance_id),
                CONSTRAINT fk_correlation_flow_instance_id FOREIGN KEY (flow_instance_id) REFERENCES flow_instances(id) ON DELETE CASCADE
            );
            
            -- Create index on correlation_id for quick lookups
            CREATE INDEX IF NOT EXISTS idx_correlations_correlation_id ON correlations(correlation_id);
            
            -- Create timers table
            CREATE TABLE IF NOT EXISTS timers (
                id TEXT PRIMARY KEY,
                flow_instance_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                scheduled_time TIMESTAMPTZ NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'processing', 'completed'
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_timer_flow_instance_id FOREIGN KEY (flow_instance_id) REFERENCES flow_instances(id) ON DELETE CASCADE
            );
            
            -- Create index on scheduled_time and status for efficient timer processing
            CREATE INDEX IF NOT EXISTS idx_timers_scheduled_time_status ON timers(scheduled_time, status);
            "#
        ),
        
        // Update for additional indexes or optimizations
        (
            "20240402000000_additional_indexes",
            r#"
            -- Add index on flow instance updated_at for efficient cleanup
            CREATE INDEX IF NOT EXISTS idx_flow_instances_updated_at ON flow_instances(updated_at);
            
            -- Add index on component states updated_at for efficient cleanup
            CREATE INDEX IF NOT EXISTS idx_component_states_updated_at ON component_states(updated_at);
            "#
        ),
    ]
} 