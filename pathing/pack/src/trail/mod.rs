use {
    crate::{
        attributes::{AttrString, MarkerAttributes},
        category::id::IdNameBox,
        loader::{LoaderAssetReader, PackLoaderContext},
        pack::{taco_safe_name, taco_xml_to_guid, PackBuilderMarkerWarnings},
    },
    anyhow::Context,
    core::f32,
    glamour::{Box3, Point3, Size3},
    std::{
        fmt,
        io::{self, BufRead, Read},
        mem,
    },
    uuid::Uuid,
};

#[derive(Debug, Clone)]
pub struct Trail {
    pub category: IdNameBox,
    pub guid: Uuid,
    pub attributes: MarkerAttributes,
    pub parent_path: Option<AttrString>,
    pub trail_path: Option<String>,
    pub map_id: Option<i32>,
}

impl Trail {
    pub fn from_xml(
        warnings: &mut PackBuilderMarkerWarnings,
        asset_parent: Option<&AttrString>,
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
                if !attr.value.is_empty() {
                    guid = Some(taco_xml_to_guid(&attr.value));
                }
                Ok(true)
            } else if attr.name.local_name.eq_ignore_ascii_case("mapid") {
                attr.value
                    .parse()
                    .map(|v| {
                        map_id = Some(v);
                        true
                    })
                    .map_err(From::from)
            } else if attr.name.local_name.starts_with("bh-") {
                attributes_bh.try_add(attr.name.borrow(), attr.value)
            } else {
                attributes.try_add(attr.name.borrow(), attr.value)
            }
            .with_context(|| format!("Trail attribute '{}'", attr.name));
            match res {
                Err(e) => log::warn!("{e:#}"),
                Ok(false) => {
                    warnings.attr_warning(&attr.name, &"Trail");
                },
                Ok(true) => (),
            }
        }

        #[cfg(feature = "fixup-fvd")]
        {
            /// `GUID="/nvefCms5kWE7aSlgxB5KA" trailData="Data/Trails/ARAH/ARAH_P2_MineSkip.trl"`
            const TYPE_ARAH2_GUID: Uuid =
                match Uuid::try_parse_ascii(b"7cde7bfe-ac29-45e6-84ed-a4a583107928") {
                    Ok(u) => u,
                    Err(..) => unreachable!(),
                };
            const TYPE_ARAH2: &'static str = "fvd_guide.arah.p2.path";
            if category.is_empty() && guid == Some(TYPE_ARAH2_GUID) {
                category = TYPE_ARAH2.into();
            }
        }
        if category.is_empty() {
            anyhow::bail!("No 'type' specified for Trail");
        }

        let guid = guid.unwrap_or_default();

        // TODO: support bh features properly...
        attributes.merge(&attributes_bh, false);

        Ok(Self {
            category: category.into(),
            guid,
            attributes,
            parent_path: asset_parent.cloned(),
            trail_path,
            map_id,
        })
    }

    /// Populate ID from [trail data](Self::read_trl_data)
    pub fn update_map_id(&mut self, ctx: &mut dyn PackLoaderContext) -> anyhow::Result<()> {
        let mut asset = self.open_trl_data(ctx)?;
        let header = TrailHeader::read_from_trl(&mut asset)
            .with_context(|| format!("Reading trail header from {self}"))?;
        self.map_id = Some(header.map_id);
        Ok(())
    }

    pub fn open_trl_data<'l>(
        &self,
        ctx: &mut dyn PackLoaderContext,
    ) -> anyhow::Result<Box<dyn LoaderAssetReader>> {
        let Some(trail_path) = &self.trail_path else {
            anyhow::bail!("No 'trailData' specified for Trail '{self}'");
        };
        match &self.parent_path {
            Some(parent) => ctx.find_asset_near(parent, trail_path),
            None => ctx.load_asset_dyn(trail_path),
        }
    }

    pub fn read_trl_data(&self, ctx: &mut dyn PackLoaderContext) -> anyhow::Result<TrailData> {
        let mut asset = self.open_trl_data(ctx)?;
        let data =
            TrailData::read_from_trl(&mut asset).with_context(|| format!("Reading trail data from {self}"));

        match (self.map_id, &data) {
            (Some(map_id), Ok(data)) if data.header.map_id != map_id => {
                log::warn!(
                    "MapID mismatch on {self}: expected {map_id} but read {} from trl",
                    data.header.map_id
                );
            },
            _ => (),
        }

        data
    }

    #[inline]
    pub fn texture_name(&self) -> Option<&str> {
        self.attributes
            .get_trail()
            .and_then(|trail| trail.texture.as_ref())
            .map(|s| &s[..])
    }

    #[inline]
    pub fn scale(&self) -> f32 {
        self.attributes
            .get_trail()
            .and_then(|trail| trail.trail_scale)
            .unwrap_or(1.0)
    }

    #[inline]
    pub fn is_wall(&self) -> bool {
        self.attributes
            .get_trail()
            .and_then(|trail| trail.is_wall)
            .unwrap_or(false)
    }
}

impl fmt::Display for Trail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let category = &self.category;
        let guid = &self.guid;
        match &self.parent_path {
            Some(parent) => write!(f, "{parent}{category}/{guid}"),
            None => write!(f, "{category}/{guid}"),
        }
    }
}

/// Version '0' is the only known .trl format
pub type TrailHeader = TrailHeader0;

/// trl file format [version 0](Self::VERSION)
#[derive(Debug, Clone, Default)]
pub struct TrailHeader0 {
    pub map_id: i32,
}

impl TrailHeader0 {
    pub const VERSION: u32 = 0;
    pub const SIZE: usize = 8;

