use {
    crate::{
        controller::ControllerEvent,
        exports::runtime as rt,
        marker::atomic::MarkerInputData,
        settings::{pathing::SpaceSettings, PathingSettings, Settings},
        space::{
            dx11::{prelude::*, PerspectiveInputData, RenderBackend},
            pack::PackCollection,
            MapContext, MapTarget,
        },
        timer::{PhaseState, TimerFile, TimerMarker},
        Controller,
    },
    anyhow::Context,
    bevy_ecs::prelude::*,
    glam::{Vec3, Vec4},
    nexus::{imgui::Ui, rtapi::RealTimeApi},
    std::{
        collections::{HashMap, HashSet},
        num::NonZeroU32,
        sync::Arc,
    },
    tokio::{sync::mpsc::{Receiver, Sender}, time::Instant},
};
#[cfg(feature = "space-ecs")]
use {
    crate::{
        resources::ObjFile,
        space::object::{ObjectBacking, ObjectLoader},
        timer::RotationType,
    },
    std::path::PathBuf,
};
#[cfg(feature = "goggles")]
use crate::space::goggles;

#[derive(Component)]
struct Render {
    disabled: bool,
    #[cfg(feature = "space-ecs")]
    backing: Arc<ObjectBacking>,
    #[cfg(feature = "space-ecs")]
    rotation: RotationType,
}
#[cfg(feature = "space-ecs")]
#[derive(Component)]
struct Position(Vec3);

#[derive(Component)]
#[allow(unused)]
struct Marker {
    phase: Arc<PhaseState>,
    start: Instant,
    marker: TimerMarker,
}

#[derive(Bundle)]
struct MarkerBundle {
    #[cfg(feature = "space-ecs")]
    position: Position,
    render: Render,
}

pub enum SpaceEvent {
    MarkerFeed(PhaseState),
    MarkerReset(Arc<TimerFile>),
    PathingToggle,
    MapToggle(MapContext),
    DisabledPaths(HashSet<String>),
    PackLoad {
        pack: Arc<taimi_pack::Pack>,
        loader: super::pack::LoaderBox,
    },
    PackUnloadAll,
}

fn handle_marker_timings(mut commands: Commands, mut query: Query<(Entity, &Marker, &mut Render)>) {
    let now = Instant::now();
    for (entity, marker, mut render) in &mut query {
        if now > marker.marker.end(marker.start) {
            log::info!(
                "Entity {} reached end after {}, despawning.",
                entity,
                marker.marker.duration
            );
            commands.entity(entity).despawn();
        } else if now > marker.marker.start(marker.start) && render.disabled {
            log::info!(
                "Entity {} reached start at {}!",
                entity,
                marker.marker.timestamp
            );
            render.disabled = false;
        }
    }
}

pub struct Engine {
    receiver: Receiver<SpaceEvent>,
    pub render_backend: RenderBackend,
    #[cfg(feature = "space-ecs")]
    pub model_files: HashMap<PathBuf, ObjFile>,
    #[cfg(feature = "space-ecs")]
    pub object_kinds: HashMap<String, Arc<ObjectBacking>>,
    phase_states: Vec<Arc<PhaseState>>,
    associated_entities: HashMap<String, Vec<Entity>>,
    rtapi: Option<RealTimeApi>,
    pub gameplay_map: Result<NonZeroU32, u32>,

    schedule: Schedule,

    // ECS stuff
    pub world: World,

    pub packs: PackCollection,

    pub settings: Option<PathingSettings>,
}

