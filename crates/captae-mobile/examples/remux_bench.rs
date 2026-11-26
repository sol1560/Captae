//! 量 Rust 封装（engine/remux.rs）的速度：选手机端编译配置（opt-level "s" 还是 "z"）时用。
//! 用法：`remux_bench <输入.ts> <输出目录>`，打印用了多久和每秒多少 MB
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (src, dir) = (std::path::PathBuf::from(&a[0]), std::path::PathBuf::from(&a[1]));
    let size = std::fs::metadata(&src).unwrap().len() as f64 / 1e6;
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        captae_core::engine::remux::remux(&src, None, &dir.join("out.mp4"), &dir.join("work"), |_| {}).unwrap();
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("{size:.0} MB，最快 {best:.2} 秒，{:.0} MB/s", size / best);
}
