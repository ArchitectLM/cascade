#!/bin/bash
set -e

echo "======================================================="
echo "Cascade Knowledge Base Environment Cleanup"
echo "======================================================="
echo ""
echo "This script will clean up Docker resources used by the Knowledge Base."
echo "Options:"
echo "1. Clean containers only (preserves data)"
echo "2. Clean containers and volumes (removes all data)"
echo ""

read -p "Enter your choice (1 or 2): " choice

case $choice in
  1)
    echo ""
    echo "Stopping and removing containers..."
    docker-compose down
    echo "Containers removed. Data volumes are preserved."
    ;;
    
  2)
    echo ""
    echo "WARNING: This will remove all containers AND data volumes."
    read -p "Are you sure? (y/n): " confirm
    
    if [ "$confirm" == "y" ] || [ "$confirm" == "Y" ]; then
      echo "Stopping and removing containers with volumes..."
      docker-compose down -v
      
      echo "Removing any remaining Neo4j data directories..."
      rm -rf data/neo4j-data/* data/neo4j-logs/* data/neo4j-plugins/* 2>/dev/null || true
      
      echo "All containers and volumes have been removed."
    else
      echo "Operation cancelled."
    fi
    ;;
    
  *)
    echo "Invalid choice. Please run the script again and enter 1 or 2."
    exit 1
    ;;
esac

echo ""
echo "Cleanup complete!" 