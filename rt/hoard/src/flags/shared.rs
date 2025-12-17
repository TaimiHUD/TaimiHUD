use {crate::flags::set::BitFlagForSet, radium::Radium};

#[repr(transparent)]
pub struct SharedFlags<T: BitFlagShared> {
    pub shared: T::SharedRepr,
}

pub trait BitFlagShared {
    type SharedRepr: Radium;
}