impl Engine {
    pub fn initialise(ui: &Ui, receiver: Receiver<SpaceEvent>) -> anyhow::Result<Engine> {
        let render_backend = RenderBackend::setup(ui.io().display_size)
            .context("Failed to set up render backend")?;

        #[cfg(feature = "space-ecs")]
        let object_kinds = {
            let models_dir = crate::ADDON_DIR.join("models");
            let object_descs = ObjectLoader::load_desc(&models_dir)
                .context("Failed to load model descriptors")?;
            log::debug!("{:?}", object_descs);
            let model_files = ObjFile::load(&models_dir, &object_descs)
                .context("Failed to load model object")?;

            let object_kinds = object_descs.to_backings(
                &render_backend.device,
                &model_files,
                &render_backend.shaders.0,
                &render_backend.shaders.1,
            );

            object_kinds
        };

        let world = World::new();

        let mut schedule = Schedule::default();

        schedule.add_systems(handle_marker_timings);

        let packs = PackCollection::new(&render_backend)
            .context("Initializing packs")?;
        Controller::try_send(ControllerEvent::PathingLoadAll);

        let rtapi = rt::rtapi()
            .map_err(anyhow::Error::msg)
            .context("RTAPI unavailable");
        let rtapi = match rtapi.map_err(anyhow::Error::msg) {
            Ok(rtapi) => {
                match &rtapi {
                    Some(rtapi) if rtapi.is_active() =>
                        log::info!("Using RTAPI as perspective data source"),
                    _ =>
                        log::info!("RTAPI unavailable"),
                }
                rtapi
            },
            Err(e) => {
                // TODO: listen for events in case it gets loaded later or something
                log::debug!("{e:#}");
                None
            },
        };

        let engine = Engine {
            rtapi,
            gameplay_map: Err(0),
            #[cfg(feature = "space-ecs")]
            model_files,
            receiver,
            render_backend,
            #[cfg(feature = "space-ecs")]
            object_kinds,
            schedule,
            world,
            associated_entities: Default::default(),
            phase_states: Default::default(),
            packs,
            #[cfg(feature = "goggles")]
            settings: None,
        };

        #[cfg(feature = "space-ecs")]
        let mut engine = engine;
        #[cfg(feature = "space-ecs")]
        if let Some(backing) = engine.object_kinds.get("Cat") {
            engine.world.spawn((
                #[cfg(feature = "space-ecs")]
                Position(Vec3::new(0.0, 130.0, 0.0)),
                Render {
                    disabled: false,
                    backing: backing.clone(),
                    rotation: RotationType::Rotation(Vec3::ZERO),
                },
            ));
        }
        Ok(engine)
    }

    pub fn new_phase(&mut self, phase_state: PhaseState) -> anyhow::Result<()> {
        let phase_state = Arc::new(phase_state);
        let markers = &phase_state.markers;
        let entry = self
            .associated_entities
            .entry(phase_state.timer.name.clone())
            .or_default();
        for marker in markers {
            if let Some(_base_path) = &phase_state.timer.path {
                #[cfg(feature = "space-ecs")]
                let backing = Arc::new(ObjectBacking::create_marker(
                    &self.render_backend,
                    marker,
                    _base_path.clone(),
                ).context("marker object creation failed")?);
                let entity = self.world.spawn((
                    #[cfg(feature = "space-ecs")]
                    Position(marker.position),
                    Marker {
                        phase: phase_state.clone(),
                        start: phase_state.start,
                        marker: marker.clone(),
                    },
                    #[cfg(feature = "space-ecs")]
                    Render {
                        rotation: marker.kind.clone(),
                        disabled: true,
                        backing,
                    },
                ));
                let id = entity.id();
                log::debug!(
                    "Creating entity {id} at {} from timer {} markers, phase {}",
                    marker.position,
                    phase_state.timer.name(),
                    phase_state.phase.name
                );
                entry.push(id);
            }
        }
        self.phase_states.push(phase_state);
        Ok(())
    }
    pub fn remove_phase(&mut self, timer: Arc<TimerFile>) -> anyhow::Result<()> {
        if let Some(entry) = self.associated_entities.remove(&timer.name.clone()) {
            entry.iter().for_each(|entity| {
                log::debug!("Despawning {entity} from timer {} markers", timer.name());
                self.world.despawn(*entity);
            });
        }
        self.phase_states.retain(|p| !Arc::ptr_eq(&p.timer, &timer));
        Ok(())
    }
    #[allow(dead_code)]
    pub fn reset_phases(&mut self) {
        for entities in self.associated_entities.values() {
            for entity in entities {
                self.world.despawn(*entity);
            }
        }
        self.associated_entities.clear();
        self.phase_states.clear();
    }

