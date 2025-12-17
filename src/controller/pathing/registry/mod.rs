use {
    std::sync::Arc,
    core::fmt,
    taimi_pack::{loader::PackLoaderContext, Pack},
};

pub type LoaderBox = Box<dyn PackLoaderContext + Send + 'static>;

#[derive(Debug)]
pub struct ActivePack {
    pub pack: Arc<Pack>,
}

impl fmt::Display for ActivePack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.pack.name)
    }
}
