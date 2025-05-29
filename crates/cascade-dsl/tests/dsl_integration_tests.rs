use cascade_dsl::{
    parse_and_validate_flow_definition, 
    DslError,
};

// Helper function to provide a representative error code for error assertions
fn error_contains(err: &DslError, expected_code: &str) -> bool {
    let error_str = format!("{:?}", err);
    error_str.contains(expected_code)
}

#[test]
fn test_parse_and_validate_valid_flow() {
    // Create a valid DSL definition string
    let dsl_string = r#"
    dsl_version: "1.0"
    definitions:
      components:
        - name: logger
          type: StdLib:Logger
          config:
            level: "info"
          inputs:
            - name: message
              schema_ref: "schema:string"
              is_required: true
          outputs:
            - name: success
              schema_ref: "schema:void"
        - name: http-call
          type: StdLib:HttpCall
          config:
            method: "GET"
            url: "https://api.example.com/data"
          inputs:
            - name: headers
              schema_ref: "schema:object"
              is_required: false
            - name: query_params
              schema_ref: "schema:object"
              is_required: false
          outputs:
            - name: response
              schema_ref: "schema:object"
            - name: error
              schema_ref: "schema:error"
              is_error_path: true
      flows:
        - name: data-processing-flow
          description: "A sample flow for processing data"
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
            output_name: trigger_data
          steps:
            - step_id: fetch-data
              component_ref: http-call
              inputs_map:
                headers: "trigger.headers"
            - step_id: log-data
              component_ref: logger
              inputs_map:
                message: "steps.fetch-data.outputs.response.body"
              run_after: [fetch-data]
              condition:
                type: StdLib:JsonPath
                config:
                  expression: "$.response.success == true"
                  data_ref: "steps.fetch-data.outputs.response"
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert successful parsing and validation
    assert!(result.is_ok(), "Failed to parse valid flow: {:?}", result.err());
    
    let flow_def = result.unwrap();
    
    // Verify the DSL version
    assert_eq!(flow_def.dsl_version, "1.0");
    
    // Verify components
    assert_eq!(flow_def.definitions.components.len(), 2);
    let logger = &flow_def.definitions.components[0];
    assert_eq!(logger.name, "logger");
    assert_eq!(logger.component_type, "StdLib:Logger");
    
    // Verify flow
    assert_eq!(flow_def.definitions.flows.len(), 1);
    let flow = &flow_def.definitions.flows[0];
    assert_eq!(flow.name, "data-processing-flow");
    
    // Verify trigger
    assert_eq!(flow.trigger.trigger_type, "HttpEndpoint");
    assert_eq!(flow.trigger.output_name.as_deref().unwrap_or("trigger"), "trigger_data");
    
    // Verify steps
    assert_eq!(flow.steps.len(), 2);
    let fetch_step = &flow.steps[0];
    assert_eq!(fetch_step.step_id, "fetch-data");
    assert_eq!(fetch_step.component_ref, "http-call");
    
    let log_step = &flow.steps[1];
    assert_eq!(log_step.step_id, "log-data");
    assert_eq!(log_step.component_ref, "logger");
    assert_eq!(log_step.run_after, vec!["fetch-data"]);
    assert!(log_step.condition.is_some());
    
    // Verify inputs_map
    assert!(log_step.inputs_map.contains_key("message"));
    assert_eq!(log_step.inputs_map["message"], "steps.fetch-data.outputs.response.body");
}

#[test]
fn test_parse_and_validate_flow_with_invalid_component_ref() {
    // Create a DSL with an invalid component reference
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
        - name: invalid-component-flow
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
          steps:
            - step_id: log-step
              component_ref: non-existent-component
              inputs_map:
                message: "trigger.body"
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert failure due to invalid component reference
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(error_contains(&err, "INVALID_REFERENCE"));
    assert!(format!("{:?}", err).contains("Component reference 'non-existent-component' not found"));
}

#[test]
fn test_parse_and_validate_flow_with_invalid_step_dependency() {
    // Create a DSL with an invalid step dependency
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
        - name: invalid-dependency-flow
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
          steps:
            - step_id: log-step
              component_ref: logger
              inputs_map:
                message: "trigger.body"
              run_after: [missing-step]
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert failure due to invalid step dependency
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(error_contains(&err, "INVALID_REFERENCE"));
    assert!(format!("{:?}", err).contains("depends on non-existent step ID"));
}

