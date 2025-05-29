#!/bin/bash
# Script to run all BDD tests

# Set the directory to the location of this script
cd "$(dirname "$0")"

echo "=== Running BDD tests ==="
echo

echo "=== Running all feature tests ==="
# Run the BDD tests
RUST_LOG=debug cargo test --test bdd
if [ $? -ne 0 ]; then
    echo "Error: BDD tests failed!"
    exit 1
fi

echo
echo "All tests completed successfully!"
exit 0 