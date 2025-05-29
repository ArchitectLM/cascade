# Cascade DSL

The Cascade DSL is a YAML-based domain-specific language for defining reactive workflows in the Cascade Platform. 
This crate provides functionality for parsing, validating, and representing Cascade DSL documents.

## Features

- **YAML-based DSL**: Define workflows as YAML documents
- **Component-based Architecture**: Reusable, modular components
- **Reactive Workflow Model**: Event-driven execution of steps
- **Comprehensive Validation**: Catch errors before runtime
- **Schema Validation**: Ensure data types match component requirements
- **Reference Resolution**: Validate references between components and steps
- **Circular Dependency Detection**: Prevent infinite loops

## Usage

```rust
use cascade_dsl::parse_and_validate_flow_definition;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dsl_string = r#"
    dsl_version: "1.0"
    definitions:
      components:
        - name: logger
          type: StdLib:Logger
          inputs:
            - name: message
              schema_ref: "schema:string"
          outputs:
            - name: success
              schema_ref: "schema:void"
      flows:
        - name: hello-world
          trigger:
            type: HttpEndpoint
            config:
              path: "/hello"
          steps:
            - step_id: log-hello
              component_ref: logger
              inputs_map:
                message: "trigger.body"
    "#;

    let flow_def = parse_and_validate_flow_definition(dsl_string)?;
    println!("Parsed flow: {}", flow_def.definitions.flows[0].name);
    Ok(())
}
```

## DSL Structure

The DSL is structured as follows:

- **dsl_version**: The version of the DSL
- **definitions**: Container for components and flows
  - **components**: List of component definitions
  - **flows**: List of flow definitions

### Components

Components define reusable building blocks with inputs and outputs:

```yaml
components:
  - name: logger
    type: StdLib:Logger
    inputs:
      - name: message
        schema_ref: "schema:string"
    outputs:
      - name: success
        schema_ref: "schema:void"
```

### Flows

Flows define the workflow with a trigger and a series of steps:

```yaml
flows:
  - name: hello-world
    trigger:
      type: HttpEndpoint
      config:
        path: "/hello"
    steps:
      - step_id: log-hello
        component_ref: logger
        inputs_map:
          message: "trigger.body"
```

## Validation

The DSL includes comprehensive validation to ensure:

- References to components and steps are valid
- Step IDs are unique within a flow
- No circular dependencies exist
- Data references have the correct format
- Schema references are valid

## License

This project is licensed under the MIT License - see the LICENSE file for details.