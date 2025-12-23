use {
    anyhow::Context,
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

    let mut pack = Pack::load_strict(&mut loader, true)?;
    pack.categories.trim_attributes();
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

    for (traili, trail) in pack.trails.iter().enumerate() {
        let trl_name = trail.trail_path.as_ref().map(|p| &p[..]).unwrap_or("<unavail>");
        let context = format!("{trl_name} (trail#{traili} of {})", &trail.category);
        let res = trail.read_trl_data(&mut loader)
            .context(context.clone());
        let trl = match res {
            Ok(t) => t,
            Err(e) => {
                log::error!("{e:#}");
                continue
            },
        };
        let mut empty_count = 0usize;
        for (_sectioni, section) in trl.sections.iter().enumerate() {
            if section.is_empty() {
                empty_count += 1;
                if _sectioni == 0 || _sectioni + 1 == trl.sections.len() {
                    eprintln!("cap section#{_sectioni} was empty");
                }
            }
        }
        if empty_count > 0 {
            eprintln!("{empty_count} sections empty in {context}");
        } else if trl.sections.is_empty() {
            eprintln!("empty trail? {context}");
        }
    }

    Ok(())
}
