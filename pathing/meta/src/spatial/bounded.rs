use {
    crate::{
        coords::{LocalPoint2, LocalSpace},
        spatial::{box2aabb, box3aabb, ConstNan, MintConv},
    },
    bvh::{
        aabb,
        ball,
        bounding_hierarchy::{BHShape, BHValue},
        bvh::Bvh,
    },
    glamour::{Box2, Box3, FloatScalar, Point3, Unit, Vector2, Vector3},
    num_traits::Signed,
    std::{cmp, mem, ops},
};

pub struct BvhEntities<T, P = usize, const D: usize = 3> {
    entities: Vec<T>,
    group_ranges: Vec<(P, ops::Range<usize>)>,
    bvh: Bvh<f32, D>,
    dirty_ranges: bool,
    dirty_from: usize,
}

impl<T, P, const D: usize> BvhEntities<T, P, D> {
    pub const fn empty() -> Self {
        Self {
            entities: Vec::new(),
            group_ranges: Vec::new(),
            bvh: Bvh { nodes: Vec::new() },
            dirty_ranges: false,
            dirty_from: 0,
        }
    }
    const EMPTY_RANGE: ops::Range<usize> = 0..0;

    pub fn entities(&self) -> &Vec<T> {
        &self.entities
    }
    pub fn bvh(&self) -> &Bvh<f32, D> {
        &self.bvh
    }
}
impl<T: BHShape<f32, D>, P, const D: usize> BvhEntities<T, P, D> {
    pub unsafe fn drain_entities_at(&mut self, group_idx: usize) -> Box<[T]> {
        let range = unsafe {
            let (_, range) = self.group_ranges.get_unchecked_mut(group_idx);
            mem::replace(range, Self::EMPTY_RANGE)
        };
        self.dirty_ranges = true;
        let amt = range.len();
        let range = range.clone();
        if range.start < self.dirty_from {
            // TODO: could intelligently/partially rebuild using damage masks or using the swap argument?
            for i in range.start..range.end {
                self.bvh.remove_shape(&mut self.entities, i, false);
            }
            #[cfg(todo = "unnecessary")]
            {
                for i in range.end..self.dirty_from {
                    self.bvh.remove_shape(&mut self.entities, i, false);
                }
                self.dirty_from = self.dirty_from.min(range.start);
            }
        }
        let entities = self.entities.drain(range.clone()).collect::<Box<[T]>>();
        if amt > 0 {
            // shift everything down...
            let later_ranges = self
                .group_ranges
                .iter_mut()
                .filter(|(_p, r)| r.start >= range.end);
            for (_p, later) in later_ranges {
                later.start -= amt;
                later.end -= amt;
            }
            #[cfg(todo = "unnecessary")]
            let later_ranges = self.group_ranges.iter().filter(|(_p, r)| r.start >= new_end);
            #[cfg(todo = "unnecessary")]
            for (_p, later) in later_ranges {
                for i in later.clone() {
                    self.bvh.add_shape(&mut self.entities, i);
                }
            }
        }
        entities
    }

    pub unsafe fn add_group_at<E>(&mut self, group_idx: usize, entities: E, append: bool)
    where
        E: IntoIterator<Item = T>,
    {
        let mut start = self.entities.len();
        let prev_entities;
        let (_, range) = unsafe {
            let prev_range = {
                let (_, prev_range) = self.group_ranges.get_unchecked(group_idx);
                prev_range.clone()
            };
            let keep = match prev_range.is_empty() {
                false if prev_range.end == start && append => {
                    // the final group can just be appended to directly...
                    start = prev_range.start;
                    true
                },
                empty => empty,
            };
            prev_entities = match (!keep).then(|| self.drain_entities_at(group_idx)) {
                _ if !append => None,
                pe => pe,
            };
            self.group_ranges.get_unchecked_mut(group_idx)
        };
        let prev_start = self.entities.len();
        range.start = start.min(prev_start);
        self.entities
            .extend(prev_entities.into_iter().flatten().chain(entities));
        range.end = self.entities.len();
        self.dirty_from = self.dirty_from.min(prev_start);
        self.dirty_ranges = true;
    }

