// Minimal BDD test to prove the pattern works

use cucumber::World;
use std::convert::Infallible;

// Minimal world implementation
#[derive(Debug, World)]
struct MyWorld {
    counter: i32,
}

impl Default for MyWorld {
    fn default() -> Self {
        Self { counter: 0 }
    }
}

// Step definition module
mod steps {
    use cucumber::{given, then, when};
    use super::MyWorld;

    #[given(expr = "I start with {int}")]
    fn start_with(world: &mut MyWorld, value: i32) {
        world.counter = value;
    }

    #[when(expr = "I add {int}")]
    fn add(world: &mut MyWorld, value: i32) {
        world.counter += value;
    }

    #[then(expr = "I should have {int}")]
    fn check_value(world: &mut MyWorld, expected: i32) {
        assert_eq!(world.counter, expected);
    }
}

// Main test runner
#[tokio::main]
async fn main() {
    println!("Running minimal BDD test");
    
    // In cucumber 0.19.1, the World trait uses Default
    MyWorld::cucumber()
        .run("minimal_features")
        .await;
} 