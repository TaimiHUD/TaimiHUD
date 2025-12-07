use {
    std::{env, fs, path::Path, time::Instant},
    taimi_hoard::statistics::allocator::CounterAllocator,
    taimi_pack::{loader, Pack},
};
#[global_allocator]
static ALLOC: CounterAllocator = CounterAllocator::new();

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let fname = env::args_os().nth(1).expect("marker path to parse");
    let fname = Path::new(&fname);

    let meta = fs::metadata(fname);
    let mem_pre = CounterAllocator::total_allocated();
    let time_pre = Instant::now();
    let mut loader_zip;
    let mut loader_dir;
    let mut loader =
        if fname.extension().map(|ext| ext.eq_ignore_ascii_case("taco")) == Some(true) || !meta?.is_dir() {
            loader_zip = loader::ZipLoader::new(fname)?;
            &mut loader_zip as &mut dyn loader::PackLoaderContext
        } else {
            loader_dir = loader::DirectoryLoader::new(fname);
            &mut loader_dir as &mut dyn loader::PackLoaderContext
        };

    let pack = Pack::load_strict(&mut loader, true)?;
    let mem_consumed = CounterAllocator::total_allocated() - mem_pre;
    let time_consumed = time_pre.elapsed();

    eprintln!(
        "loaded pack {} with {} trails and {} pois in {:.3}s with {:.3} MB",
        pack.name,
        pack.trails.len(),
        pack.pois.len(),
        time_consumed.as_secs_f64(),
        (mem_consumed as f64) / (1024.0 * 1024.0),
    );

    Ok(())
}
