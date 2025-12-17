use {
    std::sync::Arc,
    taimi_pack::Pack,
};

#[derive(Debug)]
pub struct ActivePack {
    pub pack: Arc<Pack>,
}
