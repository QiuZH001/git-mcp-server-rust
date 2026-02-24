use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use git_mcp_server::config::Config;
use git_mcp_server::server::execute_tool;
use git_mcp_server::tools::ToolContext;
use serde_json::Value;
use tokio::runtime::Runtime;

fn bench_execute_tool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = Config::default();
    let ctx = ToolContext::new(config);

    let arguments = serde_json::json!({
        "path": "/tmp/test"
    });

    let mut group = c.benchmark_group("execute_tool");

    for tool_name in ["git_status", "git_log", "git_branch"] {
        group.bench_with_input(
            BenchmarkId::from_parameter(tool_name),
            &tool_name,
            |b, &name| {
                b.iter(|| {
                    let _ = rt.block_on(execute_tool(
                        &ctx,
                        black_box(name),
                        black_box(arguments.clone()),
                    ));
                });
            },
        );
    }

    group.finish();
}

fn bench_json_serialization(c: &mut Criterion) {
    let test_data = serde_json::json!({
        "success": true,
        "branch": "main",
        "ahead": 0,
        "behind": 0,
        "staged": ["file1.rs", "file2.rs"],
        "modified": ["file3.rs"],
        "untracked": ["new.rs"]
    });

    let mut group = c.benchmark_group("json_serialization");

    group.bench_function("to_string", |b| {
        b.iter(|| serde_json::to_string(black_box(&test_data)));
    });

    group.bench_function("to_string_pretty", |b| {
        b.iter(|| serde_json::to_string_pretty(black_box(&test_data)));
    });

    group.finish();
}

fn bench_response_format(c: &mut Criterion) {
    let test_value: Value = serde_json::json!({
        "content": [{
            "type": "text",
            "text": "Sample git output with multiple lines\nBranch: main\nStatus: clean"
        }],
        "isError": false
    });

    let mut group = c.benchmark_group("response_format");

    group.bench_function("json", |b| {
        b.iter(|| serde_json::to_string(black_box(&test_value)).unwrap());
    });

    group.bench_function("markdown", |b| {
        b.iter(|| {
            let text = serde_json::to_string(black_box(&test_value)).unwrap();
            format!("```json\n{}\n```", text)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_execute_tool,
    bench_json_serialization,
    bench_response_format
);
criterion_main!(benches);
