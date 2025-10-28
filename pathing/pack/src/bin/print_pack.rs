use {
    std::{collections::BTreeMap, env, fs, path::Path},
    taimi_pack::{loader, Pack},
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let fname = env::args_os().nth(1).expect("marker path to parse");
    let fname = Path::new(&fname);

    let meta = fs::metadata(fname);
    let mut loader_zip;
    let mut loader_dir;
    let mut loader = if fname
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("taco"))
        == Some(true)
        || !meta?.is_dir()
    {
        loader_zip = loader::ZipLoader::new(fname)?;
        &mut loader_zip as &mut dyn loader::PackLoaderContext
    } else {
        loader_dir = loader::DirectoryLoader::new(fname);
        &mut loader_dir as &mut dyn loader::PackLoaderContext
    };

    let pack = Pack::load_strict(&mut loader, true)?;

    eprintln!(
        "loaded pack {} with {} trails and {} pois",
        pack.name,
        pack.trails.len(),
        pack.pois.len()
    );

    Ok(())
}
