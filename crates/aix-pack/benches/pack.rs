//! Benchmarks for the pack path.
//!
//! These measure end-to-end packing throughput, which now includes DEFLATE
//! compression for text entries (binary images stay stored).

use aiui_aix_pack::{InputFile, PackOptions};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};

/// Builds a set of repetitive text files plus a minimal app.json.
fn build_files(file_count: usize, file_size: usize) -> Vec<InputFile> {
    let mut files = Vec::with_capacity(file_count + 1);
    files.push(InputFile::new("app.json", br#"{"pages":[]}"#));
    let content = "the quick brown fox jumps over the lazy dog ".repeat(file_size / 45 + 1);
    for index in 0..file_count {
        files.push(InputFile::new(
            format!("data/file_{index}.txt"),
            content.clone(),
        ));
    }
    files
}

fn bench_pack(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack");
    for (file_count, file_size) in [(10usize, 1_024usize), (100, 1_024), (500, 4_096)] {
        let files = build_files(file_count, file_size);
        let total_bytes: u64 = files.iter().map(|file| file.data.len() as u64).sum();
        let id = BenchmarkId::new("text", format!("{file_count}x{file_size}"));
        group.throughput(Throughput::Bytes(total_bytes));
        group.bench_with_input(id, &files, |b, files| {
            b.iter_batched(
                || files.to_vec(),
                |files| {
                    let output = aiui_aix_pack::pack(
                        files,
                        PackOptions {
                            build_id: "bench-build".into(),
                            agent_id: None,
                            engine: Some("*".into()),
                            optimize: None,
                            signing_key: None,
                        },
                    )
                    .unwrap();
                    criterion::black_box(&output.data);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_pack);
criterion_main!(benches);
