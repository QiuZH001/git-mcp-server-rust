#!/usr/bin/env bash
# Memory allocation benchmark for git-mcp-server
# Measures heap allocations and peak memory

set -e

echo "=== Git MCP Server Memory Benchmark ==="
echo

# Check if binary exists
if [ ! -f "./target/release/git-mcp-server" ]; then
    echo "Building release binary..."
    cargo build --release
fi

BINARY="./target/release/git-mcp-server"

# Test 1: Baseline memory (just the binary)
echo "1. Baseline memory (idle server):"
/usr/bin/time -l "$BINARY" 2>&1 | grep -E "(maximum resident|page reclaims)" || true
echo

# Test 2: Single tool call (git_status)
echo "2. Single git_status call:"
(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"git_status","arguments":{"path":"/tmp"}}}' | /usr/bin/time -l "$BINARY" 2>&1) | grep -E "(maximum resident|page reclaims)" || true
echo

# Test 3: Multiple rapid calls
echo "3. 10 rapid tool calls:"
(
    for i in {1..10}; do
        echo '{"jsonrpc":"2.0","id":'$i',"method":"tools/call","params":{"name":"git_status","arguments":{}}}'
    done
    sleep 0.5
) | /usr/bin/time -l "$BINARY" 2>&1 | grep -E "(maximum resident|page reclaims)" || true
echo

# Test 4: Memory allocator stats (with jemalloc if available)
echo "4. Allocator statistics (if available):"
MALLOC_CONF=stats_print:true "$BINARY" 2>&1 | head -50 || echo "Jemalloc not available, using system allocator"
echo

echo "=== Benchmark Complete ==="