#[test]
fn test_parse_and_validate_flow_with_duplicate_step_ids() {
    // Create a DSL with duplicate step IDs
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
        - name: duplicate-step-id-flow
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
          steps:
            - step_id: log-step
              component_ref: logger
              inputs_map:
                message: "trigger.body"
            - step_id: log-step  # Duplicate step_id
              component_ref: logger
              inputs_map:
                message: "trigger.headers"
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert failure due to duplicate step IDs
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(error_contains(&err, "DUPLICATE_ID"));
    assert!(format!("{:?}", err).contains("Duplicate step ID: 'log-step'"));
}

#[test]
fn test_parse_and_validate_flow_with_circular_dependency() {
    // Create a DSL with a circular dependency (A depends on B, B depends on C, C depends on A)
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
        - name: circular-dependency-flow
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
          steps:
            - step_id: step-a
              component_ref: logger
              inputs_map:
                message: "trigger.body"
              run_after: [step-c]
            - step_id: step-b
              component_ref: logger
              inputs_map:
                message: "steps.step-a.outputs.success"
              run_after: [step-a]
            - step_id: step-c
              component_ref: logger
              inputs_map:
                message: "steps.step-b.outputs.success"
              run_after: [step-b]
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert failure due to circular dependency
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(error_contains(&err, "CIRCULAR_DEPENDENCY"));
    assert!(format!("{:?}", err).contains("Circular dependency detected"));
}

#[test]
fn test_parse_and_validate_invalid_data_reference() {
    // Create a DSL with an invalid data reference
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
        - name: invalid-data-ref-flow
          trigger:
            type: HttpEndpoint
            config:
              path: "/process"
              method: "POST"
          steps:
            - step_id: log-step
              component_ref: logger
              inputs_map:
                message: "steps.nonexistent.outputs.something" # Invalid data reference
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert failure due to invalid data reference
    assert!(result.is_err());
    
    let err = result.unwrap_err();
    assert!(error_contains(&err, "INVALID_REFERENCE"));
    assert!(format!("{:?}", err).contains("Data reference 'steps.nonexistent"));
}

