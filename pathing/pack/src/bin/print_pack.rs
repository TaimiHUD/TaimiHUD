use {
    std::{collections::BTreeSet, env, ffi::OsStr, fs, path::Path, time::Instant},
    taimi_hoard::{lazyfmt, statistics::allocator::CounterAllocator},
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

    let relaxed = env::var_os("TAIMI_RELAXED")
        .as_ref()
        .map(|v| v.as_os_str() == OsStr::new("1"));
    let strict = !relaxed.unwrap_or(false);
    let mut pack = Pack::load_strict(&mut loader, strict)?;
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

    let mut seen = BTreeSet::new();
    for (_key, cat) in pack.categories.all_categories.iter() {
        let unseen = seen.insert(cat.full_id.as_id());
        #[cfg(todo = "unnecessary")]
        while let Some(parent) = id.parent() {
            if let Some(cat) = pack.categories.all_categories.get(parent) {
                assert_eq!(cat.full_id.as_id(), parent);
            } else {
                panic!("missing {parent:?} for {cat:?}");
            }
            id = parent;
        }
        if unseen {
            log::trace!("cat: {}", cat.full_id.as_id());
            if cat.full_id.as_id().as_str().ends_with(".") {
                log::warn!("invalid category {:?}", cat.full_id.as_id());
            }
        } else {
            log::error!("duplicate cat??? {}", cat.full_id.as_id());
        }
    }
    seen.clear();
    for trail in pack.trails.iter() {
        let trl_name = trail.trail_path.as_ref().map(|p| &p.path[..]).unwrap_or("<unavail>");
        let context = lazyfmt::StrFmt::fmt_fn(|f| write!(f, "{trl_name} ({trail})"));
        let res = context.annotate_result(trail.read_trl_data(&mut loader));
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
                    log::trace!("cap section#{_sectioni} was empty");
                }
            }
        }
        if empty_count > 0 {
            log::trace!("{empty_count} sections empty in {context}");
        } else if trl.sections.is_empty() {
            log::debug!("empty trail? {context}");
        }
    }

    Ok(())
}
