use {
    super::{
        dx11::{prelude::*, InstanceBufferData, RenderBackend, PerspectiveInputData},
        object::{ObjectBacking, ObjectLoader},
        pack::PackCollection,
        MapContext, MapTarget,
    },
    crate::{
        exports::runtime as rt,
        controller::ControllerEvent,
        marker::atomic::MarkerInputData,
        resources::ObjFile,
        timer::{PhaseState, RotationType, TimerFile, TimerMarker},
        Controller, ADDON_DIR,
    },
    anyhow::Context,
    bevy_ecs::prelude::*,
    glam::Vec3,
    nexus::{imgui::Ui, rtapi::RealTimeApi},
    std::{collections::{HashMap, HashSet}, num::NonZeroU32, path::PathBuf, sync::Arc},
    tokio::{sync::mpsc::{Receiver, Sender}, time::Instant},
};

#[derive(Component)]
struct Render {
    disabled: bool,
    backing: Arc<ObjectBacking>,
    rotation: RotationType,
}
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
    position: Position,
    render: Render,
}

pub enum SpaceEvent {
    MarkerFeed(PhaseState),
    MarkerReset(Arc<TimerFile>),
    PathingToggle,
    MapToggle,
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
    pub model_files: HashMap<PathBuf, ObjFile>,
    pub object_kinds: HashMap<String, Arc<ObjectBacking>>,
    phase_states: Vec<Arc<PhaseState>>,
    associated_entities: HashMap<String, Vec<Entity>>,
    pub render_pathing: bool,
    pub render_pathing_map: bool,
    rtapi: Option<RealTimeApi>,
    gameplay_map: Result<NonZeroU32, u32>,

    schedule: Schedule,

    // ECS stuff
    pub world: World,

    pub packs: PackCollection,

    // need settings somewhere...
    #[cfg(feature = "goggles")]
    pub obscured_alpha: f32,
}

impl Engine {
    pub fn initialise(ui: &Ui, receiver: Receiver<SpaceEvent>) -> anyhow::Result<Engine> {
        let addon_dir = &*ADDON_DIR;

        let render_backend = RenderBackend::setup(&addon_dir, ui.io().display_size)
            .context("Failed to set up render backend")?;

        let models_dir = addon_dir.join("models");
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

        let world = World::new();

        let mut schedule = Schedule::default();

        schedule.add_systems(handle_marker_timings);

        let packs = PackCollection::new(&render_backend)
            .context("Initializing packs")?;
        Controller::try_send(ControllerEvent::PathingLoadAll);
        #[cfg(todo)]
        {
            packs.load_all(&addon_dir.join("pathing"))
                .context("Loading pathing packs")?;
        }

        let rtapi = match rt::rtapi() {
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
                log::debug!("RTAPI unavailable: {e}");
                None
            },
        };

        let mut engine = Engine {
            render_pathing: true,
            render_pathing_map: true,
            rtapi,
            gameplay_map: Err(0),
            model_files,
            receiver,
            render_backend,
            object_kinds,
            schedule,
            world,
            associated_entities: Default::default(),
            phase_states: Default::default(),
            packs,
            #[cfg(feature = "goggles")]
            obscured_alpha: 0.15,
        };

        Controller::try_send(ControllerEvent::RequestDisabledPaths);

