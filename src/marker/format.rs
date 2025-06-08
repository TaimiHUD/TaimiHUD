use {
    crate::{
        render::RenderState,
        timer::{BlishVec3, Polytope, Position},
        ADDON_DIR, SETTINGS,
    },
    anyhow::anyhow,
    arcdps::extras::Control,
    chrono::{DateTime, Utc},
    glam::Vec3,
    glob::Paths,
    nexus::{gamebind::GameBind, imgui::Ui},
    ordered_float::OrderedFloat,
    serde::{Deserialize, Serialize},
    serde_repr::{Deserialize_repr, Serialize_repr},
    std::{
        collections::HashMap,
        fs::exists,
        path::{Path, PathBuf},
        sync::Arc,
    },
    strum_macros::{Display, EnumIter, FromRepr},
    tokio::{
        fs::{create_dir_all, read_to_string, File, OpenOptions},
        io::AsyncWriteExt,
        sync::Semaphore,
        task::JoinSet,
    },
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum MarkerFormats {
    // community
    Community(MarkerFile),
    // integrated into the original
    Integrated(IntegratedMarkers),
    // mine
    Taimi(Vec<MarkerSet>),
}

#[derive(Debug, PartialEq, Eq, EnumIter, Display, Clone)]
pub enum MarkerFiletype {
    Community,
    Integrated,
    Taimi,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RuntimeMarkers {
    pub path: Option<PathBuf>,
    pub file: MarkerFormats,
}
impl RuntimeMarkers {
    pub fn glob() -> String {
        "**/*.markers".to_string()
    }

    pub fn path_glob(path: &Path) -> PathBuf {
        path.join(Self::glob())
    }

    pub fn get_paths(path: &Path) -> anyhow::Result<Paths> {
        let pathbuf_glob = Self::path_glob(path);

        let path_glob_str = pathbuf_glob
            .to_str()
            .ok_or_else(|| anyhow!("Timer file loading path glob unparseable for {path:?}"))?;
        Ok(glob::glob(path_glob_str)?)
    }

    /*
    pub fn append() -> anyhow::Result<()> {
    }*/

    pub async fn load_many(
        load_dir: &Path,
        simultaneous_limit: usize,
    ) -> anyhow::Result<Vec<Arc<Self>>> {
        log::debug!("Beginning load_many for {load_dir:?} with a simultaneous open limit of {simultaneous_limit}.");
        let mut marker_files = Vec::new();
        if exists(load_dir)? {
            let mut set = JoinSet::new();
            let semaphore = Arc::new(Semaphore::new(simultaneous_limit));
            let mut paths = Self::get_paths(load_dir)?;
            while let Some(path) = paths.next() {
                let permit = semaphore.clone().acquire_owned().await?;
                let path = path?.clone();
                set.spawn(async move {
                    let marker_file = Self::load(&path).await?;
                    drop(permit);
                    Ok::<Arc<Self>, anyhow::Error>(marker_file)
                });
            }
            let (mut join_errors, mut load_errors): (usize, usize) = (0, 0);
            while let Some(marker_file) = set.join_next().await {
                match marker_file {
                    Ok(res) => match res {
                        Ok(marker_file) => {
                            marker_files.push(marker_file);
                        }
                        Err(err) => {
                            load_errors += 1;
                            log::error!("marker load_many error for {load_dir:?}: {err}");
                        }
                    },
                    Err(err) => {
                        join_errors += 1;
                        log::error!("marker load_many join error for {load_dir:?}: {err}");
                    }
                }
            }
            log::debug!(
                "Finished load_many for {load_dir:?}: {} succeeded, {join_errors} join errors, {load_errors} other errors.",
                marker_files.len()
            );
        }
        Ok(marker_files)
    }
    pub async fn load_arcless(path: &PathBuf) -> anyhow::Result<Self> {
        log::debug!("Attempting to load the markers file at \"{path:?}\".");
        let mut file_data = read_to_string(path).await?;
        json_strip_comments::strip(&mut file_data)?;
        let format: MarkerFormats = serde_json::from_str(&file_data)?;
        let data = Self {
            file: format,
            path: Some(path.to_path_buf()),
        };
        log::debug!("Successfully loaded the markers file at \"{path:?}\".");
        Ok(data)
    }

    pub async fn create_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        log::debug!("MarkerFormat: Saving to \"{}\".", path.display());
        let settings_str = serde_json::to_string(&self.file)?;
        let mut file = File::create(path).await?;
        file.write_all(settings_str.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        log::debug!("MarkerFormat: Saving to \"{}\".", path.display());
        let settings_str = serde_json::to_string(&self.file)?;
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .await?;
        file.write_all(settings_str.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn edit(
        ms: MarkerSet,
        path: &PathBuf,
        original_category: Option<String>,
        idx: usize,
    ) -> anyhow::Result<()> {
        let mut file = Self::load_arcless(path).await?;
        if ms.category != original_category {
            file.remove(path, original_category, idx).await?;
            file.append_raw(ms).await?;
        } else {
            let entry = file.get_entry(path, ms.category.clone(), idx).await?;
            *entry = ms;
        }
        file.save(path).await?;
        Ok(())
    }

    pub async fn delete(
        path: &PathBuf,
        category: Option<String>,
        idx: usize,
    ) -> anyhow::Result<()> {
        let mut file = Self::load_arcless(path).await?;
        file.remove(path, category, idx).await?;
        file.save(path).await?;
        Ok(())
    }

    pub async fn get_entry(
        &mut self,
        path: &PathBuf,
        category: Option<String>,
        idx: usize,
    ) -> anyhow::Result<&mut MarkerSet> {
        match &mut self.file {
            MarkerFormats::Community(f) => {
                let category =
                    category.ok_or(anyhow!("Category not provided, required for File format"))?;
                let category = f
                    .categories
                    .iter_mut()
                    .find(|c| c.name == category)
                    .ok_or(anyhow!("Couldn't find category {}", category))?;
                Ok(&mut category.marker_sets[idx])
            }
            MarkerFormats::Taimi(t) => Ok(&mut t[idx]),
            MarkerFormats::Integrated(c) => Ok(&mut c.squad_marker_preset[idx]),
        }
    }

    pub async fn remove(
        &mut self,
        path: &PathBuf,
        category: Option<String>,
        idx: usize,
    ) -> anyhow::Result<MarkerSet> {
        match &mut self.file {
            MarkerFormats::Community(f) => {
                let category =
                    category.ok_or(anyhow!("Category not provided, required for File format"))?;
                let category = f
                    .categories
                    .iter_mut()
                    .find(|c| c.name == category)
                    .ok_or(anyhow!("Couldn't find category {}", category))?;
                Ok(category.marker_sets.remove(idx))
            }
            MarkerFormats::Taimi(t) => Ok(t.remove(idx)),
            MarkerFormats::Integrated(c) => Ok(c.squad_marker_preset.remove(idx)),
        }
    }

    pub async fn load(path: &PathBuf) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self::load_arcless(path).await?))
    }

    pub async fn append_raw(&mut self, ms: MarkerSet) -> anyhow::Result<()> {
        match &mut self.file {
            MarkerFormats::Community(f) => {
                if let Some(mscat) = &ms.category {
                    let category = match f.categories.iter_mut().find(|c| &c.name == mscat) {
                        Some(c) => c,
                        None => {
                            f.categories.push(MarkerCategory {
                                name: mscat.clone(),
                                marker_sets: Vec::new(),
                            });
                            f.categories
                                .iter_mut()
                                .find(|c| &c.name == mscat)
                                .ok_or(anyhow!(
                                    "Can't find the category \"{}\" we should've made",
                                    mscat
                                ))?
                        }
                    };
                    category.marker_sets.push(ms);
                    f.last_edit = Utc::now();
                } else {
                    let category = match f.categories.iter_mut().find(|c| &c.name == "No category")
                    {
                        Some(c) => c,
                        None => {
                            f.categories.push(MarkerCategory {
                                name: "No category".to_string(),
                                marker_sets: Vec::new(),
                            });
                            f.categories
                                .iter_mut()
                                .find(|c| &c.name == "No category")
                                .ok_or(anyhow!(
                                    "Can't find the category \"No category\" we should've made"
                                ))?
                        }
                    };
                    category.marker_sets.push(ms);
                    f.last_edit = Utc::now();
                }
            }
            MarkerFormats::Taimi(t) => {
                t.push(ms);
            }
            MarkerFormats::Integrated(c) => {
                c.squad_marker_preset.push(ms);
            }
        }
        Ok(())
    }

    pub async fn append(path: &PathBuf, ms: MarkerSet) -> anyhow::Result<()> {
        let mut file = Self::load_arcless(path).await?;
        file.append_raw(ms).await?;
        file.save(path).await?;
        Ok(())
    }

    pub async fn create(
        path: &PathBuf,
        format: MarkerFiletype,
        ms: MarkerSet,
    ) -> anyhow::Result<()> {
        let markers_dir = ADDON_DIR.join("markers");
        if !exists(&markers_dir)? {
            create_dir_all(&markers_dir).await?;
        }
        let path = markers_dir.join(format!("{}.markers", path.display()));
        match format {
            MarkerFiletype::Community => {
                let file_data = MarkerFile {
                    last_edit: Utc::now(),
                    path: Some(path.clone()),
                    categories: vec![MarkerCategory {
                        name: ms.category.clone().unwrap_or("No category".to_string()),
                        marker_sets: vec![ms],
                    }],
                };
                let file = RuntimeMarkers {
                    path: Some(path.clone()),
                    file: MarkerFormats::Community(file_data),
                };
                file.create_file(&path).await?;
            }
            MarkerFiletype::Taimi => {
                let file_data = vec![ms];
                let file = RuntimeMarkers {
                    path: Some(path.clone()),
                    file: MarkerFormats::Taimi(file_data),
                };
                file.create_file(&path).await?;
            }
            MarkerFiletype::Integrated => {
                let file_data = IntegratedMarkers {
                    version: "2.0.0".to_string(),
                    path: Some(path.clone()),
                    squad_marker_preset: vec![ms],
                };
                let file = RuntimeMarkers {
                    path: Some(path.clone()),
                    file: MarkerFormats::Integrated(file_data),
                };
                file.create_file(&path).await?;
            }
        }
        Ok(())
    }

    pub async fn markers(marker_packs: Vec<Arc<Self>>) -> HashMap<String, Vec<Arc<MarkerSet>>> {
        let mut finalized: HashMap<String, Vec<Arc<MarkerSet>>> = HashMap::new();
        for pack in marker_packs {
            match &pack.file {
                MarkerFormats::Community(f) => {
                    for category in &f.categories {
                        let category_name = category.name.clone();
                        let entry = finalized.entry(category_name.clone()).or_default();
                        for (i, marker_set) in category.marker_sets.iter().enumerate() {
                            let mut marker_set_data = marker_set.clone();
                            marker_set_data.category = Some(category_name.clone());
                            marker_set_data.path = pack.path.clone();
                            marker_set_data.idx = Some(i);
                            let marker_set_arc = Arc::new(marker_set_data);
                            entry.push(marker_set_arc);
                        }
                    }
                }
                MarkerFormats::Taimi(t) => {
                    for (i, marker_set) in t.iter().enumerate() {
                        let category_name = marker_set
                            .category
                            .clone()
                            .unwrap_or("No category".to_string());
                        let entry = finalized.entry(category_name.clone()).or_default();
                        let mut marker_set_data = marker_set.clone();
                        marker_set_data.category = Some(category_name.clone());
                        marker_set_data.path = pack.path.clone();
                        marker_set_data.idx = Some(i);
                        let marker_set_arc = Arc::new(marker_set_data);
                        entry.push(marker_set_arc);
                    }
                }
                MarkerFormats::Integrated(c) => {
                    for (i, marker_set) in c.squad_marker_preset.iter().enumerate() {
                        // extend their format by allowing the Category :)
                        let category_name = marker_set
                            .category
                            .clone()
                            .unwrap_or("Integrated".to_string());
                        let entry = finalized.entry(category_name.clone()).or_default();
                        let mut marker_set_data = marker_set.clone();
                        marker_set_data.category = Some(category_name.clone());
                        marker_set_data.path = pack.path.clone();
                        marker_set_data.idx = Some(i);
                        let marker_set_arc = Arc::new(marker_set_data);
                        entry.push(marker_set_arc);
                    }
                }
            }
        }
        finalized
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarkerFile {
    pub last_edit: DateTime<Utc>,
    #[serde(skip, default)]
    pub path: Option<PathBuf>,
    pub categories: Vec<MarkerCategory>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IntegratedMarkers {
    pub version: String,
    #[serde(skip, default)]
    pub path: Option<PathBuf>,
    pub squad_marker_preset: Vec<MarkerSet>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarkerCategory {
    #[serde(alias = "categoryName")]
    pub name: String,
    pub marker_sets: Vec<MarkerSet>,
}

fn default_true() -> bool {
    true
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarkerSet {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub category: Option<String>,
    pub author: Option<String>,
    pub name: String,
    pub description: String,
    pub map_id: u32,
    pub trigger: MarkerPosition,
    pub markers: Vec<MarkerEntry>,
    #[serde(default, skip)]
    pub path: Option<PathBuf>,
    #[serde(default, skip)]
    pub idx: Option<usize>,
}

impl MarkerSet {
    pub fn id(&self) -> String {
        let mut pieces = Vec::new();
        if let Some(category) = &self.category {
            pieces.push(category.clone());
        }
        if let Some(author) = &self.author {
            pieces.push(author.clone());
        }
        if !self.description.is_empty() {
            pieces.push(self.description.clone());
        }
        if !self.name.is_empty() {
            pieces.push(self.name.clone());
        }
        pieces.join("/")
    }

    pub fn status(&self) -> bool {
        let settings = SETTINGS.get().unwrap();
        if let Ok(settings_lock) = settings.try_read() {
            let result = if let Some(marker) = settings_lock.markers.get(&self.id()) {
                !marker.disabled
            } else {
                self.enabled
            };
            result
        } else {
            self.enabled
        }
    }

    pub fn trigger(&self, pos: Vec3) -> bool {
        let trig_vec: Vec3 = self.trigger.clone().into();
        trig_vec.distance(pos) <= 15.0
    }
    pub fn combined(&self) -> String {
        if let Some(author) = &self.author {
            format!("{}\nAuthor: {}", self.name.clone(), author.clone())
        } else {
            format!("{}\nUnknown Author", self.name.clone())
        }
    }
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct MarkerEntry {
    #[serde(alias = "i")]
    pub marker: MarkerType,
    #[serde(alias = "d", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(flatten)]
    pub position: MarkerPosition,
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct MarkerPosition {
    pub x: OrderedFloat<f32>,
    pub y: OrderedFloat<f32>,
    pub z: OrderedFloat<f32>,
}

impl From<MarkerPosition> for Vec3 {
    // it's pre-swizzled
    fn from(local: MarkerPosition) -> Self {
        Self::new(*local.x, *local.z, *local.y)
    }
}

impl From<Vec3> for MarkerPosition {
    // it's pre-swizzled
    fn from(local: Vec3) -> Self {
        Self {
            x: OrderedFloat(local.x),
            y: OrderedFloat(local.z),
            z: OrderedFloat(local.y),
        }
    }
}

impl From<MarkerPosition> for BlishVec3 {
    fn from(local: MarkerPosition) -> Self {
        Self::from_vec3(local.into())
    }
}

impl From<MarkerPosition> for Position {
    fn from(local: MarkerPosition) -> Self {
        let local_vec3: Vec3 = local.into();
        Self::from(local_vec3)
    }
}

impl From<MarkerPosition> for Polytope {
    fn from(local: MarkerPosition) -> Self {
        Polytope::NSphere {
            center: local.into(),
            radius: 15.0,
        }
    }
}

#[derive(
    Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr, FromRepr, Display, Debug, Copy, Clone,
)]
#[repr(u8)]
pub enum MarkerType {
    // Schema reference: https://github.com/manlaan/BlishHud-CommanderMarkers/blob/bhud-static/Manlaan.CommanderMarkers/README.md?plain=1#L69-L78
    // According to the schema, 0 and 9 are both Clear Markers.
    // Code reference: https://github.com/manlaan/BlishHud-CommanderMarkers/blob/7c3b2081596f7b8746e5e57d65213711aafa938c/Library/Enums/SquadMarker.cs#L6-L28
    // According to their code, 0 is None and 9 is Clear Markers.
    // This is why I have trust issues, man.
    Blank = 0,
    Arrow = 1,
    Circle = 2,
    Heart = 3,
    Square = 4,
    Star = 5,
    Spiral = 6,
    Triangle = 7,
    Cross = 8,
    ClearMarkers = 9,
}

impl MarkerType {
    pub fn iter_real_values() -> impl Iterator<Item = Self> {
        (1..9).flat_map(|i| Self::from_repr(i))
    }

    pub fn icon(&self, ui: &Ui) {
        RenderState::marker_icon(ui, Some(32.0), &self);
    }

    pub fn to_place_world_gamebind(&self) -> GameBind {
        match self {
            Self::Blank => panic!("i can't believe you've done this"),
            Self::Arrow => GameBind::SquadMarkerPlaceWorldArrow,
            Self::Circle => GameBind::SquadMarkerPlaceWorldCircle,
            Self::Heart => GameBind::SquadMarkerPlaceWorldHeart,
            Self::Square => GameBind::SquadMarkerPlaceWorldSquare,
            Self::Star => GameBind::SquadMarkerPlaceWorldStar,
            // what in the fuck my dudes
            Self::Spiral => GameBind::SquadMarkerPlaceWorldSwirl,
            Self::Triangle => GameBind::SquadMarkerPlaceWorldTriangle,
            Self::Cross => GameBind::SquadMarkerPlaceWorldCross,
            Self::ClearMarkers => GameBind::SquadMarkerClearAllWorld,
        }
    }

    #[allow(dead_code)]
    pub fn to_set_agent_gamebind(&self) -> GameBind {
        match self {
            Self::Blank => panic!("i can't believe you've done this"),
            Self::Arrow => GameBind::SquadMarkerSetAgentArrow,
            Self::Circle => GameBind::SquadMarkerSetAgentCircle,
            Self::Heart => GameBind::SquadMarkerSetAgentHeart,
            Self::Square => GameBind::SquadMarkerSetAgentSquare,
            Self::Star => GameBind::SquadMarkerSetAgentStar,
            // what in the fuck my dudes
            Self::Spiral => GameBind::SquadMarkerSetAgentSwirl,
            Self::Triangle => GameBind::SquadMarkerSetAgentTriangle,
            Self::Cross => GameBind::SquadMarkerSetAgentCross,
            Self::ClearMarkers => GameBind::SquadMarkerClearAllWorld,
        }
    }

    pub const fn control_location(&self) -> Control {
        match self {
            Self::Blank => panic!("i can't believe you've done this"),
            Self::Arrow => Control::Squad_Location_Arrow,
            Self::Circle => Control::Squad_Location_Circle,
            Self::Heart => Control::Squad_Location_Heart,
            Self::Square => Control::Squad_Location_Square,
            Self::Star => Control::Squad_Location_Star,
            Self::Spiral => Control::Squad_Location_Spiral,
            Self::Triangle => Control::Squad_Location_Triangle,
            Self::Cross => Control::Squad_Location_X,
            Self::ClearMarkers => Control::Squad_ClearAllLocationMarkers,
        }
    }

    #[allow(dead_code)]
    pub const fn control_object(&self) -> Control {
        match self {
            Self::Blank => panic!("i can't believe you've done this"),
            Self::Arrow => Control::Squad_Object_Arrow,
            Self::Circle => Control::Squad_Object_Circle,
            Self::Heart => Control::Squad_Object_Heart,
            Self::Square => Control::Squad_Object_Square,
            Self::Star => Control::Squad_Object_Star,
            Self::Spiral => Control::Squad_Object_Spiral,
            Self::Triangle => Control::Squad_Object_Triangle,
            Self::Cross => Control::Squad_Object_X,
            Self::ClearMarkers => Control::Squad_ClearAllObjectMarkers,
        }
    }
}
