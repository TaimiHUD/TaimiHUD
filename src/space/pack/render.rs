use {
    crate::controller::pathing::space::DrawSpace,
    glamour::{Point3, Vector3},
    std::collections::BinaryHeap,
    taimi_hoard::cmp::CmpIgnore,
};

/// BvhIter expected to produce positions, which should be [Point3::INFINITY]
/// for items that can ignore the distance priority queue
pub struct RenderOrderBuilder<'a, T, BvhIter> {
    pub bvh_iter: BvhIter,
    pub draw_order_heap: &'a mut BinaryHeap<HeapEntityOf<T>>,
    pub cam_origin: Point3<DrawSpace>,
    pub cam_dir: Vector3<DrawSpace>,
}

impl<'a, T, BvhIter> Iterator for RenderOrderBuilder<'a, T, BvhIter>
where
    BvhIter: Iterator<Item = (Point3<DrawSpace>, T)>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((position, entity)) = self.bvh_iter.next() {
            let cam_dist = match position {
                pos if pos.x.is_infinite() => return Some(entity),
                #[cfg(todo)]
                true => {
                    // TODO: broken or inaccurate idk
                    let cam_dist = (position - self.cam_origin).dot(self.cam_dir);
                    let cam_dist = f32::to_bits(cam_dist) as i32;
                    let cam_dist = cam_dist ^ ((cam_dist >> 30) as u32 >> 1) as i32;
                    cam_dist
                },
                position => (position.distance_squared(self.cam_origin) * 1_000_000.0)
                    .min(0x40000000i32 as f32) as i32,
            };
            self.draw_order_heap
                .push(HeapEntity { cam_dist, value: CmpIgnore(entity) });
        }

        self.draw_order_heap.pop().map(|he| he.value.0)
    }
}

pub type RenderOrderHeap<T> = BinaryHeap<HeapEntity<CmpIgnore<T>>>;
pub type HeapEntityOf<T> = HeapEntity<CmpIgnore<T>>;
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeapEntity<T> {
    cam_dist: i32,
    value: T,
}