    pub fn process_event(&mut self) -> anyhow::Result<()> {
        match self.receiver.try_recv() {
            Ok(event) => {
                use SpaceEvent::*;
                match event {
                    DisabledPaths(disabled_paths) => {
                        self.packs.disable_paths(disabled_paths);
                    }
                    PathingToggle => {
                        let res = self.map_settings_mut(|s| {
                            let visible = !s.space.visible_space();
                            s.space.visible_space = Some(visible);
                            visible
                        }).context("toggle paths");
                        #[cfg(feature = "goggles")]
                        match &res {
                            _ if !goggles::is_enabled() => (),
                            Ok(false) =>
                                goggles::clear_lens(),
                            Ok(true) =>
                                goggles::pick_lens(),
                            _ => (),
                        }
                        if let Err(e) = res {
                            log::warn!("{e:#}");
                        }
                    },
                    MapToggle(cx) => {
                        if let Err(e) = self.map_settings_mut(|s| match cx {
                            MapContext::Minimap =>
                                s.space.visible_map_mini = Some(!s.space.visible_minimap()),
                            MapContext::Global =>
                                s.space.visible_map_world = Some(!s.space.visible_worldmap()),
                        }).context("toggle map paths") {
                            log::warn!("{e:#}");
                        }
                    },
                    PackLoad { pack, loader } => {
                        let pack_idx = self.packs.add_pack(pack, loader);
                        if let Err(e) = self.packs.load_pack(&self.render_backend.device, pack_idx) {
                            log::error!("{e:#}");
                        }
                    },
                    PackUnloadAll => {
                        log::info!("Unloading all paths...");
                        self.packs.clear();
                    },
                    MarkerFeed(phase_state) => self.new_phase(phase_state)
                        .context("marker new phase")?,
                    MarkerReset(timer) => self.remove_phase(timer)
                        .context("marker remove phase")?,
                }
            }
            Err(_error) => (),
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn check_phase_ends() {
        todo!("this is supposed to terminate a phase when there are no more markers, ideally we should actually make something that finds the latest timestamp between sounds, directions, markers, alerts etc");
    }

    pub fn render(&mut self, ui: &Ui) -> anyhow::Result<()> {
        let display_size = ui.io().display_size;
        self.process_event()
            .context("render engine event processing failure")?;
        self.schedule.run(&mut self.world);

        let mut pdata = PerspectiveInputData::cloned();
        if let Some(rtapi) = &self.rtapi {
            use nexus::rtapi::GameState;

            let mut dirty = true;
            if let Some(player) = rtapi.read_player() {
                let playerfront = Vec3::from_array(player.character_facing);
                if playerfront != Vec3::ZERO {
                    pdata.playpos = Vec3::from_array(player.character_position);
                    dirty = true;
                }
            }
            if let Some(camera) = rtapi.read_camera() {
                let front = Vec3::from_array(camera.camera_facing);
                if front != Vec3::ZERO {
                    pdata.front = front;
                    pdata.pos = Vec3::from_array(camera.camera_position);
                }
                if camera.camera_fov != 0.0f32 {
                    pdata.fov = camera.camera_fov;
                }
                dirty = true;
            }
            let ingame = rtapi.read_game().and_then(|game| match game.game_state {
                Ok(GameState::Gameplay) =>
                    Some(true),
                Ok(GameState::LoadingScreen | GameState::CharacterSelection | GameState::CharacterCreation | GameState::Cinematic) =>
                    Some(false),
                Err(_) => None,
            });
            if let Some(ingame) = ingame {
                pdata.is_gameplay = Some(ingame);
                dirty = true;
            }
            if dirty {
                pdata.clone().commit();
            }
        }

        self.render_backend.prepare(&display_size);
        let device_context =
            unsafe { self.render_backend.device.GetImmediateContext() }
            .context("I lost my context!")?;

        let map_data = MarkerInputData::read();

        let map_id = map_data.as_ref().map(|mid| NonZeroU32::new(mid.map_id));
        let map_res = match (self.gameplay_map, pdata.is_gameplay, map_id) {
            (Ok(map_id), Some(true), Some(Some(new_map_id))) => {
                if new_map_id != map_id {
                    log::info!("map changed from {map_id} to {new_map_id} without a loading screen?");
                    self.gameplay_map_enter(&device_context, new_map_id)
                } else { Ok(()) }
            },
            (Ok(prev), Some(false), _) => {
                log::debug!("leaving map {prev}");
                self.gameplay_map_exit(&device_context, prev)
            },
            (Err(_prev), Some(false), Some(None)) => {
                log::debug!("forgetting about previous map {_prev}");
                self.gameplay_map = Err(0);
                Ok(())
            },
            (Err(prev), Some(true), Some(Some(map_id))) => {
                log::debug!("{}entering map {map_id}", if prev == map_id.get() { "re-" } else { "" });
                self.gameplay_map_enter(&device_context, map_id)
            },
            (Err(prev), Some(false), Some(Some(map_id))) if map_id.get() != prev => {
                log::trace!("map changed to {map_id} but not in game yet...");
                Ok(())
            },
            // waiting for map info...
            (_, None, _) | (_, _, None) | (_, Some(true), Some(None)) => Ok(()),
            _ => Ok(()),
        };
        if let Err(e) = map_res {
            log::error!("Map error: {e:#}");
        }

        self.packs.update();

        let map_ctx = map_data.as_ref().map(|mid| mid.perspective);
        let (visible_space, visible_map) = self.map_settings(|s| (
            (
                s.space.visible_space().then_some(s.space.distance_max()),
                map_ctx.map(|ctx| s.space.visible_map(ctx)),
            )
        ));
        let render_map = match visible_map.unwrap_or(false) && pdata.is_gameplay.unwrap_or(false) {
            true => map_data.as_ref().map(|data| MapTarget::new(data)),
            _ => None,
        };
        let render_world = match visible_space {
            Some(distance_max) if pdata.world_visible() =>
                Some(self.packs.update_for_draw(&pdata, distance_max, &mut self.render_backend)),
            _ => None,
        };

        let perspective_slot = 0;
        self.render_backend.blending_handler.set(&device_context);

        let minimap_bounds = match &render_map {
            Some(map) if matches!(map.perspective, MapContext::Minimap) => Some(map.bounds_screen),
            Some(..) => None,
            None => map_data.as_ref()
                .and_then(|map_data| match map_data.perspective {
                    MapContext::Minimap => Some({
                        use glamour::{Box2, TransformMap};

                        let bounds = map_data.fakespace_minimap_bound();
                        let trans = map_data.screen_to_fake().inverse();
                        Box2::new(
                            trans.map(bounds.min()),
                            trans.map(bounds.max()),
                        )
                    }),
                    _ => None,
                }),
        };

        if let Some(minimap_bounds) = &minimap_bounds {
            self.render_backend.depth_handler.setup_minimap_scissor(&device_context, minimap_bounds);
        }

        if let Some(map) = &render_map {
            let (
                trail_textured,
                trail_scale,
                trail_alpha,
                poi_scale,
                poi_alpha,
            ) = self.map_settings(|s| (
                s.space.trail_textured_map(map.perspective),
                s.space.trail_scale_map(map.perspective),
                s.space.trail_alpha_map(map.perspective),
                s.space.poi_scale_map(map.perspective),
                s.space.poi_alpha_map(map.perspective),
            ));
            {
                let vdata = &mut self.render_backend.perspective_handler.constant_buffer_mapv_data;
                vdata.expand = Self::scale_expand(trail_scale, trail_textured, poi_scale);
            }
            {
                // TODO: cpbuffer per type? just mixing them together for now...
                let alpha = trail_alpha * poi_alpha;
                let pdata = &mut self.render_backend.perspective_handler.constant_buffer_mapp_data;
                pdata.colour.w = alpha;
            }

            if trail_alpha > 0.0 || poi_alpha > 0.0 {
                let backend = &mut self.render_backend;
                backend.perspective_handler.update_map(map);

                backend.perspective_handler.update_map_cb(&device_context);

                backend.depth_handler.setup_map(&device_context, map);
                backend.perspective_handler.set_map_cb(&device_context, perspective_slot);

                let entities = self.packs.entities_map(map);
                PackCollection::draw_map_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, map, entities);
            }
        }

        let distance_max = visible_space
            .unwrap_or(SpaceSettings::DEFAULT_DISTANCE_MAX);
        self.render_backend.depth_handler.setup(&device_context, distance_max);

        if let Some(..) = &minimap_bounds {
            let backend = &mut self.render_backend;
            backend.depth_handler.setup_depth_write(&device_context, true);

            // TODO: reusing this shader is a hack
            backend.shaders.0["map"].set(&device_context);
            backend.depth_handler.fill_clipped(&device_context, &mut backend.perspective_handler);

            backend.depth_handler.setup_depth_write(&device_context, false);
            backend.depth_handler.clear_scissor(&device_context);
            // TODO: flush context state?
        }

        self.render_backend.perspective_handler.set_cb(&device_context, perspective_slot);

        #[cfg(feature = "space-ecs")]
        let mut query = self.world.query::<(&mut Render, &Position)>();
        #[cfg(feature = "space-ecs")]
        for (_k, c) in &query
            .iter(&self.world)
            .chunk_by(|(r, _p)| r.backing.name.clone())
        {
            let mut itery = c.into_iter();
            let slice = itery.next().ok_or(anyhow!("empty slice!"))?;
            let (r, p) = slice;
            if !r.disabled {
                let rot = match r.rotation {
                    RotationType::Billboard => {
                        let mark2d = (p.0.xz() - pdata.pos.xz()).to_angle();
                        let y = Mat4::from_rotation_y(-90.0f32.to_radians() - mark2d);
                        y
                        //Mat4::IDENTITY
                    }
                    _ => Mat4::IDENTITY,
                };
                let ibd: Vec<_> = vec![slice]
                    .into_iter()
                    .chain(itery)
                    .map(|(_r, p)| {
                        //  r.backing.render.metadata.model_matrix *
                        let affy = Mat4::from_translation(p.0)
                            * rot
                            * r.backing.render.metadata.model_matrix;
                        InstanceBufferData {
                            world: affy,
                            //world_position: affy.translation,
                            colour: Vec3::new(1.0, 1.0, 1.0),
                        }
                    })
                    .collect();
                r.backing
                    .set_and_draw(perspective_slot, &self.render_backend.device, &device_context, &ibd)?;
            }
        }

        if let Some(context) = &render_world {
            let (
                overlap_threshold,
                distance_intensity,
                trail_textured,
                trail_scale,
                trail_alpha,
                poi_scale,
                poi_alpha,
                _obscured_alpha,
            ) = self.map_settings(|s| (
                s.space.player_overlap_threshold(),
                s.space.distance_fade_intensity(),
                s.space.trail_textured_space(),
                s.space.trail_scale_space(),
                s.space.trail_alpha(),
                s.space.poi_scale_space(),
                s.space.poi_alpha(),
                match () {
                    #[cfg(feature = "goggles")]
                    _ => s.space.goggles.obscured_alpha(),
                    #[cfg(not(feature = "goggles"))]
                    _ => (),
                },
            ));
            // TODO: cpbuffer per type? just mixing them together for now...
            let alpha = trail_alpha * poi_alpha;
            let expand = Self::scale_expand(trail_scale, trail_textured, poi_scale);
            {
                let vdata = &mut self.render_backend.perspective_handler.constant_buffer_data;
                vdata.expand = expand;
            }
            {
                let pdata = &mut self.render_backend.perspective_handler.constant_buffer_pixel_data;
                pdata.set_overlap_threshold(overlap_threshold);
                pdata.set_intensity(distance_intensity);
            }

            self.render_backend.perspective_handler.update_perspective(Vec3::splat(expand.y + 1.0));

            #[cfg(feature = "goggles")]
            if goggles::is_enabled() && _obscured_alpha > 0.0 {

                // first pass at reduced opacity
                let backend = &mut self.render_backend;
                backend.perspective_handler.set_alpha(_obscured_alpha);
                backend.perspective_handler.update_cb(&device_context);
                backend.depth_handler.set_state_obscured(&device_context, true);

                let entities = self.packs.entities_obscured(context);
                PackCollection::draw_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, entities);

                backend.depth_handler.set_state_obscured(&device_context, false);
            }

            if trail_alpha > 0.0 || poi_alpha > 0.0 {
                self.render_backend.perspective_handler.set_alpha(alpha);
                self.render_backend.perspective_handler.update_cb(&device_context);

                self.packs.draw(&pdata, context, &self.render_backend, &device_context);
            }
        }
        Ok(())
    }

