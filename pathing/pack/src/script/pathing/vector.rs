#[cfg(feature = "script-lua")]
use crate::script::lua::{IColour, IVec3};
use crate::script::user::{impl_source_tag, ScriptSourceTag};

pub trait InstanceColour {
    fn new_colour(c: [u8; 4]) -> Self
    where
        Self: Sized;
    fn get4(&self) -> [u8; 4];
    fn set_r(&mut self, r: u8);
    fn set_g(&mut self, g: u8);
    fn set_b(&mut self, b: u8);
    fn set_a(&mut self, a: u8);
    #[inline]
    fn get_r(&self) -> u8 {
        let [r, ..] = self.get4();
        r
    }
    #[inline]
    fn get_g(&self) -> u8 {
        let [_, g, ..] = self.get4();
        g
    }
    #[inline]
    fn get_b(&self) -> u8 {
        let [_, _, b, ..] = self.get4();
        b
    }
    #[inline]
    fn get_a(&self) -> u8 {
        let [_, _, _, a] = self.get4();
        a
    }

    #[inline]
    #[cfg(feature = "script-lua")]
    fn into_lua_colour(self) -> crate::script::lua::IColour
    where
        Self: Sized,
    {
        IColour(InstanceColour::new_colour(self.get4()))
    }
}
impl InstanceColour for u32 {
    #[inline]
    fn new_colour(c4: [u8; 4]) -> Self {
        u32::from_le_bytes(c4)
    }
    #[inline]
    fn get4(&self) -> [u8; 4] {
        self.to_le_bytes()
    }
    #[inline]
    fn set_r(&mut self, r: u8) {
        let v = r as u32;
        *self = v | (*self & 0xffffff00);
    }
    #[inline]
    fn set_g(&mut self, g: u8) {
        let v = (g as u32) << 8;
        *self = v | (*self & 0xffff00ff);
    }
    #[inline]
    fn set_b(&mut self, b: u8) {
        let v = (b as u32) << 16;
        *self = v | (*self & 0xff00ffff);
    }
    #[inline]
    fn set_a(&mut self, a: u8) {
        let v = (a as u32) << 24;
        *self = v | (*self & 0x00ffffff);
    }
}
impl InstanceColour for [u8; 4] {
    #[inline]
    fn new_colour(c4: [u8; 4]) -> Self {
        c4
    }
    #[inline]
    fn get4(&self) -> [u8; 4] {
        *self
    }
    #[inline]
    fn set_r(&mut self, r: u8) {
        let &mut [_, _, _, ref mut v] = self;
        *v = r;
    }
    #[inline]
    fn set_g(&mut self, g: u8) {
        let &mut [_, _, ref mut v, _] = self;
        *v = g;
    }
    #[inline]
    fn set_b(&mut self, b: u8) {
        let &mut [_, ref mut v, _, _] = self;
        *v = b;
    }
    #[inline]
    fn set_a(&mut self, a: u8) {
        let &mut [ref mut v, _, _, _] = self;
        *v = a;
    }
}
impl InstanceColour for glam::Vec4 {
    #[inline]
    fn new_colour([r, g, b, a]: [u8; 4]) -> Self {
        glam::Vec4::new(r as f32, g as f32, b as f32, a as f32) / 255.0
    }
    #[inline]
    fn get4(&self) -> [u8; 4] {
        let c = *self * 255.0;
        [c.x as u8, c.y as u8, c.z as u8, c.w as u8]
    }
    #[inline]
    fn set_r(&mut self, r: u8) {
        self.x = r as f32 / 255.0;
    }
    #[inline]
    fn set_g(&mut self, g: u8) {
        self.y = g as f32 / 255.0;
    }
    #[inline]
    fn set_b(&mut self, b: u8) {
        self.z = b as f32 / 255.0;
    }
    #[inline]
    fn set_a(&mut self, a: u8) {
        self.w = a as f32 / 255.0;
    }

