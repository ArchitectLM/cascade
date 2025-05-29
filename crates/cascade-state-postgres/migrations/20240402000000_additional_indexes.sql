-- Add index on flow instance updated_at for efficient cleanup
CREATE INDEX IF NOT EXISTS idx_flow_instances_updated_at ON flow_instances(updated_at);

-- Add index on component states updated_at for efficient cleanup
CREATE INDEX IF NOT EXISTS idx_component_states_updated_at ON component_states(updated_at); 