#[test]
fn test_parses_complex_flow_definition() {
    // A complex but valid DSL definition with several different types of steps, data mappings, and expressions
    let dsl_string = r#"
    dsl_version: "1.0"
    definitions:
      components:
        - name: http-call
          type: StdLib:HttpCall
          config:
            method: "GET"
          inputs:
            - name: url
              schema_ref: "schema:string"
            - name: headers
              schema_ref: "schema:object"
              is_required: false
            - name: query_params
              schema_ref: "schema:object"
              is_required: false
          outputs:
            - name: response
              schema_ref: "schema:object"
            - name: error
              schema_ref: "schema:error"
              is_error_path: true
              
        - name: json-transform
          type: StdLib:JsonTransform
          inputs:
            - name: input
              schema_ref: "schema:object"
            - name: template
              schema_ref: "schema:string"
          outputs:
            - name: result
              schema_ref: "schema:object"
              
        - name: json-schema-validator
          type: StdLib:JsonSchemaValidator
          inputs:
            - name: data
              schema_ref: "schema:object"
            - name: schema
              schema_ref: "schema:object"
          outputs:
            - name: validated_data
              schema_ref: "schema:object"
            - name: validation_error
              schema_ref: "schema:error"
              is_error_path: true
              
        - name: condition-branch
          type: StdLib:Switch
          inputs:
            - name: value
              schema_ref: "schema:any"
            - name: cases
              schema_ref: "schema:object"
          outputs:
            - name: case_1
              schema_ref: "schema:any"
            - name: case_2
              schema_ref: "schema:any"
            - name: default
              schema_ref: "schema:any"
              
        - name: db-save
          type: StdLib:DatabaseWrite
          inputs:
            - name: connection
              schema_ref: "schema:string"
            - name: data
              schema_ref: "schema:object"
            - name: table
              schema_ref: "schema:string"
          outputs:
            - name: result
              schema_ref: "schema:object"
            - name: error
              schema_ref: "schema:error"
              is_error_path: true
              
      flows:
        - name: complex-data-processing-flow
          description: "A complex flow for testing DSL parsing capabilities"
          trigger:
            type: HttpEndpoint
            config:
              path: "/process-complex"
              method: "POST"
            output_name: trigger_data
            
          steps:
            - step_id: validate-input
              component_ref: json-schema-validator
              inputs_map:
                data: "trigger.body"
                schema: "trigger.schema"
                
            - step_id: fetch-user-data
              component_ref: http-call
              run_after: [validate-input]
              inputs_map:
                url: "steps.validate-input.outputs.validated_data.user_id"
                headers: "trigger.headers"
                
            - step_id: fetch-source-data
              component_ref: http-call
              run_after: [validate-input]
              inputs_map:
                url: "steps.validate-input.outputs.validated_data.data_source"
                query_params: "trigger.query"
                
            - step_id: transform-combined-data
              component_ref: json-transform
              run_after: [fetch-user-data, fetch-source-data]
              inputs_map:
                input: "steps.fetch-user-data.outputs.response.body"
                template: "steps.fetch-source-data.outputs.response.body"
                  
            - step_id: check-item-count
              component_ref: condition-branch
              run_after: [transform-combined-data]
              inputs_map:
                value: "steps.transform-combined-data.outputs.result.total_count"
                cases: "trigger.cases"
                
            - step_id: handle-no-items
              component_ref: json-transform
              run_after: [check-item-count]
              condition:
                type: StdLib:EqualsValue
                config:
                  left: "steps.check-item-count.outputs.case_1"
                  right: true
              inputs_map:
                input: "steps.transform-combined-data.outputs.result"
                template: "trigger.templates.no_items"
                  
            - step_id: handle-too-many-items
              component_ref: json-transform
              run_after: [check-item-count]
              condition:
                type: StdLib:EqualsValue
                config:
                  left: "steps.check-item-count.outputs.case_2"
                  right: true
              inputs_map:
                input: "steps.transform-combined-data.outputs.result"
                template: "trigger.templates.too_many"
                  
            - step_id: process-normal-items
              component_ref: json-transform
              run_after: [check-item-count]
              condition:
                type: StdLib:NotExpression
                config:
                  expression: "steps.check-item-count.outputs.case_1 == true || steps.check-item-count.outputs.case_2 == true"
              inputs_map:
                input: "steps.transform-combined-data.outputs.result"
                template: "trigger.templates.normal"
                  
            - step_id: save-result
              component_ref: db-save
              run_after: [handle-no-items, handle-too-many-items, process-normal-items]
              inputs_map:
                connection: "trigger.connection"
                table: "trigger.table"
                data: "steps.process-normal-items.outputs.result"
    "#;

    // Parse and validate the flow definition
    let result = parse_and_validate_flow_definition(dsl_string);
    
    // Assert successful parsing
    assert!(result.is_ok(), "Failed to parse complex flow: {:?}", result.err());
    
    let flow_def = result.unwrap();
    
    // Verify the number of components and steps
    assert_eq!(flow_def.definitions.components.len(), 5, "Should have 5 components");
    assert_eq!(flow_def.definitions.flows[0].steps.len(), 9, "Should have 9 steps");
    
    // Verify conditional branching structure
    let steps = &flow_def.definitions.flows[0].steps;
    let check_step = steps.iter().find(|s| s.step_id == "check-item-count").unwrap();
    assert_eq!(check_step.component_ref, "condition-branch");
    
    // Verify conditional execution paths
    let no_items_step = steps.iter().find(|s| s.step_id == "handle-no-items").unwrap();
    assert!(no_items_step.condition.is_some());
    
    let too_many_step = steps.iter().find(|s| s.step_id == "handle-too-many-items").unwrap();
    assert!(too_many_step.condition.is_some());
    
    let normal_step = steps.iter().find(|s| s.step_id == "process-normal-items").unwrap();
    assert!(normal_step.condition.is_some());
    
    // Verify the final save step dependencies
    let save_step = steps.iter().find(|s| s.step_id == "save-result").unwrap();
    assert_eq!(save_step.run_after.len(), 3);
    assert!(save_step.run_after.contains(&"handle-no-items".to_string()));
    assert!(save_step.run_after.contains(&"handle-too-many-items".to_string()));
    assert!(save_step.run_after.contains(&"process-normal-items".to_string()));
} 