    #[inline]
    fn get_r(&self) -> u8 {
        (self.x * 255.0) as u8
    }
    #[inline]
    fn get_g(&self) -> u8 {
        (self.y * 255.0) as u8
    }
    #[inline]
    fn get_b(&self) -> u8 {
        (self.z * 255.0) as u8
    }
    #[inline]
    fn get_a(&self) -> u8 {
        (self.w * 255.0) as u8
    }
}
impl_source_tag! {
    impl ScriptSourceTag for glam::Vec4 {}
}
pub trait InstanceVec3: ScriptSourceTag {
    fn new_vec3(v3: [f32; 3]) -> Self
    where
        Self: Sized;
    fn get3(&self) -> [f32; 3];
    fn set_x(&mut self, x: f32);
    fn set_y(&mut self, y: f32);
    fn set_z(&mut self, z: f32);

    fn vec3_length(&self) -> f32;
    fn vec3_dot<V>(&self, rhs: V) -> f32
    where
        V: InstanceVec3;
    fn vec3_cross<V>(&self, rhs: V) -> [f32; 3]
    where
        V: InstanceVec3;
    // NOTE: api creates new instances prior to returning these in-place op results
    fn vec3_norm(&mut self);
    fn vec3_recip(&mut self);
    fn vec3_negate(&mut self);
    fn vec3_mul_scalar(&mut self, amt: f32);
    fn vec3_mul_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3;
    fn vec3_div_component<V>(&mut self, mut rhs: V)
    where
        V: InstanceVec3,
    {
        rhs.vec3_recip();
        self.vec3_mul_component(rhs)
    }
    fn vec3_add_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3;
    fn vec3_sub_component<V>(&mut self, mut rhs: V)
    where
        V: InstanceVec3,
    {
        rhs.vec3_negate();
        self.vec3_add_component(rhs);
    }

    #[inline]
    #[cfg(feature = "script-lua")]
    fn into_lua_vec3(self) -> IVec3
    where
        Self: Sized,
    {
        IVec3(InstanceVec3::new_vec3(self.get3()))
    }
}
#[cfg(todo)]
impl InstanceVec3 for glam::Vec3 {}
impl InstanceVec3 for glam::Vec3A {
    #[inline]
    fn new_vec3(v3: [f32; 3]) -> Self {
        Self::from_array(v3)
    }
    #[inline]
    fn get3(&self) -> [f32; 3] {
        self.to_array()
    }
    #[inline]
    fn set_x(&mut self, x: f32) {
        self.x = x
    }
    #[inline]
    fn set_y(&mut self, y: f32) {
        self.y = y
    }
    #[inline]
    fn set_z(&mut self, z: f32) {
        self.z = z
    }

    #[inline]
    fn vec3_length(&self) -> f32 {
        self.length()
    }
    #[inline]
    fn vec3_dot<V>(&self, rhs: V) -> f32
    where
        V: InstanceVec3,
    {
        self.dot(Self::from(rhs.get3()))
    }
    #[inline]
    fn vec3_cross<V>(&self, rhs: V) -> [f32; 3]
    where
        V: InstanceVec3,
    {
        self.cross(Self::from(rhs.get3())).to_array()
    }
    #[inline]
    fn vec3_norm(&mut self) {
        *self = self.normalize_or_zero()
    }
    #[inline]
    fn vec3_recip(&mut self) {
        *self = self.recip()
    }
    #[inline]
    fn vec3_negate(&mut self) {
        *self = -*self
    }
    fn vec3_mul_scalar(&mut self, amt: f32) {
        *self = *self * amt
    }
    fn vec3_mul_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3,
    {
        *self = *self * Self::from(rhs.get3())
    }
    fn vec3_div_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3,
    {
        *self = *self / Self::from(rhs.get3())
    }
    fn vec3_add_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3,
    {
        *self = *self + Self::from(rhs.get3())
    }
    fn vec3_sub_component<V>(&mut self, rhs: V)
    where
        V: InstanceVec3,
    {
        *self = *self - Self::from(rhs.get3())
    }
    #[inline]
    #[cfg(feature = "script-lua")]
    fn into_lua_vec3(self) -> IVec3
    where
        Self: Sized,
    {
        self.into()
    }
}
impl_source_tag! {
    impl ScriptSourceTag for glam::Vec3A {}
    impl ScriptSourceTag for glam::Vec3 {}
}
