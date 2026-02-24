use git_mcp_server::config::Config;
use git_mcp_server::server::execute_tool;
use git_mcp_server::tools::ToolContext;
use std::time::Instant;

fn main() {
    println!("=== Git MCP Server Quick Benchmark ===\n");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = Config::default();
    let ctx = ToolContext::new(config);

    // Warm up
    let args = serde_json::json!({"path": "/tmp"});
    let _ = rt.block_on(execute_tool(&ctx, "git_status", args.clone()));

    // Benchmark execute_tool
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let result = rt.block_on(execute_tool(&ctx, "git_status", args.clone()));
        let _ = result;
    }
    let elapsed = start.elapsed();

    println!("execute_tool (git_status):");
    println!("  {} iterations in {:?}", iterations, elapsed);
    println!(
        "  {:.2} us/iteration",
        elapsed.as_micros() as f64 / iterations as f64
    );
    println!();

    // JSON serialization benchmark
    let test_data = serde_json::json!({
        "success": true,
        "branch": "main",
        "staged": ["file1.rs", "file2.rs"],
        "modified": ["file3.rs"],
    });

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = serde_json::to_string(&test_data).unwrap();
    }
    let elapsed = start.elapsed();

    println!("serde_json::to_string:");
    println!("  {} iterations in {:?}", iterations, elapsed);
    println!(
        "  {:.2} us/iteration",
        elapsed.as_micros() as f64 / iterations as f64
    );
    println!();

    // Pretty print benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = serde_json::to_string_pretty(&test_data).unwrap();
    }
    let elapsed = start.elapsed();

    println!("serde_json::to_string_pretty:");
    println!("  {} iterations in {:?}", iterations, elapsed);
    println!(
        "  {:.2} us/iteration",
        elapsed.as_micros() as f64 / iterations as f64
    );
    println!();

    println!("=== Benchmark Complete ===");
}
