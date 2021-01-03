use criterion::{black_box, criterion_group, criterion_main, Criterion};
// use zas_lsif_tools::Indexer;
use indexer::Indexer;

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("threejs", |b| {
        b.iter(|| {
            let mut opt: Opts = Opts {
                project_root: PathBuf::from("/Users/zas/Dev/three.js"),
                language: Language::JavaScript,
                output: None,
            };
            opt.canonicalize_paths();

            let output = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&opt.output.clone().unwrap())
                .unwrap();

            let (emitter, signal_receiver) = FileEmitter::new(output);

            Indexer::index(black_box(opt), black_box(emitter)).unwrap();

            // Wait until the buffer is flushed
            signal_receiver.recv().unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
