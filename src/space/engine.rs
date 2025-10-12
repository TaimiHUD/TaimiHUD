use {
    crate::{
        controller::ControllerEvent,
        render::machine::RenderMachine,
        settings::{pathing::SpaceSettings, PathingSettings, Settings},
        space::{
            dx11::RenderBackend,
            pack::PackCollection,
            render_list::MapFrustum,
        },
        timer::{PhaseState, TimerFile, TimerMarker},
        Controller,
    },
    anyhow::{anyhow, Context},
    bevy_ecs::prelude::*,
    glam::{Vec3, Vec4},
    glamour::Size2,
    std::{
        collections::{HashMap, HashSet},
        num::NonZeroU32,
        sync::Arc,
    },
    taimi_d3d::dx11::prelude::*,
    taimi_meta::{
        coords::ScreenSpace,
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapContext,
        },
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

#[derive(strum::IntoStaticStr)]
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
    GameplayStatus {
        gameplay: GameplayState,
        trans: GameplayTransition,
    },
    UiResize(Option<Size2<ScreenSpace>>),
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

    #[cfg(feature = "goggles")]
    goggles_select_lens_delay: Option<(u32, bool)>,

    schedule: Schedule,

    // ECS stuff
    pub world: World,

    pub packs: PackCollection,

    settings: Option<PathingSettings>,
    settings_dirty: bool,
}

impl Engine {
    pub fn initialise(machine: &RenderMachine, receiver: Receiver<SpaceEvent>) -> anyhow::Result<Engine> {
        let display_size = machine.display_size()
            .ok_or_else(|| anyhow!("display size unknown"))?;

        let render_backend = RenderBackend::setup(display_size)
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
                &render_backend.shaders,
            );