    pub unsafe fn append_to_group_at<E>(&mut self, group_idx: usize, entities: E)
    where
        E: IntoIterator<Item = T>,
    {
        let entities = entities.into_iter();
        if entities.size_hint().1 == Some(0) {
            // short-circuit on empty changes because this is awkward...
            return
        }

        #[cfg(todo)]
        if prev_range.end < prev_len {
            let prev_len = self.entities.len();
            let prev_amt = prev_range.len();
            // TODO: could get away with marking less dirty if some full packs were moved, but bleh...
            let new_start = prev_len - prev_amt;
            let offset = new_start - prev_range.start;
            for i in (prev_range.start..new_start).rev() {
                let new_i = i + offset;
                let truncated = unsafe { self.entities.get_unchecked_mut(..=new_i) };
                let swap_end = true;
                self.bvh.remove_shape(truncated, i, swap_end);
                self.entities.swap(i, new_i);
            }
            let prev_end = prev_range.end;
            range.start = new_start;
            range.end = prev_len;
            self.dirty_ranges = true;
            self.dirty_from = self.dirty_from.min(new_start.min(prev_end));
        };

        self.add_group_at(group_idx, entities, true)
    }

    fn prepare_ranges(&mut self) {
        if !self.dirty_ranges {
            return
        }

        // canonicalize empty ranges to avoid weirdness when appending
        let empty_groups = self
            .group_ranges
            .iter_mut()
            .filter(|(_p, r)| r.start > 0 && r.is_empty());
        for (_p, empty) in empty_groups {
            *empty = Self::EMPTY_RANGE;
        }

        self.group_ranges
            .sort_unstable_by_key(|(_, r)| (!r.is_empty(), r.start));

        self.dirty_ranges = false;
    }

    pub fn prepare(&mut self) {
        let len = self.entities.len();
        if self.dirty_from < len {
            if self.dirty_from >= len * 3 / 4 {
                for i in self.dirty_from..len {
                    self.bvh.add_shape(&mut self.entities, i);
                }
            } else {
                self.rebuild();
            }
        }
        self.dirty_from = len;
        self.prepare_ranges();
    }

    /// TODO: executor argument for parallel build
    pub fn rebuild(&mut self) {
        self.prepare_ranges();
        if self.entities.capacity() / 4 > self.entities.len() {
            self.entities.shrink_to_fit();
        }
        // free memory first, because apparently we can't reuse it :<
        let _ = mem::take(&mut self.bvh.nodes);
        self.bvh = Bvh::build(&mut self.entities);
    }