    pub const fn with_map_id(map_id: i32) -> Self {
        Self { map_id }
    }

    pub fn read_from_trl<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; Self::SIZE];
        reader.read_exact(&mut buf)?;

        let [v0, v1, v2, v3, m0, m1, m2, m3] = buf;
        let version = u32::from_le_bytes([v0, v1, v2, v3]);
        let map_id = i32::from_le_bytes([m0, m1, m2, m3]);

        match version {
            Self::VERSION => Ok(Self::with_map_id(map_id)),
            version => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported .trl version {version}, expected version 0"),
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrailData {
    pub header: TrailHeader,
    pub sections: Box<[TrailSection]>,
}

impl TrailData {
    const POINT_BUFFER_LEN: usize = 256;
    const AVERAGE_SECTION_LEN: usize = 32;

    pub fn read_from_trl<R: Read>(read: &mut R) -> anyhow::Result<Self> {
        let mut read =
            io::BufReader::with_capacity(TrailSection::POINT_SIZE * Self::POINT_BUFFER_LEN, read);
        let header = TrailHeader::read_from_trl(&mut read).context("Reading trail header")?;
        let read_ahead_points = read.fill_buf()?.len() / TrailSection::POINT_SIZE;
        let mut points_buf = Vec::with_capacity(Self::AVERAGE_SECTION_LEN.min(read_ahead_points));
        let mut sections = Vec::with_capacity(
            (read_ahead_points + Self::AVERAGE_SECTION_LEN - 1) / Self::AVERAGE_SECTION_LEN,
        );
        TrailSection::read_all_from_trl(&mut read, &mut sections, &mut points_buf)
            .context("Reading trail sections")?;

        Ok(Self {
            header,
            sections: sections.into_boxed_slice(),
        })
    }

    pub fn map_id(&self) -> i32 {
        self.header.map_id
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrailSection {
    pub points: Box<[Point3]>,
    pub bounds: Box3,
}

impl TrailSection {
    pub const BOUNDS_EMPTY: Box3 = Box3::ZERO;
    #[cfg(todo)]
    pub const BOUNDS_EMPTY: Box3 = glamour::Box3 {
        min: point3!(f32::INFINITY, f32::INFINITY, f32::INFINITY),
        max: point3!(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    /// Zero-height trail bounds are common, but empty boxes are invalid...
    pub const BOUNDS_MIN_SIZE: Size3 = {
        // ~2^(1-13) to account for distances up to ~9k?
        let eps = 2e-4;
        Size3::new(eps, eps, eps)
    };

    pub fn with_points<P: Into<Box<[Point3]>>>(points: P) -> Self {
        let points = points.into();
        let bounds = match points.is_empty() {
            true => Self::BOUNDS_EMPTY,
            false => match Box3::from_points(points.iter().copied()) {
                mut bounds if bounds.is_empty() => {
                    let size = bounds.size();
                    bounds.max = bounds.min + size.max(Self::BOUNDS_MIN_SIZE).to_vector();
                    debug_assert!(!bounds.is_empty());
                    bounds
                },
                bounds => bounds,
            },
        };
        Self { bounds, points }
    }

    /// Read a section from a .trl file
    ///
    /// Ensure `points_buf` is empty prior to reading, unless you intend to prepend points to the section!
    pub fn read_from_trl<R: io::Read>(
        reader: &mut R,
        points_buf: &mut Vec<Point3>,
    ) -> io::Result<Option<Self>> {
        let mut next_point = match Self::read_point(reader) {
            Err(e) => return Err(e),
            Ok(None) => return Ok(None),
            Ok(Some(p)) => p,
        };
        while let Some(point) = next_point {
            points_buf.push(point);
            next_point = match Self::read_point(reader)? {
                #[cfg(todo)]
                None =>
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unterminated trail section",
                    )),
                None => {
                    log::trace!("unterminated trail section after {} points", points_buf.len());
                    None
                },
                Some(p) => p,
            };
        }

        let section = Self::with_points(&points_buf[..]);
        Ok(Some(section))
    }

    pub fn read_all_from_trl<R: io::Read>(
        reader: &mut R,
        out: &mut Vec<Self>,
        points_buf: &mut Vec<Point3>,
    ) -> io::Result<()> {
        points_buf.clear();
        while let Some(section) = TrailSection::read_from_trl(&mut *reader, points_buf)? {
            out.push(section);
            points_buf.clear();
        }

        Ok(())
    }

    pub const POINT_SIZE: usize = mem::size_of::<f32>() * 3;
    const POINT_EMPTY_DATA: [u8; Self::POINT_SIZE] = [0u8; Self::POINT_SIZE];
    pub fn read_point<R: Read>(reader: &mut R) -> io::Result<Option<Option<Point3<f32>>>> {
        let point_data = {
            let mut buf = Self::POINT_EMPTY_DATA;
            let rest = match reader.read(&mut buf) {
                Err(e) => return Err(e),
                Ok(0) => return Ok(None),
                Ok(amt) => buf.get_mut(amt..).expect("io::Read::read amount invalid"),
            };
            reader.read_exact(rest)?;
            buf
        };

        Ok(Some(match point_data {
            Self::POINT_EMPTY_DATA => None,
            [x0, x1, x2, x3, y0, y1, y2, y3, z0, z1, z2, z3] => Some(Point3::new(
                f32::from_le_bytes([x0, x1, x2, x3]),
                f32::from_le_bytes([y0, y1, y2, y3]),
                f32::from_le_bytes([z0, z1, z2, z3]),
            )),
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}
