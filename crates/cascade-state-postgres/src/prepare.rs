// This file is used to prepare SQLx for offline mode
// Run `cargo sqlx prepare -- --lib` to generate the sqlx-data.json file

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This file doesn't need to do anything, it just needs to exist
    // for cargo sqlx prepare to work
    println!("Run cargo sqlx prepare to generate sqlx-data.json for offline mode");
    
    Ok(())
}

// Comment this out when running cargo sqlx prepare
#[cfg(test)]
mod test {
    #[test]
    fn dummy_test() {
        assert!(true);
    }
} 