    pub fn sender() -> Option<Sender<SpaceEvent>> {
        crate::SPACE_SENDER.try_read()
            .as_ref().ok()
            .and_then(|s| (*s).clone())
    }

    pub fn is_available() -> bool {
        match crate::SPACE_SENDER.try_read() {
            Ok(sender) => sender.is_some(),
            _ => false,
        }
    }

    pub fn try_send(e: SpaceEvent) {
        let sender = crate::SPACE_SENDER.try_read();
        let sender = sender.as_ref().map(|s| &**s);
        if let Ok(Some(sender)) = sender {
            let _ = sender.try_send(e);
        }
    }

    pub fn cleanup(&mut self) {
        #[cfg(debug_assertions)] {
            log::warn!("TODO: Please clean up the engine when the program quits");
        }
    }

    pub fn gameplay_map_exit(&mut self, device_context: &ID3D11DeviceContext, prev_map_id: NonZeroU32) -> anyhow::Result<()> {
        #[cfg(feature = "goggles")]
        if goggles::is_enabled() {
            goggles::clear_lens();
        }

        let res = self.packs.unload_map(device_context, prev_map_id.get());

        self.gameplay_map = Err(prev_map_id.get());

        res
    }

    pub fn gameplay_map_enter(&mut self, device_context: &ID3D11DeviceContext, map_id: NonZeroU32) -> anyhow::Result<()> {
        #[cfg(feature = "goggles")]
        {
            use crate::space;

            goggles::pick_lens();

            let depth = self.map_settings_ref(|s| s.map(|s|
                s.space.goggles.map_depth_calibration(map_id.get())
            ));
            if let Some((min, max)) = depth {
                space::set_min_depth(space::MIN_DEPTH * min);
                space::set_max_depth(space::MAX_DEPTH * max);
            }
        }

        let res = self.packs.load_map(&self.render_backend.device, device_context, map_id.get());

        self.gameplay_map = Ok(map_id);

        res
    }

    pub fn map_settings_ref<R, F: FnOnce(Option<&PathingSettings>) -> R>(&self, f: F) -> R {
        match self.settings.as_ref() {
            Some(s) => f(Some(s)),
            None => {
                let mut f = Some(f);
                let res = Settings::read_with_blocking(|s| {
                    match f.take() {
                        Some(f) => f(Some(&s.pathing())),
                        None => unreachable!(),
                    }
                }).context("map settings unavailable");
                if let Err(e) = &res {
                    log::warn!("{e:#}");
                }
                match (f, res) {
                    (Some(f), _) => {
                        f(None)
                    },
                    (None, Ok(res)) => res,
                    (None, Err(..)) => unreachable!(),
                }
            },
        }
    }

    pub fn map_settings<R, F: FnOnce(&PathingSettings) -> R>(&mut self, f: F) -> R {
        let mut fail = None;
        let s = self.settings.get_or_insert_with(|| {
            match Settings::read_with_blocking(|s| s.pathing.clone()) {
                Ok(Some(s)) => s,
                Ok(None) => {
                    fail = Some(None);
                    Default::default()
                },
                Err(e) => {
                    fail = Some(Some(e));
                    Default::default()
                },
            }
        });
        match fail {
            None => f(s),
            Some(e) => {
                if let Some(e) = e.as_ref() {
                    log::warn!("map settings unavailable: {e:#}");
                }
                let res = f(s);
                if e.is_some() {
                    let _ = self.settings.take();
                }
                res
            },
        }
    }

    pub fn map_settings_mut<R, F: FnOnce(&mut PathingSettings) -> R>(&mut self, f: F) -> anyhow::Result<R> {
        let mut fail = None;
        let s = self.settings.get_or_insert_with(|| {
            match Settings::read_with_blocking(|s| s.pathing.clone()) {
                Ok(Some(s)) => s,
                Ok(None) => {
                    fail = Some(None);
                    Default::default()
                },
                Err(e) => {
                    fail = Some(Some(e));
                    Default::default()
                },
            }
        });
        match fail {
            Some(Some(e)) => {
                Err(e)
            },
            _ => Ok({
                let res = f(s);

                if let Some(mut settings) = Settings::try_write() {
                    // TODO: copy over everything properly
                    settings.pathing_mut().space = s.space.clone();
                } else {
                    log::warn!("settings unavailable for saving");
                }
                res
            }),
        }
    }

    fn scale_factor(trail_scale: f32, poi_scale: f32) -> Vec3 {
        let trail_scale = (trail_scale - 1.0) / 2.0;
        let poi_scale = poi_scale - 1.0;
        Vec3::new(trail_scale, poi_scale, 0.0)
    }

    fn scale_expand(trail_scale: f32, trail_textured: bool, poi_scale: f32) -> Vec4 {
        let scale = Self::scale_factor(trail_scale, poi_scale);
        match trail_textured {
            true => {
                let scalex = scale.x * 1.5;
                let e = (2.22149f32, -0.388849f32);
                let scale_trail_norm = (e.1 * (scalex + 2.0)).exp() * e.0;
                let scale_trail_tex = scale_trail_norm.clamp(0.04, 0.99);
                scale.extend(scale_trail_tex)
            },
            false => {
                scale.with_z(0.39).extend(0.0)
            },
        }
    }
}

unsafe impl Send for Engine {}
