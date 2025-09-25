use {
    crate::{
        attributes::MarkerAttributes,
        loader::PackLoaderContext,
        pack::{taco_safe_name, taco_xml_to_guid},
    },
    anyhow::Context,
    core::f32,
    glamour::{point3, Box3, Point3, Size3},
    std::{
        fmt,
        io::{self, BufReader, Read},
        path::Path,
    },
    uuid::Uuid,
};

pub struct Trail {
    pub category: String,
    pub guid: Uuid,
    pub data: TrailData,
    pub attributes: MarkerAttributes,
    pub parent_path: Option<String>,
}

impl Trail {
    pub fn from_xml(
        ctx: &mut impl PackLoaderContext,
        asset: &str,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Trail> {
        let mut category = String::new();
        let mut trail_path = None;
        let mut guid = None;
        let mut map_id = None::<i32>;
        let mut attributes = MarkerAttributes::default();
        let mut attributes_bh = MarkerAttributes::default();

        for attr in attrs {
            let res = if attr.name.local_name.eq_ignore_ascii_case("type") {
                category = taco_safe_name(&attr.value, true);
                Ok(true)
            } else if attr.name.local_name.eq_ignore_ascii_case("traildata") {
                trail_path = Some(attr.value);
                Ok(true)
            } else if attr.name.local_name.eq_ignore_ascii_case("guid") {
                guid = Some(taco_xml_to_guid(&attr.value));
                Ok(true)
            } else if attr.name.local_name.eq_ignore_ascii_case("mapid") {
                attr.value.parse()
                    .map(|v| {
                        map_id = Some(v);
                        true
                    })
                    .map_err(From::from)
            } else if attr.name.local_name.starts_with("bh-") {
                attributes_bh.try_add(attr.name.borrow(), attr.value)
            } else {
                attributes.try_add(attr.name.borrow(), attr.value)
            }.with_context(|| format!("Trail attribute '{}'", attr.name));
            match res {
                Err(e) =>
                    log::warn!("{e:#}"),
                Ok(false) =>
                    log::info!("unrecognized trail attribute `{}`", attr.name),
                Ok(true) => (),
            }
        }

        if category.is_empty() {
            anyhow::bail!("No 'type' specified for Trail");
        }

        let Some(trail_path) = trail_path else {
            anyhow::bail!("No 'trailData' specified for Trail '{category}'");
        };

        let data = read_trl_file(BufReader::new(ctx.find_asset_near(asset, &trail_path)?), &trail_path)?;
        if let Some(map_id) = map_id {
            if map_id != data.map_id {
                log::warn!("trail MapID mismatch on {trail_path}: {map_id} vs {} from trl", data.map_id);
            }
        }
        let guid = guid.unwrap_or_default();

        let parent_path = Path::new(asset).parent()
            .map(|p| p.to_string_lossy().into());

        // TODO: support bh features properly...
        attributes.merge(&attributes_bh);

        Ok(Trail {
            category,
            guid,
            data,
            attributes,
            parent_path,
        })
    }

    #[inline]
    pub fn texture_name(&self) -> Option<&str> {
        self.attributes.texture.as_ref().map(|s| &s[..])
    }

    #[inline]
    pub fn scale(&self) -> f32 {
        self.attributes.trail_scale.unwrap_or(1.0)
    }

    #[inline]
    pub fn is_wall(&self) -> bool {
        self.attributes.is_wall.unwrap_or(false)
    }
}

pub struct TrailData {
    pub map_id: i32,
    pub sections: Vec<TrailSection>,
}

pub struct TrailSection {
    pub points: Vec<Point3>,
    pub bounds: Box3,
}

pub fn read_trl_file(mut reader: impl Read, name: &str) -> anyhow::Result<TrailData> {
    let mut buf32 = [0u8; 4];
    reader
        .read_exact(&mut buf32)
        .context("Reading trail version")?;
    if i32::from_le_bytes(buf32) != 0 {
        anyhow::bail!("Trl version '0' is the only known valid format version");
    }

    reader
        .read_exact(&mut buf32)
        .context("Reading trail map_id")?;
    let map_id = i32::from_le_bytes(buf32);

    let mut sections = vec![];
    let mut current_section = vec![];

    const NEG_BOX: Box3 = glamour::Box3 {
        min: point3!(f32::INFINITY, f32::INFINITY, f32::INFINITY),
        max: point3!(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    let mut read_more = true;
    while read_more {
        let point_data = match read_point(&mut reader) {
            Ok(point_data) => point_data,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                read_more = false;
                EMPTY_POINT
            }
            Err(e) => return Err(e).context("Reading trail sections"),
        };

        if point_data == EMPTY_POINT {
            if !current_section.is_empty() {
                sections.push(TrailSection {
                    bounds: match current_section.is_empty() {
                        true => NEG_BOX,
                        false => {
                            let mut bounds = Box3::<f32>::from_points(current_section.iter().copied());
                            if bounds.is_empty() {
                                // empty boxes are invalid...
                                let size = bounds.size();
                                let size = Size3::new(
                                    size.width.max(f32::EPSILON),
                                    size.height.max(f32::EPSILON),
                                    size.depth.max(f32::EPSILON),
                                );
                                bounds.max = bounds.min + size.to_vector();
                            }
                            bounds
                        },
                    },
                    points: std::mem::take(&mut current_section),
                });
            } else {
                log::debug!("Empty trail section in {name}");
            }
        } else {
            let x = f32::from_le_bytes(point_data[0]);
            let y = f32::from_le_bytes(point_data[1]);
            let z = f32::from_le_bytes(point_data[2]);
            let point = point3!(x, y, z);
            current_section.push(point);
        }
    }

    Ok(TrailData { map_id, sections })
}

const EMPTY_POINT: [[u8; 4]; 3] = [[0; 4]; 3];

fn read_point(reader: &mut impl Read) -> io::Result<[[u8; 4]; 3]> {
    let mut point_data = [[0; 4]; 3];
    reader.read_exact(&mut point_data[0])?;
    reader.read_exact(&mut point_data[1])?;
    reader.read_exact(&mut point_data[2])?;
    Ok(point_data)
}

impl fmt::Display for Trail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let guid = &self.guid;
        match &self.parent_path {
            Some(parent) =>
                write!(f, "{parent}/{guid}"),
            None =>
                write!(f, "{guid}"),
        }
    }
}