    pub fn clear(&mut self) {
        let _ = mem::take(&mut self.bvh.nodes);
        self.entities.clear();
        self.group_ranges.clear();
    }
}
impl<T: BHShape<f32, D>, P, const D: usize> BvhEntities<T, P, D>
where
    P: PartialEq,
{
    pub fn set_group<E>(&mut self, path: P, entities: E)
    where
        E: IntoIterator<Item = T>,
    {
        let group_idx = match self.group_index(&path) {
            Some(i) => i,
            None => {
                let i = self.group_ranges.len();
                self.group_ranges.push((path, Self::EMPTY_RANGE));
                self.dirty_ranges = true;
                i
            },
        };
        unsafe { self.add_group_at(group_idx, entities, false) }
    }

    pub fn append_to_group<E>(&mut self, path: &P, entities: E) -> Result<(), E>
    where
        E: IntoIterator<Item = T>,
    {
        let group_idx = match self.group_index(&path) {
            Some(i) => i,
            None => return Err(entities),
        };
        Ok(unsafe { self.append_to_group_at(group_idx, entities) })
    }

    pub fn remove_group(&mut self, path: &P) -> Option<Box<[T]>> {
        let group_idx = match self.group_index(&path) {
            Some(i) => i,
            None => return None,
        };
        Some(unsafe {
            let entities = self.drain_entities_at(group_idx);
            self.group_ranges.swap_remove(group_idx);
            self.dirty_ranges = true;
            entities
        })
    }

    pub fn group_index(&self, path: &P) -> Option<usize> {
        self.group_ranges.iter().position(|(p, _)| p == path)
    }

    pub fn group_index_of(&self, entity: &T) -> Option<(usize, &P, ops::Range<usize>)> {
        let idx = unsafe { (entity as *const T).offset_from_unsigned(self.entities.as_ptr()) };
        match idx {
            idx => {
                let group_idx = self
                    .group_ranges
                    .binary_search_by(|(_p, r)| match r.start.cmp(&idx) {
                        cmp::Ordering::Greater => cmp::Ordering::Greater,
                        cmp::Ordering::Less | cmp::Ordering::Equal if r.end < idx => cmp::Ordering::Equal,
                        _ => cmp::Ordering::Less,
                    })
                    .ok();
                group_idx.map(|idx| unsafe { self.group_ranges.get_unchecked(idx) })
            },
            #[cfg(todo)]
            idx => self.group_ranges.iter().find(|(_p, r)| r.contains(&idx)),
        }
        .map(move |(p, r)| (idx, p, r.clone()))
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BvhShape<T> {
    pub value: T,
    /// the trait isn't unsafe, but try not to mess with this...
    pub bh_index: usize,
}
impl<T> BvhShape<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value, bh_index: 0 }
    }
    #[inline]
    pub const fn new_removed(value: T) -> Self {
        Self { value, bh_index: usize::MAX }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }

    #[inline]
    pub fn set_bh_removed(&mut self) {
        self.bh_index = usize::MAX;
    }
    #[inline]
    pub fn is_bh_removed(&self) -> bool {
        self.bh_index == usize::MAX
    }
    #[inline]
    pub fn is_bh_removed_from<U: BHValue, const D: usize>(&self, bvh: &Bvh<U, D>) -> bool {
        self.bh_index >= bvh.nodes.len()
    }
}
impl<T, U: BHValue, const D: usize> aabb::Bounded<U, D> for BvhShape<T>
where
    T: aabb::Bounded<U, D>,
{
    #[inline]
    fn aabb(&self) -> aabb::Aabb<U, D> {
        self.value.aabb()
    }
}
impl<T, U: BHValue, const D: usize> BHShape<U, D> for BvhShape<T>
where
    Self: aabb::Bounded<U, D>,
{
    #[inline]
    fn set_bh_node_index(&mut self, idx: usize) {
        self.bh_index = idx
    }
    #[inline]
    fn bh_node_index(&self) -> usize {
        self.bh_index
    }
}
impl<T> ops::Deref for BvhShape<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> ops::DerefMut for BvhShape<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
impl<T> AsRef<T> for BvhShape<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.value
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriggerBoundsInfo<U: Unit = LocalSpace> {
    pub position: Point3<U>,
    pub radius: U::Scalar,
}
impl<U: Unit> TriggerBoundsInfo<U>
where
    U::Scalar: Signed,
{
    pub fn new(position: Point3<U>, radius: U::Scalar, auto: bool) -> Self {
        Self {
            position,
            radius: match auto {
                true => radius,
                false => -radius,
            },
        }
    }
    pub const fn with_parts(position: Point3<U>, radius: U::Scalar) -> Self {
        Self { position, radius }
    }

    pub fn radius(&self) -> U::Scalar {
        self.radius.abs()
    }
    pub fn set_radius(&mut self, value: U::Scalar) {
        self.radius = self.radius.signum() * value;
    }
    pub fn is_auto(&self) -> bool {
        !self.radius.is_negative()
    }
    pub fn set_auto(&mut self, auto: bool) {
        self.radius = self.radius();
        if !auto {
            self.radius = -self.radius;
        }
    }
    pub fn to_sphere(&self) -> ball::Sphere<U::Scalar>
    where
        U::Scalar: BHValue + nalgebra::SimdValue,
        Point3<U>: MintConv<MintNalg = nalgebra::Point3<U::Scalar>>,
    {
        ball::Ball::new(self.position.into_nalg(), self.radius())
    }
}
impl<U: Unit> TriggerBoundsInfo<U>
where
    U::Scalar: FloatScalar + Signed + ConstNan,
{
    pub const INVALID: Self = Self::with_parts(Point3::INFINITY, <U::Scalar as ConstNan>::NAN_NEG);
}
impl TriggerBoundsInfo<LocalSpace> {
    pub fn position2(&self) -> LocalPoint2 {
        LocalSpace::to2(self.position)
    }
    pub fn to_circle(&self) -> ball::Circle<<LocalSpace as Unit>::Scalar>
    where
        LocalPoint2: MintConv<MintNalg = nalgebra::Point2<<LocalSpace as Unit>::Scalar>>,
    {
        ball::Ball::new(self.position2().into_nalg(), self.radius())
    }
}

impl aabb::Bounded<<LocalSpace as Unit>::Scalar, 2> for TriggerBoundsInfo<LocalSpace> {
    fn aabb(&self) -> aabb::Aabb<<LocalSpace as Unit>::Scalar, 2> {
        let corner = Vector2::<LocalSpace>::splat(self.radius());
        let position = self.position2();
        let bounds = Box2::new(position - corner, position + corner);
        box2aabb(bounds)
    }
}
impl<U: Unit> aabb::Bounded<U::Scalar, 3> for TriggerBoundsInfo<U>
where
    U::Scalar: Signed + BHValue,
    Point3<U>: MintConv<MintNalg = nalgebra::Point3<U::Scalar>>,
{
    fn aabb(&self) -> aabb::Aabb<U::Scalar, 3> {
        let corner = Vector3::<U>::splat(self.radius());
        let bounds = Box3::new(self.position - corner, self.position + corner);
        box3aabb(bounds)
    }
}
