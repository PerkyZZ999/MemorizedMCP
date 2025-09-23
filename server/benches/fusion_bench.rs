use criterion::{criterion_group, criterion_main, Criterion, black_box};

fn bench_vector_distance(c: &mut Criterion) {
    let dim = 384usize;
    let a: Vec<f32> = (0..dim).map(|i| (i as f32).sin()).collect();
    let b: Vec<f32> = (0..dim).map(|i| (i as f32).cos()).collect();
    c.bench_function("cosine_distance_384", |bch| {
        bch.iter(|| {
            let mut dot = 0.0f32; let mut na = 0.0f32; let mut nb = 0.0f32;
            for i in 0..dim {
                let x = unsafe { *a.get_unchecked(i) };
                let y = unsafe { *b.get_unchecked(i) };
                dot += x*y; na += x*x; nb += y*y;
            }
            let sim = dot / (na.sqrt() * nb.sqrt() + 1e-8);
            black_box(sim)
        });
    });
}

fn bench_ann_search(c: &mut Criterion) {
    // Simple ANN-like greedy on in-memory vectors for baseline
    let dim = 384usize;
    let n = 2000usize;
    let db: Vec<Vec<f32>> = (0..n).map(|i| (0..dim).map(|j| ((i*j) as f32).sin()).collect()).collect();
    let q: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.37).cos()).collect();
    c.bench_function("ann_greedy_2000x384", |bch| {
        bch.iter(|| {
            let mut best = -1.0f32; let mut best_i = 0usize;
            for (i, v) in db.iter().enumerate() {
                let mut dot = 0.0f32; let mut na = 0.0f32; let mut nb = 0.0f32;
                for k in 0..dim { let x = unsafe{*q.get_unchecked(k)}; let y = unsafe{*v.get_unchecked(k)}; dot+=x*y; na+=x*x; nb+=y*y; }
                let sim = dot / (na.sqrt()*nb.sqrt()+1e-8);
                if sim > best { best = sim; best_i = i; }
            }
            black_box((best, best_i))
        });
    });
}

criterion_group!(benches, bench_vector_distance, bench_ann_search);
criterion_main!(benches);


