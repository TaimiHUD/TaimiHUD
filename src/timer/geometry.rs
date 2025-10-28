use {
    glam::{
        f32::{Vec2, Vec3},
        swizzles::*,
    },
    serde::{Deserialize, Serialize},
    std::cmp::Ordering,
};

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy)]
#[serde(transparent)]
pub struct BlishVec3 {
    child: Vec3,
}

impl BlishVec3 {
    pub fn to_vec3(self) -> Vec3 {
        self.child.xzy()
    }

    pub fn from_vec3(vec3: Vec3) -> Self {
        BlishVec3 { child: vec3.xzy() }
    }

    pub fn from_raw_vec3(vec3: Vec3) -> Self {
        BlishVec3 { child: vec3 }
    }
}

pub type BlishPosition = Position<BlishVec3>;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(untagged)]
pub enum Position<V3 = Vec3> {
    Vec3(V3),
    Vec2(Vec2),
}

impl Position {
    fn to_vec2(self) -> Vec2 {
        match self {
            Self::Vec3(vec) => vec.xz(),
            Self::Vec2(vec) => vec,
        }
    }
    /*
     * this turns a 3d position into a 2d position :)
     * they wouldn't let me name it 23d
     * who needs an additional 20 dimensions, anyway?
     */
    fn d32(vec: Vec3) -> Position {
        Self::Vec2(Self::d3vec2(vec))
    }

    // help i am not good at computer how did i get here oh no
    fn d3vec2(vec: Vec3) -> Vec2 {
        Self::Vec3(vec).to_vec2()
    }

    /*
     * Min (is my wife in GW2,,,)
     */

    fn min3(&self, rhs: Vec3) -> Self {
        match *self {
            Self::Vec2(_vec) => self.min2(Self::d3vec2(rhs)),
            Self::Vec3(vec) => vec.min(rhs).into(),
        }
    }

    fn min2(&self, rhs: Vec2) -> Self {
        let lhs = self.to_vec2();
        lhs.min(rhs).into()
    }

    pub fn min(self, rhs: Position) -> Self {
        match rhs {
            Self::Vec3(vec) => self.min3(vec),
            Self::Vec2(vec) => self.min2(vec),
        }
    }

    /*
     * Maxxin all cool
     */

    fn max3(&self, rhs: Vec3) -> Self {
        match *self {
            Self::Vec2(_vec) => self.max2(Self::d3vec2(rhs)),
            Self::Vec3(vec) => vec.max(rhs).into(),
        }
    }

    fn max2(&self, rhs: Vec2) -> Self {
        let lhs = self.to_vec2();
        lhs.max(rhs).into()
    }

    pub fn max(self, rhs: Position) -> Self {
        match rhs {
            Self::Vec3(vec) => self.max3(vec),
            Self::Vec2(vec) => self.max2(vec),
        }
    }

    /*
     * Distance
     */

    fn distance3(&self, rhs: Vec3) -> f32 {
        match *self {
            Self::Vec3(vec) => vec.distance(rhs),
            Self::Vec2(vec) => vec.distance(Self::d3vec2(rhs)),
        }
    }

    fn distance2(&self, rhs: Vec2) -> f32 {
        let lhs = self.to_vec2();
        lhs.distance(rhs)
    }

    pub fn distance(&self, rhs: Self) -> f32 {
        match rhs {
            Self::Vec3(vec) => self.distance3(vec),
            Self::Vec2(vec) => self.distance2(vec),
        }
    }
}

impl From<Vec3> for Position {
    fn from(vec: Vec3) -> Self {
        Position::Vec3(vec)
    }
}

impl From<Vec2> for Position {
    fn from(vec: Vec2) -> Self {
        Position::Vec2(vec)
    }
}

impl From<Position> for Vec2 {
    fn from(pos: Position) -> Self {
        pos.to_vec2()
    }
}

impl Eq for Position {}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match other {
            Self::Vec3(rhs) => self.partial_cmp(rhs),
            Self::Vec2(rhs) => self.partial_cmp(rhs),
        }
    }
}

impl PartialEq<Vec3> for Position {
    fn eq(&self, other: &Vec3) -> bool {
        match self {
            Self::Vec2(lhs) => Self::d32(*other).eq(lhs),
            &Self::Vec3(lhs) => lhs.cmpeq(*other).all(),
        }
    }
}

impl PartialEq<Vec2> for Position {
    fn eq(&self, other: &Vec2) -> bool {
        let lhs = self.to_vec2();
        lhs.cmpeq(*other).all()
    }
}

impl PartialOrd<Vec3> for Position {
    fn partial_cmp(&self, other: &Vec3) -> Option<Ordering> {
        match self {
            Self::Vec2(lhs) => Self::d32(*other).partial_cmp(lhs).map(Ordering::reverse),
            &Self::Vec3(lhs) => {
                match (
                    lhs.cmpgt(*other).all(),
                    lhs.cmpeq(*other).all(),
                    lhs.cmplt(*other).all(),
                ) {
                    (true, false, false) => Some(Ordering::Greater),
                    (false, true, false) => Some(Ordering::Equal),
                    (false, false, true) => Some(Ordering::Less),
                    _ => None,
                }
            },
        }
    }
}

impl PartialOrd<Vec2> for Position {
    fn partial_cmp(&self, other: &Vec2) -> Option<Ordering> {
        let lhs = self.to_vec2();
        match (
            lhs.cmpgt(*other).all(),
            lhs.cmpeq(*other).all(),
            lhs.cmplt(*other).all(),
        ) {
            (true, false, false) => Some(Ordering::Greater),
            (false, true, false) => Some(Ordering::Equal),
            (false, false, true) => Some(Ordering::Less),
            _ => None,
        }
    }
}

// one day someone is going to look at this and think i'm deranged
// And that's Ok! they're right, i am :)
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Polytope {
    NSphere { center: Position, radius: f32 },
    NCuboid { pode: Position, antipode: Position },
}

impl Polytope {
    pub fn point_is_within(&self, player: Position) -> bool {
        match self {
            Polytope::NSphere { radius, center } => {
                // sphere
                center.distance(player) < *radius
            },
            Polytope::NCuboid { pode, antipode } => {
                let mins = pode.min(*antipode);
                let maxes = pode.max(*antipode);
                player >= mins && player <= maxes
            },
        }
    }
}

impl BlishPosition {
    pub fn to_sane(self) -> Position<Vec3> {
        match self {
            BlishPosition::Vec3(vec) => Position::Vec3(vec.to_vec3()),
            BlishPosition::Vec2(vec) => Position::Vec2(vec),
        }
    }
}

impl From<BlishPosition> for Position {
    fn from(pos: BlishPosition) -> Self {
        pos.to_sane()
    }
}