            object_kinds
        };

        let world = World::new();

        let mut schedule = Schedule::default();

        schedule.add_systems(handle_marker_timings);

        let packs = PackCollection::new(&render_backend)
            .context("Initializing packs")?;
        Controller::try_send(ControllerEvent::PathingLoadAll);

        let engine = Engine {
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
            goggles_select_lens_delay: Some((Self::GOGGLES_START_DELAY_TICKS, true)),
            settings: None,
            settings_dirty: false,
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

    pub fn init_mut<F>(machine: &mut RenderMachine, f: F) -> anyhow::Result<()> where
        F: FnOnce(&mut Self, &mut RenderMachine) -> anyhow::Result<()>,
    {
        let enabled = Settings::try_read()
            .map(|s| s.enable_katrender);

        let mut engine = match crate::ENGINE.try_lock() {
            Ok(e) if e.is_none() && (machine.gameplay.is_initial() || !enabled.unwrap_or(false)) => {
                // if early game loading or charsel, delay init
                // TODO: make this an option, but have fallback plan if you cause crashes...
                return Ok(())
            },
            Ok(e) => e,
            _ => return Ok(()),
        };
        let mut res = None;
        let engine = engine.get_or_insert_with(|| {
            log::debug!("setting up space engine...");
            let (tx, rx) = tokio::sync::mpsc::channel::<SpaceEvent>(64);
            // TODO: remove this...
            let _ = tx.try_send(SpaceEvent::GameplayStatus {
                gameplay: machine.gameplay,
                trans: machine.gameplay.latest_transition(),
            });
            match crate::SPACE_SENDER.write().map_err(|_| anyhow!("space sender poisoned?")) {
                Ok(mut sender) =>
                    *sender = Some(tx),
                Err(e) => {
                    res = Some(e);
                    return Err(())
                },
            }
            let res = Self::initialise(machine, rx)
                .context("Space engine setup failed")
                .map_err(|e| {
                    res = Some(e);
                    ()
                });
            #[cfg(feature = "extension-nexus")]
            if res.is_ok() {
                machine.rtapi_setup();
            }
            #[cfg(feature = "goggles")]
            if let Ok(e) = &res {
                goggles::classify_space_lens(e);
            }
            res
        });
        if let Some(e) = res {
            return Err(e)
        }
        match engine {
            Ok(..) if !enabled.unwrap_or(true) => Ok(()),
            Ok(e) => f(e, machine),
            Err(..) => Ok(())
        }
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

    pub fn stop(&mut self) {
        self.packs.clear_active();
        self.packs.clear();
    }

    pub fn process_event(&mut self) -> anyhow::Result<bool> {
        let ev = self.receiver.try_recv();
        if let Ok(ev) = &ev {
            log::trace!("recv SpaceEvent::{}", <&str>::from(ev));
        }
        match ev {
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
                                self.goggles_enter(false),
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
                    GameplayStatus { gameplay, trans } => {
                        let device_context =
                            unsafe { self.render_backend.device.GetImmediateContext() }
                            .context("I lost my context!");
                        match gameplay {
                            GameplayState::Gameplay { map_id: Some(new_map_id) } => {
                                let prev = match trans {
                                    GameplayTransition::Map { prev_map_id, .. } => prev_map_id,
                                    GameplayTransition::Loaded { prev_map_id, .. } => prev_map_id,
                                    GameplayTransition::Intermission { prev_map_id } => prev_map_id,
                                };
                                log::debug!("{}entering map {new_map_id}", if prev == Some(new_map_id) { "re-" } else { "" });
                                self.gameplay_map_enter(&device_context?, new_map_id)
                            },
                            GameplayState::Gameplay { map_id: None } => {
                                log::info!("how do we know we loaded into a null map from {trans:?} to {gameplay:?}?");
                                Ok(())
                            },
                            GameplayState::Intermission { prev_map_id: Some(prev), next_map_id: None, initial: false } => {
                                log::debug!("leaving map {prev}");
                                self.goggles_exit();
                                //self.gameplay_map_exit(&device_context?, prev)
                                Ok(())
                            },
                            GameplayState::Intermission { prev_map_id: Some(prev), next_map_id: Some(next), initial: false } => {
                                if prev != next {
                                    log::info!("forget about previous map {prev}, prepare for {next}!");
                                    self.gameplay_map_exit(&device_context?, prev)
                                } else {
                                    Ok(())
                                }
                            },
                            _ => Ok(()),
                        }.with_context(|| format!("Map load error from {trans:?} to {gameplay:?}"))?;
                    },
                    UiResize(display_size) => match display_size {
                        None => {
                        },
                        Some(sz) => {
                            log::debug!("TODO: resize event to {sz:?}");
                        },
                    },
                    MarkerFeed(phase_state) => self.new_phase(phase_state)
                        .context("marker new phase")?,
                    MarkerReset(timer) => self.remove_phase(timer)
                        .context("marker remove phase")?,
                }
                Ok(true)
            }
            Err(_error) => Ok(false),
        }
    }

    #[allow(dead_code)]
    pub fn check_phase_ends() {
        todo!("this is supposed to terminate a phase when there are no more markers, ideally we should actually make something that finds the latest timestamp between sounds, directions, markers, alerts etc");
    }

    pub fn render(&mut self, machine: &mut RenderMachine) -> anyhow::Result<()> {
        let display_size = machine.display_size()
            .ok_or_else(|| anyhow!("display size unknown"))?;

        let map_ctx = machine.is_map_visible();
        let map_id = machine.is_ingame();
        let (
            visible_space,
            visible_map,
            camera_source,
            edge_feather_scale,
            trail_y_offset, trail_resolution, trail_width,
            (edge_scale, _obscured_alpha),
        ) = self.map_settings(|s| (
            (
                map_id.and_then(|_| s.space.visible_space().then_some(s.space.distance_max())),
                map_ctx.map(|ctx| s.space.visible_map(ctx)),
                s.space.camera_source(),
                s.space.edge_feather_scale(),
                s.space.trail_y_offset(), s.space.trail_resolution(), s.space.trail_width(),
                match () {
                    #[cfg(feature = "goggles")]
                    _ => (s.space.goggles.edge_scale(), s.space.goggles.obscured_alpha()),
                    #[cfg(not(feature = "goggles"))]
                    _ => (None::<f32>, ()),
                },
            )
        ));
        for _ in 0..5 {
            // try to get a couple events out of the way at a time
            // (would be nice to batch pack loads)
            let processed = self.process_event()
                .context("render engine event processing failure")?;
            if !processed {
                break
            }
        }
        self.schedule.run(&mut self.world);

        let device_context =
            unsafe { self.render_backend.device.GetImmediateContext() }
            .context("I lost my context!")?;

        if map_id.is_none() {
            return Ok(())
        }

        self.packs.trail_params.y_offset = trail_y_offset.unwrap_or(0.0);
        self.packs.trail_params.resolution = Some(trail_resolution);
        self.packs.trail_params.width = trail_width;

        self.packs.prepare(&self.render_backend.device, machine)?;
        self.packs.update();

        match (edge_scale, self.render_backend.depth_handler.fill_edge.is_none()) {
            (None, false) => {
                let _ = self.render_backend.depth_handler.fill_edge.take();
            },
            (Some(edge_scale), true) => {
                self.render_backend.depth_handler.regen_edge(&self.render_backend.device, Some((edge_scale, &machine.map.calibration)));
            },
            _ => (),
        }

        let render_map = match visible_map {
            Some(true) => map_ctx.map(|ctx| (ctx, super::dx11::PerspectiveHandler::map_local_bounds(machine))),
            _ => None,
        };
        let render_world = match visible_space {
            None => None,
            Some(..) if machine.get_map_open_state().is_visible() =>
                None,
            Some(distance_max) => {
                let depth = machine.get_depth_range()
                    .unwrap_or(RenderMachine::DEFAULT_DEPTH_RANGE);
                let camera = machine.get_camera(camera_source);
                let cull = MapFrustum::from_camera_data(
                    camera,
                    // TODO: machine.get_aspect_ratio(),
                    depth.start..depth.end.min(distance_max),
                );
                Some((camera, depth, cull))
            },
        };

        let perspective_slot = 0;
        self.render_backend.blend_state.set(&device_context);

        let minimap_bounds = match &render_map {
            Some((map_ctx, ..)) if matches!(map_ctx, MapContext::Global) => None,
            _ => Some({
                use glamour::{Box2, TransformMap};

                let bounds = machine.map.calibration.compass_bounds();
                Box2::from(machine.map.calibration.map(bounds))
            }),
        };

        let distance_max = visible_space
            .unwrap_or(SpaceSettings::DEFAULT_DISTANCE_MAX);
        self.render_backend.depth_handler.setup(&device_context, machine, distance_max);

        if let Some(minimap_bounds) = &minimap_bounds {
            self.render_backend.depth_handler.setup_minimap_scissor(&device_context, minimap_bounds);
        }

        if let Some((map_ctx, local_bounds)) = render_map {
            let (
                fwoom,
                trail_textured,
                trail_scale,
                trail_alpha,
                poi_scale,
                poi_alpha,
            ) = self.map_settings(|s| (
                s.space.map_open(),
                s.space.trail_textured_map(map_ctx),
                s.space.trail_scale_map(map_ctx),
                s.space.trail_alpha_map(map_ctx),
                s.space.poi_scale_map(map_ctx),
                s.space.poi_alpha_map(map_ctx),
            ));
            {
                let vdata = &mut self.render_backend.perspective_handler.constant_buffer_mapv_data;
                vdata.expand = Self::scale_expand(trail_scale, trail_textured, poi_scale);
            }
            {
                // TODO: cpbuffer per type? just mixing them together for now...
                let alpha = trail_alpha * poi_alpha;
                let pdata = &mut self.render_backend.perspective_handler.constant_buffer_mapp_data;
                let map_open = machine.map_open();
                pdata.colour.w = match map_open.progress_open().map(|p| p / 0.8) {
                    Some(p) if p < 1.0 => alpha * p * p * if fwoom { 1.0 } else { p },
                    _ => alpha,
                };
            }

            if trail_alpha > 0.0 || poi_alpha > 0.0 {
                let backend = &mut self.render_backend;
                backend.perspective_handler.update_map(machine, local_bounds, fwoom);

                backend.perspective_handler.update_map_cb(&device_context);

                backend.depth_handler.setup_map(&device_context);
                backend.perspective_handler.set_map_cb(&device_context, perspective_slot);

                let entities = self.packs.entities_map(local_bounds);
                PackCollection::draw_map_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, map_ctx, entities);
            }
        }

        #[cfg(feature = "goggles")]
        let goggles_enabled = goggles::is_enabled();
        #[cfg(feature = "goggles")]
        let goggles_2pass = goggles_enabled && _obscured_alpha > 0.0;

        let masking = minimap_bounds.is_some() || edge_scale.is_some();
        let masking = match render_world.is_some() && masking {
            #[cfg(feature = "goggles")]
            true if goggles_2pass => Some(true),
            true => Some(false),
            _ => None,
        };
        if let Some(depth_fill) = masking {
            let backend = &mut self.render_backend;
            backend.depth_handler.setup_depth_write(&device_context, Some(depth_fill));

            if let Some((shader, layout)) = backend.shaders.vertex.get("mask") {
                layout.set(&device_context);
                shader.set(&device_context);
            }
            backend.depth_handler.setup_fill(&device_context, &mut backend.perspective_handler);
        }

        if let Some(..) = &minimap_bounds {
            if masking.is_some() {
                self.render_backend.depth_handler.fill_clipped(&device_context);
            }
            self.render_backend.depth_handler.clear_scissor(&device_context);
        }

        if let Some(depth_fill) = masking {
            self.render_backend.depth_handler.fill_corners(&device_context, depth_fill);

            self.render_backend.depth_handler.setup_depth_write(&device_context, None);
        }

        self.render_backend.perspective_handler.set(&device_context, perspective_slot);

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

        if let Some((camera, ref _depth, ref cull)) = render_world {
            let (
                overlap_threshold,
                distance_intensity,
                trail_textured,
                trail_scale,
                trail_alpha,
                poi_scale,
                poi_alpha,
            ) = self.map_settings(|s| (
                s.space.player_overlap_threshold(),
                s.space.distance_fade_intensity(),
                s.space.trail_textured_space(),
                s.space.trail_scale_space(),
                s.space.trail_alpha(),
                s.space.poi_scale_space(),
                s.space.poi_alpha(),
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

            self.render_backend.perspective_handler.update_perspective(machine, camera, Vec3::splat(expand.y + 1.0));
            self.render_backend.perspective_handler.set_feather_scale(edge_feather_scale, display_size);

            #[cfg(feature = "goggles")]
            if goggles_2pass {
                // first pass at reduced opacity
                let backend = &mut self.render_backend;
                backend.perspective_handler.set_alpha(_obscured_alpha);
                backend.perspective_handler.update_cb(&device_context);
                backend.depth_handler.set_state_obscured(&device_context, true);

                let entities = self.packs.entities_obscured(cull);
                PackCollection::draw_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, entities);

                backend.depth_handler.set_state_obscured(&device_context, false);
            }

            if trail_alpha > 0.0 || poi_alpha > 0.0 {
                self.render_backend.perspective_handler.set_alpha(alpha);
                self.render_backend.perspective_handler.update_cb(&device_context);

                self.packs.draw(camera.clone(), cull, &self.render_backend, &device_context);
            }
        }

        self.render_backend.shaders.unset(&device_context);

        #[cfg(feature = "goggles")]
        if let Some(map_id) = map_id {
            let goggles_tick = self.goggles_select_lens_delay.as_mut()
                .map(|(d, f)| (d, *f, map_id));
            match goggles_tick {
                _ if machine.map_open ^ machine.map_open_timestamp.is_some() => (),
                Some((0, force, map_id)) if render_world.is_some() => {
                    self.goggles_start(machine, force, Some(map_id));
                    let _ = self.goggles_select_lens_delay.take();
                },
                Some((ticks, ..)) => if let Some(ui_tick) = machine.ui_tick() {
                    let amt = if ui_tick.is_player() { 6 } else { 1 };
                    *ticks = ticks.saturating_sub(amt);
                },
                _ => (),
            }
        }

        match self.settings_dirty.then(Settings::try_write) {
            Some(Some(mut settings)) => {
                if let Some(pathing) = &self.settings {
                    settings.pathing_mut().space = pathing.space.clone();
                }
                self.settings_dirty = false;
            },
            Some(None) =>
                log::debug!("settings unavailable for saving"),
            _ => (),
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

    pub fn gameplay_map_exit(&mut self, device_context: &Dx11Context, prev_map_id: NonZeroU32) -> anyhow::Result<()> {
        let res = self.packs.unload_map(device_context, prev_map_id.get());

        res
    }

    pub fn gameplay_map_enter(&mut self, device_context: &Dx11Context, map_id: NonZeroU32) -> anyhow::Result<()> {
        let res = self.packs.load_map(&self.render_backend.device, device_context, map_id.get());

        self.goggles_enter(true);

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

                self.settings_dirty = true;
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

    #[cfg(feature = "goggles")]
    const GOGGLES_START_DELAY_TICKS: u32 = 8 * 6;
    pub fn goggles_enter(&mut self, _force: bool) {
        // fastload or early notifications can throw off the lens selection...
        self.goggles_select_lens_delay = Some((Self::GOGGLES_START_DELAY_TICKS, _force));
    }
    pub fn goggles_exit(&mut self) {
        #[cfg(feature = "goggles")]
        if goggles::is_enabled() {
            goggles::clear_lens();
        }
        let _ = self.goggles_select_lens_delay.take();
    }

    #[cfg(feature = "goggles")]
    fn goggles_start(&mut self, machine: &mut RenderMachine, force: bool, map_id: Option<NonZeroU32>) {
        use crate::{
            render::goggles as render_goggles,
            space,
        };

        let settings = self.map_settings_ref(|s| s.map(|s| (
            s.space.goggles.enabled(),
            map_id.map(|map_id| s.space.goggles.map_depth_calibration(map_id.get()))
        )));

        if let Some((true, depth)) = settings {
            if let (false, needs_setup) = render_goggles::get_state() {
                log::info!("Goggles setup: {}...", if needs_setup { "initializing" } else { "restarting" });
                render_goggles::enable(needs_setup);
            }

            goggles::pick_lens(force);

            if let Some((min, max)) = depth {
                machine.depth_range = Some(space::MIN_DEPTH*min..space::MAX_DEPTH*max);
            }
        }
    }
}

unsafe impl Send for Engine {}