        if let Some(backing) = engine.object_kinds.get("Cat") {
            engine.world.spawn((
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
            if let Some(base_path) = &phase_state.timer.path {
                let backing = Arc::new(ObjectBacking::create_marker(
                    &self.render_backend,
                    marker,
                    base_path.clone(),
                ).context("marker object creation failed")?);
                let entity = self.world.spawn((
                    Position(marker.position),
                    Marker {
                        phase: phase_state.clone(),
                        start: phase_state.start,
                        marker: marker.clone(),
                    },
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
                        self.render_pathing = !self.render_pathing;
                    },
                    MapToggle => {
                        self.render_pathing_map = !self.render_pathing_map;
                    },
                    PackLoad { pack, loader } => {
                        let pack_idx = self.packs.add_pack(pack, loader);
                        if let Err(e) = self.packs.load_pack(&self.render_backend.device, pack_idx) {
                            log::error!("{e}");
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
            log::error!("Map error: {e:?}");
        }

        self.packs.update();

        let backend = &mut self.render_backend;

        let render_map = match self.render_pathing_map && pdata.is_gameplay.unwrap_or(false) {
            true => map_data.as_ref().map(|data| MapTarget::new(data)),
            _ => None,
        };
        let render_world = match self.render_pathing && pdata.world_visible() {
            true => Some(self.packs.update_for_draw(&pdata, backend)),
            false => None,
        };

        let perspective_slot = 0;
        backend.blending_handler.set(&device_context);

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
            backend.depth_handler.setup_minimap_scissor(&device_context, minimap_bounds);
        }

        if let Some(map) = &render_map {
            backend.perspective_handler.update_map(map);

            backend.perspective_handler.update_map_cb(&device_context);

            backend.depth_handler.setup_map(&device_context, map);
            backend.perspective_handler.set_map_cb(&device_context, perspective_slot);

            let entities = self.packs.entities_map(map);
            PackCollection::draw_map_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, map, entities);
        }

        backend.depth_handler.setup(&device_context);

        if let Some(..) = &minimap_bounds {
            backend.depth_handler.setup_depth_write(&device_context, true);

            // TODO: reusing this shader is a hack
            backend.shaders.0["map"].set(&device_context);
            backend.depth_handler.fill_clipped(&device_context, &mut backend.perspective_handler);

            backend.depth_handler.setup_depth_write(&device_context, false);
            backend.depth_handler.clear_scissor(&device_context);
        }

        backend.perspective_handler.set_cb(&device_context, perspective_slot);

        // let mut query = self.world.query::<(&mut Render, &Position)>();
        // for (_k, c) in &query
        //     .iter(&self.world)
        //     .chunk_by(|(r, _p)| r.backing.name.clone())
        // {
        //     let mut itery = c.into_iter();
        //     let slice = itery.next().ok_or(anyhow!("empty slice!"))?;
        //     let (r, p) = slice;
        //     if !r.disabled {
        //         let rot = match r.rotation {
        //             RotationType::Billboard => {
        //                 let mark2d = (p.0.xz() - pdata.pos.xz()).to_angle();
        //                 let y = Mat4::from_rotation_y(-90.0f32.to_radians() - mark2d);
        //                 y
        //                 //Mat4::IDENTITY
        //             }
        //             _ => Mat4::IDENTITY,
        //         };
        //         let ibd: Vec<_> = vec![slice]
        //             .into_iter()
        //             .chain(itery)
        //             .map(|(_r, p)| {
        //                 //  r.backing.render.metadata.model_matrix *
        //                 let affy = Mat4::from_translation(p.0)
        //                     * rot
        //                     * r.backing.render.metadata.model_matrix;
        //                 InstanceBufferData {
        //                     world: affy,
        //                     //world_position: affy.translation,
        //                     colour: Vec3::new(1.0, 1.0, 1.0),
        //                 }
        //             })
        //             .collect();
        //         r.backing
        //             .set_and_draw(perspective_slot, &backend.device, &device_context, &ibd)?;
        //     }
        // }

        if let Some(context) = &render_world {
            #[cfg(feature = "goggles")]
            if crate::space::goggles::is_enabled() {
                let prev_alpha = backend.perspective_handler.alpha();

                // first pass at reduced opacity
                backend.perspective_handler.set_alpha(self.obscured_alpha);
                backend.perspective_handler.update_cb(&device_context);
                backend.depth_handler.set_state_obscured(&device_context, true);

                let entities = self.packs.entities_obscured(context);
                PackCollection::draw_entities(&self.packs.loaded_packs, &self.packs.poi_common, &device_context, &backend, entities);

                backend.perspective_handler.set_alpha(prev_alpha);
                backend.depth_handler.set_state_obscured(&device_context, false);
            }

            backend.perspective_handler.update_cb(&device_context);

            self.packs.draw(&pdata, context, &backend, &device_context);
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
        if crate::space::goggles::is_enabled() {
            crate::space::goggles::clear_lens();
        }

        let res = self.packs.unload_map(device_context, prev_map_id.get());

        self.gameplay_map = Err(prev_map_id.get());

        res
    }

    pub fn gameplay_map_enter(&mut self, device_context: &ID3D11DeviceContext, map_id: NonZeroU32) -> anyhow::Result<()> {
        #[cfg(feature = "goggles")]
        {
            crate::space::goggles::pick_lens();
        }

        let res = self.packs.load_map(&self.render_backend.device, device_context, map_id.get());

        self.gameplay_map = Ok(map_id);

        res
    }
}

unsafe impl Send for Engine {}
