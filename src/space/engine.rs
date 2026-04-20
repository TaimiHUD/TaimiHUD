use {
    crate::{
        controller::{
            pathing::{
                space::{PoiScale, TrailScale, TrailTextureMap},
                PathingEvent,
            },
            Controller,
        },
        exports::{
            runtime as rt,
            runtime::statistics::{StatsDesc, StatsRef, StatsUnit},
        },
        render::machine::{frame_log, RenderMachine, RenderPosition},
        settings::{pathing::SpaceSettings, PathingSettings, Settings},
        space::{
            dx11::RenderBackend,
            pack::{self, render::Drawing, ArcrenderSettings, PackRender, PackRenderList},
            DrawSpace,
        },
        timer::{PhaseState, TimerFile, TimerMarker},
    },
    anyhow::{anyhow, Context},
    arcffi::nn,
    bevy_ecs::prelude::*,
    glam::Vec3,
    glamour::{Box2, Box3, Rect, Size2, TransformMap},
    std::{collections::HashMap, mem, num::NonZeroU32, sync::Arc},
    taimi_d3d::{
        dx11::{
            depth::ClearFlags,
            prelude::*,
        },
        shader::ShaderKind,
    },
    taimi_hoard::{
        flags::{BitArray, BitSet, BitSlice, BitView, BitsNative},
        iters::IterExt as _,
        vec::vec32_eq,
    },
    taimi_meta::{
        coords::ScreenSpace,
        spatial::cull::MapFrustum,
        ui::{
            gameplay::{GameplayState, GameplayTransition},
            MapContext, MapOpen, LocalContext,
        },
    },
    taimi_sync::watched::{watch, Watched},
    tokio::sync::mpsc::{Receiver, Sender},
    std::{time::Instant, ops},
};
#[cfg(feature = "paths-interact")]
use {
    crate::controller::pathing::state::interactive::{InteractionEvent, InteractionEventAction},
    crate::settings::pathing::TriggerKind,
    tokio::sync::broadcast,
};
#[cfg(feature = "space-ecs")]
use {
    crate::{
        resources::ObjFile,
        space::object::{ObjectBacking, ObjectLoader},
        timer::RotationType,
    },
    glam::{Mat4, Vec3Swizzles, Vec4},
    itertools::Itertools as _,
    std::path::PathBuf,
};

#[cfg(feature = "goggles")]
use {
    crate::{space::goggles, settings::goggles::GogglesEnables},
    taimi_d3d::dx11,
};

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
    SettingsDirty,
    UiResize(Option<Size2<ScreenSpace>>),
    ProcessShader(&'static str, ShaderKind, taimi_d3d::blob::Blob, String),
    #[cfg(deleteme)]
    #[cfg(feature = "goggles")]
    GogglesRefreshLens {
        delay_override: Option<u32>,
        force: bool,
    },
    #[cfg(deleteme)]
    #[cfg(feature = "goggles")]
    GogglesClearLens,
    #[cfg(feature = "goggles")]
    RefreshEdgeScale,
}

fn handle_marker_timings(mut commands: Commands, mut query: Query<(Entity, &Marker, &mut Render)>) {
    let now = Instant::now();
    for (entity, marker, mut render) in &mut query {
        if now > marker.marker.end(marker.start.into()).into_std() {
            log::trace!(
                "Entity {} reached end after {}, despawning.",
                entity,
                marker.marker.duration
            );
            commands.entity(entity).despawn();
        } else if now > marker.marker.start(marker.start.into()).into_std() && render.disabled {
            log::trace!("Entity {} reached start at {}!", entity, marker.marker.timestamp);
            render.disabled = false;
        }
    }
}

pub struct Engine {
    receiver: Receiver<SpaceEvent>,
    gameplay: Watched<GameplayState>,
    pub render_backend: RenderBackend,
    #[cfg(feature = "space-ecs")]
    pub model_files: HashMap<PathBuf, ObjFile>,
    #[cfg(feature = "space-ecs")]
    pub object_kinds: HashMap<String, Arc<ObjectBacking>>,
    phase_states: Vec<Arc<PhaseState>>,
    associated_entities: HashMap<String, Vec<Entity>>,

    #[cfg(feature = "goggles")]
    #[cfg(deleteme)]
    goggles_select_lens_delay: Option<(u32, bool)>,

    schedule: Schedule,

    // ECS stuff
    pub world: World,

    pub packs: PackRender,
    pub arcdata: ArcrenderSettings,
    #[cfg(feature = "paths-interact")]
    pub interact_rx: Option<broadcast::Receiver<InteractionEvent>>,

    pub drawing: FrameContext,

    settings: Option<PathingSettings>,
    settings_dirty: bool,
}

impl Engine {
    pub fn initialise(
        machine: &RenderMachine,
        receiver: Receiver<SpaceEvent>,
        gameplay: &watch::Sender<GameplayState>,
    ) -> anyhow::Result<Engine> {
        Self::setup_stats();

        let display_size = machine
            .display_size()
            .ok_or_else(|| anyhow!("display size unknown"))?;

        let mut render_backend =
            RenderBackend::setup(display_size).context("Failed to set up render backend")?;
        render_backend
            .perspective_handler
            .constant_buffer_data
            .trail_expansion = TrailScale::DIRTY;
        render_backend
            .perspective_handler
            .constant_buffer_mapv_data
            .trail_expansion = TrailScale::DIRTY;

        #[cfg(feature = "space-ecs")]
        let model_files;
        #[cfg(feature = "space-ecs")]
        let object_kinds = {
            let models_dir = crate::ADDON_DIR.join("models");
            let object_descs =
                ObjectLoader::load_desc(&models_dir).context("Failed to load model descriptors")?;
            log::debug!("{:?}", object_descs);
            model_files =
                ObjFile::load(&models_dir, &object_descs).context("Failed to load model object")?;

            let object_kinds =
                object_descs.to_backings(&render_backend.device, &model_files, &render_backend.shaders);

            object_kinds
        };

        let world = World::new();

        let mut schedule = Schedule::default();

        schedule.add_systems(handle_marker_timings);

        let packs = PackRender::new(&render_backend).context("Initializing packs")?;
        PathingEvent::LoadAll.try_send();

        let mut gameplay = Watched::start_watching(gameplay);
        let _ = gameplay.try_get_mut();

        let engine = Engine {
            #[cfg(feature = "space-ecs")]
            model_files,
            receiver,
            gameplay,
            render_backend,
            #[cfg(feature = "space-ecs")]
            object_kinds,
            schedule,
            world,
            associated_entities: Default::default(),
            phase_states: Default::default(),
            packs,
            arcdata: ArcrenderSettings::DEFAULT,
            drawing: Default::default(),
            #[cfg(feature = "goggles")]
            #[cfg(deleteme)]
            goggles_select_lens_delay: Some((Self::GOGGLES_START_DELAY_TICKS, true)),
            #[cfg(feature = "paths-interact")]
            interact_rx: None,
            settings: None,
            settings_dirty: true,
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

    /// `Ok(true)` if freshly (re)initialized
    pub fn init_mut<F>(
        machine: &mut RenderMachine,
        slot: &mut Option<anyhow::Result<Self>>,
        f: F,
    ) -> anyhow::Result<bool>
    where
        F: FnOnce(&mut Self, &mut RenderMachine) -> anyhow::Result<()>,
    {
        let (enabled, _goggles) = Settings::try_read().map(|s| (
            s.enable_katrender,
            match s.pathing.as_ref() {
                #[cfg(feature = "goggles")]
                Some(p) => machine.goggles.settings_enables(&p.space),
                _ => Default::default(),
            },
        )).unwrap_or((false, Default::default()));

        let engine = match slot {
            None if machine.gameplay.is_initial() || !enabled => {
                // if early game loading or charsel, delay init
                // TODO: make this an option, but have fallback plan if you cause crashes...
                return Ok(false)
            },
            e => e,
        };
        let mut res = None;
        let fresh = engine.is_none();
        let engine = engine.get_or_insert_with(|| {
            log::debug!("setting up space engine...");
            let (tx, rx) = tokio::sync::mpsc::channel::<SpaceEvent>(64);
            #[cfg(feature = "goggles")]
            let _ = tx.try_send(SpaceEvent::RefreshEdgeScale);
            match crate::SPACE_SENDER
                .write()
                .map_err(|_| anyhow!("space sender poisoned?"))
            {
                Ok(mut sender) => *sender = Some(tx),
                Err(e) => {
                    res = Some(anyhow!("{e:#}"));
                    return Err(e)
                },
            }
            #[cfg(feature = "paths-interact")]
            let mut interact_rx = None;
            let gameplay = Controller::with_sender(|s| {
                #[cfg(feature = "paths-interact")]
                if let Some(pathing) = &s.pathing {
                    interact_rx = Some(pathing.shared.interact.events.subscribe());
                }
                s.gameplay.clone()
            });
            let Some(gameplay) = gameplay.flatten() else {
                anyhow::bail!("controller unavailable");
            };
            match Self::initialise(machine, rx, &gameplay) {
                Err(e) => {
                    res = Some(anyhow!("{e:#}"));
                    Err(e)
                },
                Ok(mut e) => {
                    #[cfg(feature = "paths-interact")]
                    {
                        e.interact_rx = interact_rx;
                    }
                    #[cfg(feature = "extension-nexus")]
                    machine.rtapi_setup();
                    #[cfg(feature = "goggles")]
                    machine.goggles.setup_engine(&mut e, _goggles);

                    Ok(e)
                },
            }
            .context("Space engine setup failed")
        });
        if let Some(e) = res {
            return Err(e)
        }
        match engine {
            Ok(..) if !enabled => Ok(false),
            Ok(e) => f(e, machine).map(move |()| fresh),
            Err(..) => Ok(false),
        }
    }

    /// TODO: revisit, avoid, etc
    pub fn cleanup_background(self) {
        let Self {
            render_backend,
            #[cfg(feature = "space-ecs")]
            model_files,
            #[cfg(feature = "space-ecs")]
            object_kinds,
            #[cfg(feature = "space-ecs")]
            phase_states,
            associated_entities,
            world,
            packs,
            ..
        } = self;
        render_backend.cleanup_background();
        packs.cleanup_background();
        log::debug!("skipping engine drop()");
        mem::forget((
            #[cfg(feature = "space-ecs")]
            model_files,
            #[cfg(feature = "space-ecs")]
            object_kinds,
            #[cfg(feature = "space-ecs")]
            phase_states,
            associated_entities,
            world,
        ));
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
                let backing = Arc::new(
                    ObjectBacking::create_marker(&self.render_backend, marker, _base_path.clone())
                        .context("marker object creation failed")?,
                );
                let entity = self.world.spawn((
                    #[cfg(feature = "space-ecs")]
                    Position(marker.position),
                    Marker {
                        phase: phase_state.clone(),
                        start: phase_state.start.into(),
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
                log::trace!(
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
                log::trace!("Despawning {entity} from timer {} markers", timer.name());
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
        self.packs.stop();
    }

    pub fn process_event(&mut self, machine: &mut RenderMachine) -> anyhow::Result<bool> {
        let ev = self.receiver.try_recv();
        if let Ok(ev) = &ev {
            log::trace!("recv SpaceEvent::{}", <&str>::from(ev));
        }
        match ev {
            Ok(event) => {
                use SpaceEvent::*;
                match event {
                    SettingsDirty => {
                        self.settings = None;
                        self.settings_dirty = true;
                    },
                    UiResize(display_size) => match display_size {
                        None => {},
                        Some(sz) => {
                            #[cfg(taimi_debug)]
                            log::debug!("TODO: resize event to {sz:?}");
                        },
                    },
                    ProcessShader(id, kind, bytecode, partial_id) => {
                        let res = self.render_backend.shaders.load_partial(&self.render_backend.device, &id, &bytecode, &partial_id);
                        if let Ok(..) = &res {
                            log::debug!("received shader {id}");
                            self.packs.draw_state.shaders_incomplete.remove(&(kind, id));
                        }
                        let _ = rt::log::warn_ok(res);
                    },
                    #[cfg(feature = "goggles")]
                    RefreshEdgeScale => {
                        let edge_scale = self.map_settings(|s| s.space.goggles.edge_scale());
                        let edge_scale = edge_scale.map(|s| (s, &machine.map.calibration));
                        let res = self
                            .render_backend
                            .depth_handler
                            .regen_edge(&self.render_backend.device, edge_scale)
                            .context("generating fill geometry");
                        if let Err(e) = res {
                            log::error!("{e:#}");
                        }
                    },
                    #[cfg(feature = "goggles")]
                    #[cfg(deleteme)]
                    GogglesClearLens if !goggles::is_enabled() => (),
                    #[cfg(feature = "goggles")]
                    #[cfg(deleteme)]
                    GogglesRefreshLens { force, delay_override } => {
                        if self.map_settings(|s| s.space.goggles.enabled()) {
                            self.goggles_enter(machine, force);
                        }
                        if let (Some(delay_override), Some((delay, ..))) =
                            (delay_override, &mut self.goggles_select_lens_delay)
                        {
                            *delay = 8 * delay_override;
                        }
                    },
                    #[cfg(feature = "goggles")]
                    #[cfg(deleteme)]
                    GogglesClearLens => {
                        goggles::clear_lens();
                    },
                    MarkerFeed(phase_state) => self.new_phase(phase_state).context("marker new phase")?,
                    MarkerReset(timer) => self.remove_phase(timer).context("marker remove phase")?,
                }
                Ok(true)
            },
            Err(_error) => Ok(false),
        }
    }
    fn process_gameplay_event(
        &mut self,
        machine: &mut RenderMachine,
        gameplay: GameplayState,
        trans: GameplayTransition,
        device_context: &mut Option<Dx11Context>,
    ) -> anyhow::Result<()> {
        let device_context = || match device_context {
            Some(ctx) => Ok(ctx),
                ctx @ None => unsafe {
                    self.render_backend.device.GetImmediateContext()
                }.context("I lost my context!").map(|c| ctx.insert(c)),
        };
        match gameplay {
            GameplayState::Gameplay { map_id: Some(new_map_id) } => {
                let prev = match trans {
                    GameplayTransition::Map { prev_map_id, .. } => prev_map_id,
                    GameplayTransition::Loaded { prev_map_id, .. } => prev_map_id,
                    GameplayTransition::Intermission { prev_map_id } => prev_map_id,
                };
                log::debug!(
                    "{}entering map {new_map_id}",
                    if prev == Some(new_map_id) { "re-" } else { "" }
                );
                self.gameplay_map_enter(machine, device_context()?, new_map_id)
            },
            GameplayState::Gameplay { map_id: None } => {
                log::info!("how do we know we loaded into a null map from {trans:?} to {gameplay:?}?");
                Ok(())
            },
            GameplayState::Intermission {
                prev_map_id: Some(_prev),
                next_map_id: None,
                initial: false,
            } => {
                #[cfg(taimi_debug)]
                log::debug!("leaving map {_prev}");
                #[cfg(deleteme)]
                {
                    self.goggles_exit();
                    //self.gameplay_map_exit(device_context()?, prev)
                }
                Ok(())
            },
            GameplayState::Intermission {
                prev_map_id: Some(prev),
                next_map_id: Some(next),
                initial: false,
            } =>
                if prev != next {
                    log::info!("forget about previous map {prev}, prepare for {next}!");
                    self.gameplay_map_exit(device_context()?, prev)
                } else {
                    Ok(())
                },
            _ => Ok(()),
        }
        .with_context(|| format!("Map load error from {trans:?} to {gameplay:?}"))
    }

    #[allow(dead_code)]
    pub fn check_phase_ends() {
        todo!("this is supposed to terminate a phase when there are no more markers, ideally we should actually make something that finds the latest timestamp between sounds, directions, markers, alerts etc");
    }

    fn prepare(&mut self, machine: &mut RenderMachine, device_context: &mut Option<Dx11Context>) -> anyhow::Result<()> {
        let gameplay_prev = self.gameplay.get_mut().clone();
        if let Some(gameplay) = self.gameplay.try_read_if_changed().cloned() {
            let trans = gameplay.latest_transition_from(gameplay_prev);
            let res = self
                .process_gameplay_event(machine, gameplay, trans, device_context)
                .with_context(|| format!("Map load error from {trans:?} to {gameplay:?}"));
            if let Err(e) = res {
                log::error!("{e:#}");
            }
        }
        for _ in 0..5 {
            // try to get a couple events out of the way at a time
            // (would be nice to batch pack loads)
            let processed = self
                .process_event(machine)
                .context("render engine event processing failure")?;
            if !processed {
                break
            }
        }
        #[cfg(feature = "paths-interact")]
        if let Some(rx) = &mut self.interact_rx {
            for _ in 0..48 {
                let when = || self.drawing.scene_start.as_ref()
                    .map(|s| s.elapsed().as_secs_f32());
                match rx.try_recv() {
                    Ok(InteractionEvent::Interact { action: InteractionEventAction::Report(action), loaded_path, path: _,  } ) => {
                        if action.contains(TriggerKind::BOUNCE) {
                            if let Some(when) = when() {
                                self.packs.poi_anim_start(loaded_path, when);
                            }
                        }
                    },
                    Ok(InteractionEvent::Gone { loaded_path, .. }) => {
                        if let Some(when) = when() {
                            self.packs.poi_anim_end(loaded_path, when);
                        }
                    },
                    Ok(..) => (),
                    Err(broadcast::error::TryRecvError::Lagged(..)) => (),
                    Err(..) => break,
                }
            }
        }
        self.schedule.run(&mut self.world);

        let settings_dirty = match machine.is_ingame() {
            Some(..) => mem::take(&mut self.settings_dirty),
            _ => false,
        };
        if settings_dirty {
            let _ = self.fill_settings();
            if let Some(settings) = &self.settings {
                self.drawing.prepare_enabled(settings);
                let display_size = match self.render_backend.display_size {
                    #[cfg(todo)]
                    sz => Some(sz),
                    _ => None,
                };
                self.arcdata.set_from(settings, display_size);
            }
        }
        let settings = self.settings.as_ref()
            .map(|s| &s.space);
        match self.packs.prepare(&self.render_backend.device, machine, settings) {
            Ok(prepared) => {
                self.drawing.prepared = prepared;
            },
            Err(e) => {
                self.drawing.prepared = false;
                return Err(e)
            },
        }
        //self.packs.update();

        #[cfg(feature = "goggles")]
        {
            let ctx = match device_context {
                None if machine.goggles.wants_d3d_context(&self.drawing) => unsafe {
                    self.render_backend.device.GetImmediateContext()
                }.ok().map(|c| &*device_context.insert(c)),
                ctx => ctx.as_ref(),
            };
            let view = &self.render_backend.depth_handler.render_target_view.views;
            machine.goggles.act_pre_render_frame(ctx, Some(view), &mut self.drawing);
        }
        Ok(())
    }
    pub fn setup_frame(
        &mut self,
        _machine: &mut RenderMachine,
        device_context: &Dx11Context,
    ) {
        self.packs.setup_frame(device_context)
    }

    pub fn prepare_new_frame(&mut self, machine: &mut RenderMachine) {
        self.drawing.new_frame(machine.latest_space_timestamp());
        frame_log!("engine; frame/new: +{:?}", self.drawing.time_offset());
        self.drawing.prepare_new_frame(machine);

        let anim_timestamp = self.drawing.is_drawing().then_some(self.drawing.time_offset());
        self.packs.prepare_frame(anim_timestamp);
    }
    fn pre_render(&mut self, machine: &mut RenderMachine, device_context: &mut Option<Dx11Context>) -> anyhow::Result<bool> {
        let is_stale = match () {
            #[cfg(feature = "goggles2-project")]
            _ if machine.goggles.is_enabled(GogglesEnables::PROJECT_ENABLE) =>
                self.drawing.is_stale_frame(),
            _ => true,
        };
        if is_stale {
            self.prepare_new_frame(machine);
        }
        self.prepare(machine, device_context)?;
        let drawing = self.drawing.prepared && self.drawing.is_enabled() && !machine.is_ingame_paused();
        Ok(drawing)
    }
    pub fn render(&mut self, machine: &mut RenderMachine) -> anyhow::Result<()> {
        let mut device_context = None;
        match self.pre_render(machine, &mut device_context) {
            Ok(true) => (),
            res => return res.map(drop),
        }
        let device_context = match &mut device_context {
            Some(ctx) => ctx,
            ctx @ None => ctx.insert(unsafe {
                self.render_backend.device.GetImmediateContext()
            }.context("I lost my context!")?),
        };

        self.drawing.prepare_present(machine);
        frame_log!("engine; frame/render vis={:?} enabled={:?} drawing={:?} drawn={:?}",
            self.drawing.visible,
            self.drawing.enabled,
            self.drawing.drawing,
            self.drawing.drawn,
        );
        if self.drawing.is_drawing() {
            self.draw0(machine, &*device_context);
        }
        Ok(())
    }
    #[cfg(feature = "goggles2-project")]
    #[inline]
    pub(crate) fn project_proceed(
        &mut self,
        _machine: &mut RenderMachine,
        ctx: LocalContext,
    ) -> bool {
        if !self.drawing.prepared { return false }
        match (ctx, self.drawing.drawn.has(ctx)) {
            #[cfg(todo)]
            (LocalContext::World, true) if desc.shadowboxing => (),
            (LocalContext::MAP, true) => return false,
            (LocalContext::MINIMAP, true) if self.drawing.map_anim.is_anim() => return false,
            _ => (),
        }
        true
    }
    #[cfg(feature = "goggles2-project")]
    pub fn render_carefully(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: DrawDescSpace,
        ctx: LocalContext,
    ) {
        #[cfg(todo)]
        if self.drawing.drawn.has(ctx) {
            // TODO: repeats aren't necessarily bad as long as fallback behaviour is well defined...
            // but maybe not needed if we can confirm success after a draw?
            return
        }

        match self.drawing.drawing.has(ctx) {
            false => return,
            true => (),
        }

        self.draw_carefully(machine, &device_context, desc, ctx);
    }
    #[cfg(feature = "goggles2-project")]
    fn draw_carefully(&mut self, machine: &mut RenderMachine, device_context: &Dx11Context,
        mut desc: DrawDescSpace,
        ctx: LocalContext,
    ) {
        if machine.goggles.project_wants_flush() {
            unsafe {
                device_context.Flush();
            }
        }
        #[cfg(todo)]
        let target = desc.goggles.render_view();
        #[cfg(todo)]
        let depth_view = desc.goggles.depth_view();
        let _state_prim = device_context.get_snapshot::<taimi_d3d::state::PrimitiveTopology>();
        let _state_blend = device_context.get_snapshot::<dx11::OMBlendState<Option<dx11::BlendState>>>();
        let _state_depth = device_context.get_snapshot::<dx11::OMDepthState>();
        let _state_raster = device_context.get_snapshot::<Option<dx11::RasterizerState>>();
        let _shaderp = device_context.get_snapshot::<Option<dx11::ShaderP>>();
        let _shaderv = device_context.get_snapshot::<Option<dx11::ShaderV>>();
        let _shaderlayout = device_context.get_snapshot::<Option<dx11::shader::InputLayout>>();
        // TODO: increase to max bleh? or is that 8?
        let _rendertarget =
            device_context.get_snapshot::<dx11::RenderTargetViews<[Option<dx11::RenderTargetView>; 8]>>();
        let _viewport = device_context.get_snapshot::<Vec<dx11::Viewport>>();
        let _scissor = device_context.get_snapshot::<Vec<dx11::ScissorRect>>();
        #[cfg(todo = "unnecessary")]
        let _index = device_context.get_snapshot::<Option<dx11::IndexBuffer>>();
        let _cbufferv = device_context.get_snapshot_buffers::<Vec<Option<dx11::buffer::ConstantBufferV>>>();
        let _cbufferp = device_context.get_snapshot_buffers::<Vec<Option<dx11::buffer::ConstantBufferP>>>();
        let _samplers = device_context.get_snapshot_buffers::<Vec<Option<dx11::buffer::SamplerState>>>();
        let _vbuffer = device_context.get_snapshot_buffers::<Vec<Option<dx11::VertexBuffer>>>();
        let _srvp = device_context.get_snapshot_buffers::<Vec<Option<dx11::buffer::ShaderResourceViewP>>>();
        #[cfg(todo = "unnecessary")]
        let _srvv = device_context.get_snapshot_buffers::<Vec<Option<dx11::buffer::ShaderResourceViewV>>>();
        let vp_rect = match desc.goggles.target_viewport {
            vp @ Some(..) => Some(vp),
            None if desc.goggles.buffer_compat => Some(None),
            None => {
                let vp = _viewport.state.iter()
                    .zip(_rendertarget.state.views.iter())
                    .find(|(_, rt)| desc.goggles.target_renderview.is_some() && rt.as_ref().map(|rt| *rt.as_d3d_raw()) == desc.goggles.target_renderview)
                    .and_then(|(vp, _)| vp.get());
                let vp_size = vp.map(|vp| (vp, vp.size2()))
                    .and_then(|(vp, sz)| match sz {
                        Size2 { width: 0.0f32, .. } | Size2 { height: 0.0f32, .. } => None,
                        _ => Some((vp, sz)),
                    });
                match desc.goggles.inherit {
                    #[cfg(todo = "unnecessary")]
                    false => {
                        let aspect = self.render_backend.viewport.viewport.Width / self.render_backend.viewport.viewport.Height;
                        let vp_size_valid = vp_size.and_then(|(vp, size)| {
                            if vp.viewport.TopLeftX != 0.0 || vp.viewport.TopLeftY != 0.0 { return None }
                            if (aspect - (size.width / size.height)).abs() > 2e-3 {
                                return None
                            }
                            Some(vp)
                        });
                        vp_size_valid
                    },
                    false => None,
                    _ => vp_size.map(|(vp, ..)| vp),
                }.map(|vp| Rect::new(vp.top_left().cast(), vp.size2().cast()))
            }.map(Some),
        };
        let Some(vp_rect) = vp_rect else {
            frame_log!("viewport missing");
            return
        };
        let can_inherit = match () {
            #[cfg(todo)]
            _ => {
                let mut rtviews = _rendertarget.state.views.iter().filter_map(|v| v.as_ref());
                let unique_rt = rtviews.next();
                desc.goggles.target_renderview.is_some() && unique_rt.map(|rtv| *rtv.as_d3d_raw()) == desc.goggles.target_renderview
                    && rtviews.next().is_none()
                    && _rendertarget.state.depth.as_ref().map(|dv| *dv.as_d3d_raw()) == desc.goggles.target_depthview
            },
            _ => false,
        };
        if can_inherit {
            desc.goggles.inherit = true;
        }
        #[cfg(deleteme)]
        {
        self.render_backend.depth_handler.inherit_depth = match machine.goggles.project.inherit_render {
            true if goggles::current_lens().is_some() => None,
            true => depth_view
                .map(|v| v.as_d3d_raw().as_ptr() as usize),
            false => None,
        }.unwrap_or(0);
        self.render_backend.depth_handler.inherit_render = match machine.goggles.project.inherit_render {
            _ if FerretResource::project_hack_shadowbox() => None,
            true => target,
            false if goggles::current_lens().is_some() => target,
            false => None,
        }.map(|v| v.as_d3d_raw().as_ptr() as usize).unwrap_or(0);
        }
        let mut buffer_compat = desc.goggles.buffer_compat;
        let (display_size, viewport) = (self.render_backend.display_size, self.render_backend.viewport);
        if let Some(vp_rect) = vp_rect {
            if vp_rect.size != display_size {
                buffer_compat = false;
            }
            self.render_backend.display_size = match vp_rect {
                #[cfg(todo)]
                vp => vp.max().to_vector().to_size(),
                vp => vp.size,
            };
            self.render_backend.viewport.viewport.Width = vp_rect.size.width;
            self.render_backend.viewport.viewport.Height = vp_rect.size.height;
            self.render_backend.viewport.viewport.TopLeftX = vp_rect.origin.x;
            self.render_backend.viewport.viewport.TopLeftY = vp_rect.origin.y;
        }
        #[cfg(deleteme)]
        match dv_size_u32 {
            Some((w, h)) if rt_size_u32.is_some() && rt_size_u32 != dv_size_u32 => {
                frame_log!("depth buffer incompatible!");
                let dv_ok = w as f32 == viewport.viewport.Width && h == viewport.viewport.Height as u32;
                if dv_ok && (desc.pass_is_obscured() || matches!(ctx, LocalContext::World) || desc.goggles.target_renderview.is_none()) {
                    self.render_backend.display_size = display_size;
                    self.render_backend.viewport = viewport;
                    desc.goggles.target_renderview = None;
                } else if desc.pass_is_obscured() || desc.colour_write {
                    return
                }
                buffer_compat = dv_ok;
            },
            _ => (),
        }
        desc.goggles.buffer_compat = buffer_compat;
        #[cfg(deleteme)]
        let mut cls = None;
        #[cfg(deleteme)]
        if let Some(rtv) = desc.goggles.target_renderview {
            goggles::class::ClassShared::with_seen(rtv, |buf| {
                cls = Some(buf.classification);
            });
        }
        #[cfg(deleteme)]
        let descmap = (machine.get_map_open_state().is_visible() && cls == Some(goggles::class::BufferClass::Target))
            .then(|| desc.clone());
        #[cfg(deleteme)]
        let ctx = if cls == Some(goggles::class::BufferClass::Minimap) {
            if self.drawing.frame_count % 1000 == 0 {
                log::debug!("minimap vp: {:?}", vp);
            }
            LocalContext::MINIMAP
        } else { ctx };
        #[cfg(deleteme)]
        if let LocalContext::Map(..) = ctx {
            desc.depth_write = false;
            desc.depth_read = false;
        }
        self.draw2(machine, device_context, desc, ctx);
        #[cfg(deleteme)]
        if let Some(mut desc) = descmap {
            //desc.goggles.target_depthview = None;
            desc.goggles.inherit = false;
            desc.depth_read = false;
            desc.depth_write = false;
            self.draw2(machine, device_context, desc, LocalContext::GLOBAL);
        }
        #[cfg(deleteme)] {
            self.draw(machine, device_context, true);
            self.render_backend.depth_handler.inherit_render = 0;
            self.render_backend.depth_handler.inherit_depth = 0;
        }
        self.render_backend.display_size = display_size;
        self.render_backend.viewport = viewport;
        if machine.goggles.project_wants_flush() {
            unsafe {
                device_context.Flush();
            }
        }
    }
    pub fn draw2(&mut self, machine: &mut RenderMachine, device_context: &Dx11Context, mut desc: DrawDescSpace, ctx: LocalContext) {
        if machine.goggles.project.project_depth_fill {
            desc.depth_write = false;
            #[cfg(deleteme)]
            {
                desc.depth_read = false;
            }
        }
        let mut state = DrawStateSpace::default();
        #[cfg(deleteme)]
        let mut setup = || {
            state.set_raster(device_context, &self.render_backend);
            state.set_sampler(device_context, &self.render_backend);
            state.set_viewport(device_context, &self.render_backend);
        };
        match ctx {
            LocalContext::World => {
                let (draw_trails, textured_trails, draw_pois, textured_pois) = self.map_settings(|s|
                    (
                        s.space.trail_alpha() > 0.0,
                        s.space.trail_textured_space(),
                        s.space.poi_alpha() > 0.0,
                        true,
                    )
                );
                desc.draw_trails = draw_trails.then_some(DrawDescEntity {
                    textured: textured_trails,
                });
                desc.draw_pois = draw_pois.then_some(DrawDescEntity {
                    textured: textured_pois,
                });
                let Some((camera, depth, cull)) = self.space_bounds(machine, &desc) else { return };
                self.setup_frame(machine, device_context);
                state.setup_target(device_context, &self.render_backend, &desc);
                #[cfg(deleteme)]
                {
                self.clear_scissor(device_context, &desc);
                }

                state.set_minimap_scissor(device_context, &self.render_backend, None);
                if desc.pass.get_pass() == Drawing::REFLECT {
                    let prev = desc.pass;
                    desc.pass.set_pass(Drawing::REFLECT_BELOW);
                    // TODO: reading depth here from shadowbox (and maybe world?) would interact with terrain properly,
                    // but this is usually just to see a little bit below the water's surface anyway so...
                    let prev_write = mem::replace(&mut desc.depth_write, false);
                    let prev_read = mem::replace(&mut desc.depth_read, false);
                    self.draw_space(machine, device_context, &desc, &mut state, camera, depth.clone(), &cull);
                    desc.pass = prev;
                    desc.depth_write = prev_write;
                    desc.depth_read = prev_read;
                }
                self.draw_space(machine, device_context, &desc, &mut state, camera, depth, &cull);
            },
            LocalContext::Map(ctx) => {
                let mut desc = DrawDescMap::from(desc);
                let (draw_trails, textured_trails, draw_pois, textured_pois) = self.map_settings(|s|
                    (
                        s.space.trail_alpha_map(ctx) > 0.0,
                        s.space.trail_textured_map(ctx),
                        s.space.poi_alpha_map(ctx) > 0.0,
                        true,
                    )
                );
                desc.draw_trails = draw_trails.then_some(DrawDescEntity {
                    textured: textured_trails,
                });
                desc.draw_pois = draw_pois.then_some(DrawDescEntity {
                    textured: textured_pois,
                });
                //desc.depth_write = true;
                if machine.goggles.project.project_depth_fill {
                    desc.depth_write = false;
                    desc.depth_read = false;
                    desc.stencil_write = false;
                    desc.stencil_read = false;
                } else {
                    desc = desc.to_map();
                }
                let local_bounds = (!desc.is_nop()).then(|| self.get_map_bounds(machine, ctx));
                let Some(local_bounds) = local_bounds else { return };
                self.setup_frame(machine, device_context);
                state.setup_target(device_context, &self.render_backend, &desc);
                self.draw0_map(machine, device_context, &desc, &mut state, ctx, local_bounds);
            },
        }
    }
    const PERSPECTIVE_SLOT: u32 = 0;
    const TEXTURE_TRAIL_SLOT: u32 = 0;
    #[cfg(deleteme)]
    pub fn setup_draw(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &DrawDescSpace,
    ) {
        self.render_backend
            .sampler_state
            .set(&device_context, Self::TEXTURE_TRAIL_SLOT);
        let (set_blend, set_viewport) = match () {
            #[cfg(feature = "goggles2-project")]
            _ if desc.goggles.is_project()  => (!machine.goggles.project.project_blend_force, true),
            _ => (true, true),
        };
        if set_blend {
            let blend_state = match () {
                #[cfg(feature = "goggles2-project")]
                _ if desc.goggles.is_project() && machine.goggles.project.project_shadow => &self.render_backend.blend_state_shadow,
                _ => &self.render_backend.blend_state,
            };
            blend_state.set(&device_context);
        }
        self.render_backend.depth_handler.setup(&device_context, &desc);
        if set_viewport {
            self.render_backend.viewport.set(&device_context);
        }
    }
    pub fn setup_draw_space_legacy(
        &mut self,
        _machine: &mut RenderMachine,
        device_context: &Dx11Context,
        _desc: &DrawDescSpace,
    ) {
        self.render_backend
            .perspective_handler
            .set(&device_context, Self::PERSPECTIVE_SLOT);
    }
    pub fn setup_minimap_scissor(
        &mut self,
        device_context: &Dx11Context,
        _desc: &DrawDescSpace,
        minimap_bounds: &Box2<ScreenSpace>,
    ) {
        #[cfg(feature = "goggles2-project")]
        if _desc.goggles.target_renderview.is_some() {
            return
        }
        self.render_backend
            .depth_handler
            .setup_minimap_scissor(&device_context, minimap_bounds);
    }
    fn map_bounds(
        &self,
        machine: &mut RenderMachine,
        desc: &DrawDescSpace,
        map_ctx: MapContext,
    ) -> Option<Box3<DrawSpace>> {
        if desc.is_nop() { return None }
        if !self.drawing.drawing.has(map_ctx) { return None }
        Some(self.get_map_bounds(machine, map_ctx))
    }
    fn get_map_bounds(
        &self,
        machine: &mut RenderMachine,
        map_ctx: MapContext,
    ) -> Box3<DrawSpace> {
        super::dx11::PerspectiveHandler::map_local_bounds(machine, map_ctx)
    }
    fn space_bounds(
        &mut self,
        machine: &mut RenderMachine,
        desc: &DrawDescSpace,
    ) -> Option<(RenderPosition, ops::Range<f32>, MapFrustum)> {
        if desc.is_nop() { return None }
        if !self.drawing.drawing.has(LocalContext::World) { return None }
        Some(self.get_space_bounds(machine, desc))
    }
    fn get_space_bounds(
        &mut self,
        machine: &mut RenderMachine,
        _desc: &DrawDescSpace,
    ) -> (RenderPosition, ops::Range<f32>, MapFrustum) {
        let (camera_source, distance_max, znear_fade, windshield) = self.settings.as_ref().map(|s|
            (
                s.space.camera_source(),
                s.space.distance_max(),
                s.space.edge_feather_scale().is_some(),
                s.space.goggles.arcrender_enabled() && s.space.poi_limit_size(),
            )
        ).unwrap_or((SpaceSettings::DEFAULT_CAMERA_SOURCE, SpaceSettings::DEFAULT_DISTANCE_MAX, true, false));
        let depth = machine.depth_range();
        let camera = machine.get_camera(camera_source);
        let cull_near = match (znear_fade, windshield) {
            (false, false) => 1.0,
            (false, true) => 0.25,
            (true, false) => 0.15,
            (true, true) => 0.075,
        };
        let cull = MapFrustum::from_camera_data(
            machine.get_fov().y,
            camera,
            machine.get_aspect_ratio(),
            depth.start * cull_near..depth.end.min(distance_max),
        );
        (camera, depth, cull)
    }
    fn minimap_bounds_screen(&self, machine: &RenderMachine) -> Option<Rect<ScreenSpace>> {
        match self.drawing.map_anim {
            taimi_meta::ui::MapOpen::Open => return None,
            _ => (),
        }
        if machine.is_ui_hidden() { return None }
        Some(self.get_minimap_bounds_screen(machine))
    }
    fn get_minimap_bounds_screen(&self, machine: &RenderMachine) -> Rect<ScreenSpace> {
        let bounds = machine.map.calibration.compass_bounds();
        machine.map.calibration.map(bounds)
    }
    pub fn clear_scissor(
        &mut self,
        device_context: &Dx11Context,
        _desc: &DrawDescSpace,
    ) {
        #[cfg(feature = "goggles2-project")]
        #[cfg(todo)]
        if _desc.goggles.target_renderview.is_some() {
            return
        }
        self.render_backend
            .depth_handler
            .set_scissor(&device_context, Box2::from_size(self.render_backend.display_size));
    }
    #[cfg(deleteme)]
    pub fn apply_masks(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &mut DrawDescSpace,
        minimap_visible: bool,
    ) {
        let masking_corners = self.render_backend.depth_handler.fill_edge.is_some();
        let masking = minimap_visible || masking_corners;
        let masking = masking && !machine.is_ui_hidden();
        #[cfg(feature = "goggles2-project")]
        let masking = masking && !desc.goggles.is_project();
        let masking = match masking {
            #[cfg(todo)]
            true if desc.stencil_write => Some(false),
            true if desc.depth_write => Some(true),
            #[cfg(todo = "unnecessary")]
            #[cfg(feature = "goggles")]
            true if desc.goggles.target_depthview.is_some() => match desc.goggles.depth_invert {
                // writing to stencil of game's buffer is a bad idea and won't clear
                #[cfg(todo)]
                false => Some(false),
                _ => Some(true),
            },
            _ => None,
        };
        match masking {
            Some(false) => {
                desc.stencil_write = true;
                desc.stencil_read = true;
            },
            _ => {
                desc.stencil_write = false;
                desc.stencil_read = false;
            },
        }
        if let Some(depth_fill) = masking {
            let backend = &mut self.render_backend;
            backend
                .depth_handler
                .setup_depth_write(&device_context, Some(depth_fill), &desc);

            if let Some((shader, layout)) = backend.shaders.vertex.get("mask") {
                layout.set(&device_context);
                shader.set(&device_context);
            }
            backend.depth_handler.setup_fill(&device_context);
        }

        if minimap_visible {
            if masking.is_some() {
                self.render_backend.depth_handler.fill_clipped(&device_context);
            }
            self.clear_scissor(device_context, desc);
        }

        if let Some(..) = masking {
            if masking_corners {
                self.render_backend
                    .depth_handler
                    .fill_corners(&device_context);
            }

            self.render_backend
                .depth_handler
                .setup_depth_write(&device_context, None, &desc);
        }
    }
    pub fn draw_map(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &DrawDescMap,
        state: &mut DrawStateSpace,
        map_ctx: MapContext,
        render_bounds: glamour::Rect<ScreenSpace>,
        local_bounds: glamour::Box3<DrawSpace>,
    ) {
        frame_log!("engine; draw/{map_ctx:?}");
        let (fwoom, trail_textured, trail_scale, trail_alpha, poi_scale, poi_alpha) = self
            .map_settings(|s| {
                (
                    s.space.map_open(),
                    s.space.trail_textured_map(map_ctx),
                    s.space.trail_scale_map(map_ctx),
                    s.space.trail_alpha_map(map_ctx),
                    s.space.poi_scale_map(map_ctx),
                    s.space.poi_alpha_map(map_ctx),
                )
            });
        {
            let vdata = &mut self.render_backend.perspective_handler.constant_buffer_mapv_data;
            vdata.poi_expansion = PoiScale::with_scale(poi_scale);
            let trail_expansion = TrailScale::with_scale(trail_scale);
            match trail_textured {
                true if vdata.trail_expansion == trail_expansion
                    && vdata.trail_texture != TrailTextureMap::UNTEXTURED =>
                    (),
                true => {
                    vdata.trail_texture.set_scale_from_expansion(trail_expansion);
                    vdata.trail_texture.v_offset = 0.0;
                },
                false => vdata.trail_texture = TrailTextureMap::UNTEXTURED,
            }
            vdata.trail_expansion = trail_expansion;
        }
        #[cfg(feature = "goggles2-project")]
        let map_is_projecting = match desc.goggles.is_project() {
            #[cfg(todo = "unnecessary")]
            true if !machine.goggles.is_enabled(GogglesEnables::PROJECT_MAP) => false,
            // methods that draw later on FB matter here...
            #[cfg(todo)]
            true if cls != BufferClass::World => false,
            p => p,
        };
        {
            // TODO: cpbuffer per type? just mixing them together for now...
            let alpha = trail_alpha * poi_alpha;
            let pdata = &mut self.render_backend.perspective_handler.constant_buffer_mapp_data;
            pdata.colour.w = match self.drawing.map_anim.progress_open().map(|p| p / 0.8) {
                #[cfg(feature = "goggles2-project")]
                _ if map_is_projecting => alpha,
                Some(p) if p < 1.0 => alpha * p * p * if fwoom { 1.0 } else { p },
                _ => alpha,
            };
        }
        #[cfg(feature = "goggles2-project")]
        let fwoom = match fwoom {
            true if map_is_projecting => false,
            f => f,
        };

        if trail_alpha > 0.0 || poi_alpha > 0.0 {
            let backend = &mut self.render_backend;
            backend
                .perspective_handler
                .update_map(machine, &backend.viewport, render_bounds, local_bounds, fwoom, &desc);

            backend.perspective_handler.update_map_cb(&device_context);

            state.setup_depth(device_context, &*backend, machine, desc);
            state.setup_blend(device_context, &*backend, machine, desc);
            backend
                .perspective_handler
                .set_map_cb(&device_context, Self::PERSPECTIVE_SLOT);

            let map_query = PackRenderList::map_bounds_to_query(map_ctx, local_bounds);
            let entities = self.packs.render_list.iter_markers_map(
                self.packs.pack_data.map_ref_as_slice(),
                map_ctx,
                &map_query,
            );
            PackRender::draw_map_entities(
                &mut self.packs.draw_state,
                &self.packs.poi_common,
                &device_context,
                &backend,
                map_ctx,
                entities,
            );
        }
        self.drawing.drawn.mark(map_ctx);
        self.drawing.tarnish_depth_ours(desc);
    }
    pub fn draw_space(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &DrawDescMap,
        state: &mut DrawStateSpace,
        camera: RenderPosition,
        depth: ops::Range<f32>,
        cull: &MapFrustum,
    ) {
        frame_log!("engine; draw/Space#{}", desc.pass);
        let arcrender = self.settings.as_ref().map(|s|
            s.space.goggles.arcrender_enabled()
        ).unwrap_or(false);
        #[cfg(feature = "goggles")]
        let goggles_2pass = desc.pass_is_obscured().then_some(self.settings.as_ref())
            .flatten()
            .and_then(|s| {
                let alpha = s.space.goggles.obscured_alpha();
                (alpha > 0.0).then_some((alpha, s.space.obscured_distance()))
            });
        #[cfg(feature = "goggles")]
        let goggles_2pass = match goggles_2pass {
            None if desc.pass_is_obscured() => {
                frame_log!("unexpected obscured draw");
                return
            },
            g @ Some(..) if desc.goggles.is_project() => g,
            Some(..) if !desc.goggles.is_project() && !desc.depth_read || desc.goggles.target_depthview.is_none() => {
                frame_log!("bad obscured draw config");
                return
            },
            g => g,
        };
        self.arcdata.apply_v(&mut self.packs.shared_v);
        self.arcdata.apply_p(&mut self.packs.shared_p, self.render_backend.viewport.size2().cast());
        #[cfg(todo = "unnecessary")]
        #[cfg(feature = "goggles2-project")]
        match desc.pass {
            //| DrawDescSpace::PASS_OBSCURED_SHADOWED
            | DrawDescSpace::PASS_REFLECTING | DrawDescSpace::PASS_REFLECTING_BELOW => {
                self.packs.shared_p.render.edge_feather = Vector2::<f32>::splat(ArcrenderSettings::FEATHER_SCALE_NONE).to_array();
                self.packs.shared_p.render.edge_feather_viewport = Vector2::splat(ArcrenderSettings::VIEWPORT_NONE);
            },
            _ => (),
        }
        let legacy_alpha = match () {
            #[cfg(feature = "goggles")]
            _ if desc.pass_is_obscured() => {
                if desc.depth_read {
                    state.set_depth_state(device_context, &self.render_backend, DrawStateSpace::DEPTH_OBSCURED);
                } else {
                    state.setup_depth(device_context, &self.render_backend, machine, desc);
                }
                let obscured_alpha = match (desc.pass.get_pass(), goggles_2pass) {
                    #[cfg(feature = "goggles2-project")]
                    (Drawing::OBSCURED_SHADOWED, Some((obscured_alpha, _))) => match obscured_alpha * 7.0 {
                        #[cfg(todo)]
                        a if a <= 1.0 => a,
                        #[cfg(todo)]
                        a => obscured_alpha * 2.5,
                        a => a,
                    },
                    (_, Some((obscured_alpha, _))) => obscured_alpha,
                    _ => 0.0,
                };
                let poi_alpha = &mut self.packs.shared_v.poi.marker.alpha;
                let trail_alpha = &mut self.packs.shared_v.trail.marker.alpha;
                let bigalpha = obscured_alpha * 2.0 / (*trail_alpha + *poi_alpha);
                *poi_alpha *= bigalpha;
                *trail_alpha *= bigalpha;
                obscured_alpha
            },
            _ => {
                state.setup_depth(device_context, &self.render_backend, machine, desc);
                self.packs.shared_v.poi.marker.alpha * self.packs.shared_v.trail.marker.alpha
            },
        };
        state.setup_blend(device_context, &self.render_backend, machine, desc);

        #[cfg(feature = "goggles")]
        let (cull_alt, cam_alt);
        let (cam, cull) = match desc.pass.get_pass() {
            #[cfg(feature = "goggles")]
            _ if desc.pass_is_obscured() => {
                let cull = match goggles_2pass {
                    Some((_, obscured_dist)) if obscured_dist < depth.end => {
                        cull_alt = MapFrustum::from_camera_data(
                            machine.get_fov().y,
                            camera,
                            machine.get_aspect_ratio(),
                            depth.start..obscured_dist,
                        );
                        &cull_alt
                    },
                    _ => cull,
                };
                (&camera, cull)
            },
            #[cfg(feature = "goggles2-project")]
            Drawing::REFLECT => {
                let (pos, dir, _up) = camera;
                let angle = camera.1.y.asin();
                let angle_edge = angle - machine.get_fov().y.copysign(pos.y) * 0.5;
                let (ang, ang_edge) = match pos.y {
                    y if y >= 0.0 => (angle, angle_edge),
                    _ => (-angle, -angle_edge),
                };
                if ang_edge >= 0.0f32 {
                    frame_log!("reflection surface invisible");
                    // TODO: move this to prepare check and disable earlier!
                    self.drawing.drawn.insert(desc.pass.to_drawn());
                    return
                }
                let magnitude = match () {
                    #[cfg(todo = "unnecessary")]
                    _ => angle.sin(),
                    _ => dir.y,
                };
                let dist = (pos.y / magnitude).abs();
                let too_far = match dist {
                    _ if pos.y.abs() > depth.end => true,
                    _dist => (pos.y / ang_edge.sin()).abs() > depth.end * 1.5,
                };
                if too_far {
                    frame_log!("reflection surface distant");
                    return
                }
                match ang {
                    #[cfg(deleteme)]
                    ang if ang < 0.0f32 => {
                        // TODO: beware positive dir.y - indicates looking up and not at the water!
                        let skewed = dir / magnitude;
                        let offset = skewed * pos.y;
                        //cam_alt = (pos - offset * 2.0, -dir, RenderMachine::LOCAL_UP);
                        //cam_alt = (pos + offset.with_y(-offset.y) * 2.0, -dir, RenderMachine::LOCAL_UP);
                        //cam_alt = (pos.with_x(pos.x+0.5), dir, RenderMachine::LOCAL_UP);
                        //cam_alt = (pos - offset, dir.with_y(-dir.y), RenderMachine::LOCAL_UP);
                        cam_alt = (pos - offset - glamour::Vector3::ZERO.with_y(dist), glamour::Vector3::Y, glamour::Vector3::Z);
                        far = match () {
                            //#[cfg(todo = "unnecessary")]
                            _ => offset.length(),
                            _ => dist,
                        };
                    },
                    _ => {
                        let redir = dir.with_y(-dir.y);
                        cam_alt = (pos.with_y(-pos.y), redir, RenderMachine::LOCAL_UP);
                        //far = dist.min(depth.end * 1.5);
                    },
                }
                #[cfg(todo)]
                let near = (far - depth.end).max(depth.start);
                let near = pos.y.abs().max(depth.start);
                // TODO: reuse obscured distance setting for this
                let far = (depth.end - depth.start) * 0.5 + near;
                cull_alt = MapFrustum::from_camera_data(machine.get_fov().y, cam_alt, machine.get_aspect_ratio(), near..far);
                (&cam_alt, &cull_alt)
            },
            #[cfg(feature = "goggles2-project")]
            Drawing::REFLECT_BELOW => {
                let (pos, _dir, _up) = camera;
                if pos.y < 0.0 { return }
                self.packs.shared_v.trail.marker.flags |= pack::instance::MarkerInstanceData::FLAG_RESERVED_14;
                self.packs.shared_v.poi.marker.flags |= pack::instance::MarkerInstanceData::FLAG_RESERVED_14;
                let near = pos.y/*.abs()*/.max(depth.start);
                let far = depth.end - depth.start + near;
                // TODO: slight angle adjustment for refraction or something idk how light works sorry
                cull_alt = MapFrustum::from_camera_data(machine.get_fov().y, camera, machine.get_aspect_ratio(), near..far);
                (&camera, &cull_alt)
            },
            #[cfg(feature = "goggles2")]
            Drawing::SHADOWBOX => {
                self.packs.shared_v.trail.marker.flags |= pack::instance::MarkerInstanceData::FLAG_RESERVED_14;
                self.packs.shared_v.poi.marker.flags |= pack::instance::MarkerInstanceData::FLAG_RESERVED_14;
                (&camera, cull)
            },
            _ => (&camera, cull),
        };

        let arcrender = arcrender.then(|| match desc.pass.get_pass() {
            #[cfg(feature = "goggles2")]
            Drawing::REFLECT | Drawing::REFLECT_BELOW => pack::render::ArcShaderVariant::Reflection,
            #[cfg(feature = "goggles2")]
            Drawing::SHADOWBOX => pack::render::ArcShaderVariant::Shadowboxing,
            #[cfg(feature = "goggles")]
            _ if desc.pass_is_obscured() => pack::render::ArcShaderVariant::Obscured,
            _ => pack::render::ArcShaderVariant::Vanilla,
        });
        let arcrender = match arcrender {
            Some(variant) if self.packs.resources.prepare_shaders_arc(&self.render_backend.shaders, &mut self.packs.draw_state, variant) => true,
            Some(..) if self.packs.draw_state.prev_waiting => true,
            _ => false,
        };
        if arcrender {
            ArcrenderSettings::setup_v(
                &mut self.packs.shared_v,
                self.render_backend.viewport_rect().size,
                &machine.map.calibration,
                cam,
                machine.get_player_pos().map(|(p, ..)| p),
                machine.get_space_perspective().matrix,
                RenderMachine::space_view(*cam).matrix,
                self.packs.resources.anim_timestamp,
            );
            #[cfg(feature = "goggles2-project")]
            match desc.pass.get_pass() {
                pass @ (Drawing::REFLECT | Drawing::REFLECT_BELOW) => {
                    // XXX: beware, this + camera_dir are used for culling as well as misc distance calc!
                    self.packs.shared_v.render.camera_pos = camera.0.to_vector().cast();
                    if pass == Drawing::REFLECT {
                        self.packs.shared_v.poi.billboard = match camera {
                            #[cfg(todo)]
                            cam_orig => {
                                // this looks too silly despite seeming "correct"
                                let orig_view = RenderMachine::space_view(cam_orig).matrix;
                                taimi_meta::coords::billboard_from_look(orig_view.into())
                            },
                            _ => {
                                let mut cam_below = *cam;
                                cam_below.0.y *= 0.5;
                                cam_below.1.y *= 0.5;
                                //cam_below.1 = cam_below.1.normalize_or(camera.1);
                                let orig_view = RenderMachine::space_view(cam_below).matrix;
                                taimi_meta::coords::billboard_from_look(orig_view.into())
                            },
                        };
                    }
                },
                _ => (),
            }
            self.packs.resources.update_shared(
                &device_context, &self.render_backend.device,
                &self.packs.shared_v, &self.packs.shared_p
            );
        } else {
            #[cfg(feature = "goggles2-project")]
            if desc.pass.intersects(Drawing::PASSES_INCOMPAT_LEGACY) {
                frame_log!("{} not supported by legacy renderer", desc.pass);
                return
            }
            self.render_backend.perspective_handler.set_alpha(legacy_alpha);
            let vdata = &mut self.render_backend.perspective_handler.constant_buffer_data;
            vdata.poi_expansion = self.arcdata.poi_expansion;
            vdata.trail_expansion = self.arcdata.trail_expansion;
            vdata.trail_texture = self.arcdata.trail_texture;
            let pdata = &mut self.render_backend.perspective_handler.constant_buffer_pixel_data;
            pdata.set_overlap_threshold(self.arcdata.trail_overlap_threshold);
            pdata.set_intensity(self.arcdata.trail_intensity);

            self.render_backend
                .perspective_handler
                .update_perspective(machine, *cam, Vec3::splat(self.arcdata.poi_expansion.scale()));
            self.render_backend
                .perspective_handler
                .set_feather_scale(self.arcdata.feather_scale1, self.render_backend.display_size);
            self.setup_draw_space_legacy(machine, device_context, desc);
            self.render_backend.perspective_handler.update_cb(&device_context);
        }

        match desc.pass.get_pass() {
            #[cfg(feature = "goggles")]
            pass if desc.pass_is_obscured() => {
                self.packs
                    .draw_obscured(cam, cull, &self.render_backend, &device_context, arcrender);
            },
            #[cfg(feature = "goggles2-project")]
            Drawing::REFLECT => {
                /// TODO: frustums have planes so this is dumb
                #[repr(transparent)]
                struct PlaneCull(MapFrustum);
                impl PlaneCull {
                    #[inline(always)]
                    fn with_frustum_ref(f: &MapFrustum) -> &Self {
                        unsafe { mem::transmute(f) }
                    }
                }
                impl bvh::aabb::IntersectsAabb<f32, 3> for PlaneCull {
                    fn intersects_aabb(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.max.y {
                            y if y < -Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb(aabb),
                        }
                    }
                }
                impl taimi_meta::spatial::cull::BvhQuery<3> for PlaneCull {
                    fn intersects_aabb_poi(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.max.y {
                            y if y < -Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb_poi(aabb),
                        }
                    }
                    fn intersects_aabb_shape(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.max.y {
                            y if y < -Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb_shape(aabb),
                        }
                    }
                }
                let cull = match cull {
                    #[cfg(todo)]
                    cull => MapFrustum {
                        bottom: y_axis_aligned_plane,
                        ..*cull
                    },
                    cull => PlaneCull::with_frustum_ref(cull),
                };
                self.packs
                    .draw_obscured(cam, cull, &self.render_backend, &device_context, arcrender);
            },
            #[cfg(feature = "goggles2-project")]
            Drawing::REFLECT_BELOW => {
                /// TODO: frustums have planes so this is dumb
                #[repr(transparent)]
                struct PlaneCull(MapFrustum);
                impl PlaneCull {
                    #[inline(always)]
                    fn with_frustum_ref(f: &MapFrustum) -> &Self {
                        unsafe { mem::transmute(f) }
                    }
                }
                impl bvh::aabb::IntersectsAabb<f32, 3> for PlaneCull {
                    fn intersects_aabb(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.min.y {
                            y if y > Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb(aabb),
                        }
                    }
                }
                impl taimi_meta::spatial::cull::BvhQuery<3> for PlaneCull {
                    fn intersects_aabb_poi(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.max.y {
                            y if y > Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb_poi(aabb),
                        }
                    }
                    fn intersects_aabb_shape(&self, aabb: &bvh::aabb::Aabb<f32, 3>) -> bool {
                        match aabb.max.y {
                            y if y > Engine::UNDERWATER_VISIBILITY => false,
                            _ => self.0.intersects_aabb_shape(aabb),
                        }
                    }
                }
                let cull = match cull {
                    #[cfg(todo)]
                    cull => MapFrustum {
                        bottom: y_axis_aligned_plane,
                        ..*cull
                    },
                    cull => PlaneCull::with_frustum_ref(cull),
                };
                self.packs
                    .draw_obscured(cam, cull, &self.render_backend, &device_context, arcrender);
            },
            #[cfg(feature = "goggles2-project")]
            Drawing::OBSCURED | Drawing::OBSCURED_SHADOWED => {
                self.packs
                    .draw_obscured(cam, cull, &self.render_backend, &device_context, arcrender);
                #[cfg(todo)]
                {
                    self.drawing.tarnish_depth_ours(desc);
                }
            },
            _ => {
                self.packs
                    .draw(cam, cull, &self.render_backend, &device_context, arcrender);
                self.drawing.tarnish_depth_ours(desc);
            },
        }
        self.drawing.drawn.insert(desc.pass.to_drawn());
    }
    /// overlap threshold to display paths just barely under the water's surface
    const UNDERWATER_VISIBILITY: f32 = match () {
        #[cfg(todo)]
        _ => 0.0f32,
        #[cfg(todo)]
        _ => 0.225f32,
        _ => 0.325f32 + 0.05,
    };
    #[cfg(feature = "space-ecs")]
    pub fn draw_ecs(
        &mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &DrawDescMap,
        state: &mut DrawStateSpace,
        camera: RenderPosition,
        depth: ops::Range<f32>,
        cull: &MapFrustum,
    ) {
        frame_log!("engine; draw/ecs");
        self.setup_draw_space_legacy(machine, device_context, desc);

        let mut drawn = false;
        let mut query = self.world.query::<(&mut Render, &Position)>();
        for (_k, c) in &query.iter(&self.world).chunk_by(|(r, _p)| r.backing.name.clone()) {
            let mut itery = c.into_iter();
            let Some(slice) = rt::log::debug_ok(itery.next().context("empty slice!")) else {
                continue
            };
            let (r, p) = slice;
            if !r.disabled {
                let rot = match r.rotation {
                    RotationType::Billboard => {
                        if let Some((pos, ..)) = machine.get_player_pos() {
                            let mark2d = (p.0.xz() - pos.xz().to_raw()).to_angle();
                            Mat4::from_rotation_y(-90.0f32.to_radians() - mark2d)
                        } else {
                            Mat4::IDENTITY
                        };
                        self.render_backend
                            .perspective_handler
                            .constant_buffer_data
                            .billboard
                    },
                    _ => Mat4::IDENTITY,
                };
                let ibd: Vec<_> = core::iter::once(slice)
                    .chain(itery)
                    .map(|(_r, p)| {
                        //  r.backing.render.metadata.model_matrix *
                        let affy =
                            Mat4::from_translation(p.0) * rot * r.backing.render.metadata.model_matrix;
                        super::dx11::InstanceBufferData {
                            world: affy,
                            //world_position: affy.translation,
                            colour: Vec4::ONE,
                        }
                    })
                    .collect();
                r.backing.set_and_draw(
                    Self::PERSPECTIVE_SLOT,
                    &self.render_backend.device,
                    &device_context,
                    &ibd,
                );
                drawn = true;
            }
        }
        if drawn {
            self.drawing.tarnish_depth_ours(desc);
        }
    }

    pub fn draw0(&mut self, machine: &mut RenderMachine, device_context: &Dx11Context) {
        self.setup_frame(machine, device_context);
        let mut state = DrawStateSpace::default();
        let mut desc = DrawDescSpace::empty();

        let is_rendering_world = self.drawing.drawing.has(LocalContext::World);

        let minimap_bounds =
            self.minimap_bounds_screen(machine);

        let stencil_prev = mem::take(&mut self.drawing.stencil);
        #[cfg(feature = "goggles")]
        {
            self.drawing.stencil.set_edge_scale(self.settings.as_ref()
                .map(|s| s.space.goggles.edge_scale())
                .unwrap_or(stencil_prev.edge_scale())
            );
        }
        self.drawing.stencil.set_minimap(minimap_bounds);
        let clear_stencil = stencil_prev != self.drawing.stencil;
        let clear_depth = self.drawing.depth_dirty_ours;
        let clear = (clear_depth | clear_stencil).then_some(
            IntoIterator::into_iter([
                clear_depth.then_some(ClearFlags::DEPTH),
                clear_stencil.then_some(ClearFlags::STENCIL),
            ]).flatten().collect()
        );
        if let Some(flags) = clear {
            self.render_backend.depth_handler.render_target_view.clear_depth(
                &*device_context,
                flags,
                DrawDescSpace::CLEAR_DEPTH,
                DrawStateSpace::STENCIL_CLEAR,
            );
            self.drawing.depth_dirty_ours = false;
        }

        let desc_map = desc.to_map();
        let minimap_bounds = (!self.drawing.drawn.has(MapContext::Minimap))
            .then(|| self.map_bounds(machine, &desc_map, MapContext::Minimap))
            .flatten();
        if let Some(bounds) = minimap_bounds {
            state.setup_target(device_context, &self.render_backend, &desc_map);
            self.draw0_map(machine, device_context, &desc_map, &mut state, MapContext::Minimap, bounds);
        }

        if clear_stencil {
            #[cfg(feature = "goggles")]
            let edge_scale = (stencil_prev.edge_scale != self.drawing.stencil.edge_scale)
                .then_some(self.drawing.stencil.edge_scale().map(|s| (s, &machine.map.calibration)));
            #[cfg(feature = "goggles")]
            if let Some(edge_scale) = edge_scale {
                let res = self
                    .render_backend
                    .depth_handler
                    .regen_edge(&self.render_backend.device, edge_scale)
                    .context("generating fill geometry");
                if let Err(e) = res {
                    log::error!("{e:#}");
                }
            }
            desc.stencil_write = true;
            if state.bound_depth == 0 {
                desc.colour_write = false;
                desc.colour_read = false;
                state.set_target(device_context, &self.render_backend, &desc);
            }
            self.drawing.stencil.apply_masks(
                device_context,
                &self.render_backend,
                &desc,
                &mut state,
            );
            desc.stencil_write = false;
            desc.colour_write = true;
            desc.colour_read = true;
        }

        #[cfg(feature = "space-ecs")]
        let ecs_empty = {
            let (tick_prev, tick) = (self.world.last_change_tick(), self.world.change_tick());
            self.world.query::<(Entity, &Render, &Position)>().is_empty(&self.world, tick_prev, tick)
        };
        #[cfg(feature = "goggles")]
        let is_rendering_obscured = is_rendering_world && self.drawing.drawing.has(Drawing::OBSCURED);
        #[cfg(feature = "goggles")]
        let has_drawn_obscured = !is_rendering_obscured || self.drawing.drawn.has(Drawing::OBSCURED);
        let is_rendering_world = match is_rendering_world {
            #[cfg(feature = "space-ecs")]
            true if !ecs_empty => true,
            true => !self.drawing.drawn.has(LocalContext::World),
            v => v,
        };
        let render_world = match is_rendering_world {
            #[cfg(feature = "goggles")]
            _ if !has_drawn_obscured => true,
            v => v,
        }.then(|| self.get_space_bounds(machine, &desc));
        #[cfg(feature = "goggles")]
        if render_world.is_some() && machine.goggles.is_enabled(GogglesEnables::LENS_ENABLE) {
            machine.goggles.lens.with_selected_lens(|lens| if let Some(lens) = lens {
                let buffer_compat = machine.goggles.lens.lens_compatible();
                let vp = match buffer_compat {
                    #[cfg(todo)]
                    false => machine.goggles.lens.vp_idk,
                    _ => None,
                };
                let mut g = DrawDescGoggles::with_buffers(vp, Some(lens), None);
                g.buffer_compat = buffer_compat;
                desc = g.to_space();
                if desc.stencil_write || is_rendering_world {
                    // DV fill is dumb if all we're doing is drawing the obscured paths...
                    state.setup_target(device_context, &self.render_backend, &desc);
                    self.drawing.stencil.apply_masks(
                        device_context,
                        &self.render_backend,
                        &desc,
                        &mut state,
                    );
                }
            });
        }
        if let Some((camera, depth, ref cull)) = render_world {
            let drawing_world = match () {
                #[cfg(feature = "space-ecs")]
                _ => !self.drawing.drawn.has(LocalContext::World),
                #[cfg(not(feature = "space-ecs"))]
                _ => true,
            };
            state.set_minimap_scissor(device_context, &self.render_backend, None);
            #[cfg(feature = "goggles")]
            if !has_drawn_obscured && desc.goggles.target_depthview.is_some() && desc.goggles.buffer_compat && desc.depth_read {
                let obscured_desc = DrawDescSpace {
                    depth_write: false,
                    pass: Drawing::OBSCURED,
                    ..desc
                };
                state.setup_target(device_context, &self.render_backend, &obscured_desc);
                self.draw_space(machine, device_context, &obscured_desc, &mut state, camera, depth.clone(), cull);
            }
            state.setup_target(device_context, &self.render_backend, &desc);
            if drawing_world {
                self.draw_space(machine, device_context, &desc, &mut state, camera, depth.clone(), cull);
            }

            #[cfg(feature = "space-ecs")]
            if !ecs_empty {
                self.draw_ecs(machine, device_context, &desc, &mut state, camera, depth, cull)
            }
        }

        let map_bounds = (!self.drawing.drawn.has(MapContext::Global))
            .then(|| self.map_bounds(machine, &desc_map, MapContext::Global))
            .flatten();
        if let Some(bounds) = map_bounds {
            state.setup_target(device_context, &self.render_backend, &desc_map);
            self.draw0_map(machine, device_context, &desc_map, &mut state, MapContext::Global, bounds);
        }

        state.cleanup(device_context, &self.render_backend);

        #[cfg(deleteme)]
        let render_minimap_bounds = match &minimap_bounds {
            None => None,
            Some(b) => Some((Box2::from(*b), b)),
        };
        #[cfg(deleteme)]
        for ctx in FrameContext::draw_contexts() {
            let map_ctx = match ctx {
                LocalContext::World => {
                    if let Some((camera, depth, ref cull)) = render_world {
                        if !self.drawing.has_drawn_context(LocalContext::World) {
                            frame_log!("engine draw: space");
                            let minimap_bounds = render_minimap_bounds.as_ref().map(|(b, ..)| b);
                            state.set_minimap_scissor(device_context, &self.render_backend, minimap_bounds);
                            self.draw_space(machine, device_context, &desc, &mut state, camera, depth.clone(), cull);
                        }

                        #[cfg(feature = "space-ecs")]
                        {
                            self.draw_ecs(machine, device_context, &desc, camera, depth, cull)
                        }
                    }
                    continue
                },
                LocalContext::Map(m) => m,
            };
            let local_bounds = match map_ctx {
                map_ctx if self.drawing.has_drawn_context(map_ctx) =>
                    None,
                map_ctx => self.map_bounds(machine, &desc, map_ctx),
            };
            let Some(local_bounds) = local_bounds else { continue };
            frame_log!("engine draw: {map_ctx:?}");
            let render_minimap_bounds = match render_minimap_bounds.as_ref() {
                Some(..) if !matches!(map_ctx, MapContext::Minimap) =>
                    None,
                b => b,
            };
            if !is_setup {
                self.setup_draw(machine, device_context, &desc);
                is_setup = true;
            }
            let screen_bounds = if let Some((render_minimap_bounds, minimap_bounds)) = render_minimap_bounds {
                self.setup_minimap_scissor(device_context, &desc, render_minimap_bounds);
                scissor_clear = false;
                **minimap_bounds
            } else {
                if !scissor_clear {
                    self.clear_scissor(device_context, &desc);
                    scissor_clear = true;
                }
                let r = self.render_backend.viewport.rect();
                glamour::Rect::new(
                    r.origin.cast(),
                    r.size.cast(),
                )
            };
            minimap_scissor_bound = !scissor_clear;
            #[cfg(feature = "goggles2-project")]
            if desc.depth_read && desc.goggles.target_depthview.is_some() {
                self.render_backend.depth_handler.depth_stencil_state_readonly.set(&device_context);
            }
            self.draw_map(machine, device_context, &desc, map_ctx, screen_bounds, local_bounds);
        }

        #[cfg(feature = "goggles")]
        #[cfg(deleteme)]
        if let Some(map_id) = machine.is_ingame() {
            let goggles_tick = self
                .goggles_select_lens_delay
                .as_mut()
                .map(|(d, f)| (d, *f, map_id));
            match goggles_tick {
                _ if machine.map_open ^ machine.map_open_timestamp.is_some() => (),
                Some((0, force, map_id)) if render_world.is_some() => {
                    self.goggles_start(machine, force, Some(map_id));
                    let _ = self.goggles_select_lens_delay.take();
                },
                Some((ticks, ..)) =>
                    if let Some(ui_tick) = machine.ui_tick() {
                        let amt = if ui_tick.is_player() { 6 } else { 1 };
                        *ticks = ticks.saturating_sub(amt);
                    },
                _ => (),
            }
        }
    }
    fn draw0_map(&mut self,
        machine: &mut RenderMachine,
        device_context: &Dx11Context,
        desc: &DrawDescMap,
        state: &mut DrawStateSpace,
        map_ctx: MapContext,
        local_bounds: Box3<DrawSpace>,
    ) {
        #[cfg(all(feature = "goggles", todo))]
        if desc.depth_read && desc.goggles.target_depthview.is_some() {
            self.render_backend.depth_handler.depth_stencil_state_readonly.set(&device_context);
        }
        let (scissor, render_bounds) = match map_ctx {
            MapContext::Global => (None, self.render_backend.viewport_rect()),
            #[cfg(feature = "goggles2-project")]
            MapContext::Minimap if machine.goggles.project.is_projecting() && self.render_backend.viewport.viewport.Width == self.render_backend.viewport.viewport.Height =>
                (None, self.render_backend.viewport_rect()),
            MapContext::Minimap => match (self.drawing.stencil.minimap, self.drawing.stencil.minimap_bounds()) {
                (Some(screen_bounds), Some(render_bounds)) =>
                    (Some(render_bounds), screen_bounds),
                _ => {
                    #[cfg(taimi_debug)]
                    log::debug!("minimap bounds missing??");
                    return
                },
            },
        };
        state.set_minimap_scissor(device_context, &self.render_backend, scissor.as_ref());
        self.draw_map(machine, device_context, &desc, state, map_ctx, render_bounds, local_bounds);
    }

    pub fn sender() -> Option<Sender<SpaceEvent>> {
        crate::SPACE_SENDER
            .try_read()
            .as_ref()
            .ok()
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

    pub fn cleanup(&mut self, unload: bool) {
        #[cfg(debug_assertions)]
        {
            log::warn!("TODO: Please clean up the engine when the program quits");
        }
        if unload {
            if let Ok(mut sender) = crate::SPACE_SENDER.write() {
                let _ = sender.take();
            }
            // pack cleanup kinda unnecessary since it's done on drop anyway?
        } else {
            self.packs.destroy_buffers();
        }
    }

    pub fn gameplay_map_exit(
        &mut self,
        _device_context: &Dx11Context,
        _prev_map_id: NonZeroU32,
    ) -> anyhow::Result<()> {
        let res = Ok(());

        self.drawing.end_scene();

        res
    }

    pub fn gameplay_map_enter(
        &mut self,
        _machine: &mut RenderMachine,
        _device_context: &Dx11Context,
        _map_id: NonZeroU32,
    ) -> anyhow::Result<()> {
        let res = Ok(());

        let prev_start = self.drawing.new_scene(None);
        self.packs.gameplay_map_enter(prev_start);

        #[cfg(feature = "goggles")]
        {
            let entry = self.settings.as_ref().and_then(|s|
                s.space.goggles.get_map_depth_calibration(_map_id.get())
            );
            if let Some(entry) = entry {
                _machine.set_depth_range(&entry);
            } else {
                _machine.depth_range = None
            }
        }

        res
    }

    fn fill_settings(&mut self) -> Result<&mut PathingSettings, Option<anyhow::Error>> {
        match &mut self.settings {
            Some(settings) => Ok(settings),
            settings @ None =>
                match Settings::read_with_blocking(|s| s.pathing.clone()) {
                    Ok(Some(s)) =>
                        Ok(settings.insert(s)),
                    Ok(None) =>
                        Err(None),
                    Err(e) =>
                        Err(Some(e)),
                }
        }
    }

    pub fn map_settings_ref<R, F: FnOnce(Option<&PathingSettings>) -> R>(&self, f: F) -> R {
        match self.settings.as_ref() {
            Some(s) => f(Some(s)),
            None => {
                let mut f = Some(f);
                let res = Settings::read_with_blocking(|s| match f.take() {
                    Some(f) => f(Some(&s.pathing())),
                    None => unreachable!(),
                })
                .context("engine settings unavailable");
                if let Err(e) = &res {
                    log::warn!("{e:#}");
                }
                match (f, res) {
                    (Some(f), _) => f(None),
                    (None, Ok(res)) => res,
                    (None, Err(..)) => unreachable!(),
                }
            },
        }
    }

    pub fn map_settings<R, F: FnOnce(&PathingSettings) -> R>(&mut self, f: F) -> R {
        let settings = match self.fill_settings() {
            Ok(s) => s,
            Err(None) =>
                &*self.settings.get_or_insert_default(),
            Err(Some(e)) => {
                log::warn!("engine settings unavailable: {e:#}");
                return f(&Default::default())
            },
        };
        f(settings)
    }

    #[cfg(feature = "goggles")]
    const GOGGLES_START_DELAY_TICKS: u32 = 8 * 6;
    #[cfg(deleteme)]
    pub fn goggles_enter(&mut self, machine: &mut RenderMachine, force: bool) {
        // fastload or early notifications can throw off the lens selection...
        self.goggles_lens_reset(Self::GOGGLES_START_DELAY_TICKS, force);
        machine.goggles.reset_search(force);
    }
    #[cfg(deleteme)]
    pub fn goggles_lens_reset(&mut self, ticks: u32, force: bool) {
        self.goggles_select_lens_delay = Some((ticks, force));
    }
    #[cfg(deleteme)]
    pub fn goggles_exit(&mut self) {
        #[cfg(feature = "goggles")]
        if goggles::is_enabled() {
            goggles::clear_lens();
        }
        let _ = self.goggles_select_lens_delay.take();
    }

    #[cfg(feature = "goggles")]
    #[cfg(deleteme)]
    fn goggles_start(&mut self, machine: &mut RenderMachine, force: bool, map_id: Option<NonZeroU32>) {
        use crate::render::goggles as render_goggles;

        let settings = self.map_settings_ref(|s| {
            s.map(|s| {
                (
                    s.space.goggles.enabled(),
                    map_id.map(|map_id| s.space.goggles.map_depth_calibration(map_id.get())),
                    ()
                )
            })
        });

        if let Some((true, depth)) = settings {
            if let (false, needs_setup) = render_goggles::get_state() {
                log::debug!(
                    "Goggles setup: {}...",
                    if needs_setup { "initializing" } else { "restarting" }
                );
                render_goggles::enable(needs_setup);
            }

            goggles::pick_lens(force);

            #[cfg(deleteme)]
            if let Some((min, max)) = depth {
                let reference = RenderMachine::GOGGLES_DEPTH_RANGE;
                machine.depth_range = Some(reference.start * min..reference.end * max);
            }
        }
    }

    pub(crate) fn setup_stats() {
        use crate::resources::texture;

        const SEC: &'static str = "stats-space-pack";
        const SEC3D: &'static str = "stats-space-engine-d3d";
        const SEC2D: &'static str = "stats-space-engine-textures";
        let stats_counters = &[
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_DRAW, StatsUnit::Count),
                StatsDesc::new(SEC, "stats-engine-drawn"),
            ),
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_DRAW_ALL, StatsUnit::Count),
                StatsDesc::new(SEC, "stats-engine-drawn-all"),
            ),
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_DRAW_PASS, StatsUnit::Count),
                StatsDesc::new(SEC, "stats-engine-drawn-pass"),
            ),
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_DRAW_MAP, StatsUnit::Count),
                StatsDesc::new(SEC, "stats-engine-mapped"),
            ),
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_COUNT, StatsUnit::Count),
                StatsDesc::new(SEC, "stats-engine-entities"),
            ),
            (
                StatsRef::with_counter(&pack::STATS_ENTITY_INSTANCE_SIZE, StatsUnit::Size),
                StatsDesc::new(SEC, "stats-engine-instance-entities"),
            ),
            (
                StatsRef::new(&pack::STATS_POI_INSTANCE_SIZE, StatsUnit::Size),
                StatsDesc::new(SEC3D, "stats-engine-instance-poi"),
            ),
            (
                StatsRef::new(&pack::STATS_TRAIL_VERTEX_SIZE, StatsUnit::Size),
                StatsDesc::new(SEC3D, "stats-engine-vertex-trail"),
            ),
            //#[cfg(feature = "texture-loader")]
            (
                StatsRef::new(&texture::STATS_TEXTURE_COUNT, StatsUnit::Count),
                StatsDesc::new(SEC3D, "stats-engine-texture-count"),
            ),
            (
                StatsRef::new(&texture::STATS_TEXTURE_SIZE, StatsUnit::Size),
                StatsDesc::new(SEC2D, "stats-engine-texture-size"),
            ),
            (
                StatsRef::new(&texture::STATS_TEXTURE_SIZE_CLONED, StatsUnit::Size),
                StatsDesc::new(SEC2D, "stats-engine-texture-size-max"),
            ),
        ];
        for &(counter, desc) in stats_counters {
            counter.register(desc);
        }
    }
}

unsafe impl Send for Engine {}

#[derive(Debug, Copy, Clone)]
pub struct DrawDescSpace {
    #[cfg(feature = "goggles")]
    pub goggles: DrawDescGoggles,
    /// similar to [DrawDescGoggles::depth_invert], except that we write the
    /// mask that obscures things (over UI or corners for example)
    ///
    /// TODO: when combined with goggles project, this can mean drawing a low-alpha
    /// 2nd pass in our normal render (imgui bg)
    #[cfg(todo)]
    pub obscured: bool,
    /// draw solid pixels in a first pass, then blend the rest in after
    #[cfg(todo)]
    pub opaque_pass: bool,
    pub pass: Drawing,
    /// whether to write back to depth buffer or not
    pub depth_write: bool,
    pub depth_read: bool,
    pub stencil_write: bool,
    pub stencil_read: bool,
    pub colour_write: bool,
    pub colour_read: bool,
    pub draw_trails: Option<DrawDescEntity>,
    pub draw_pois: Option<DrawDescEntity>,
}
impl DrawDescSpace {
    pub fn empty() -> Self {
        Self {
            goggles: DrawDescGoggles::empty(),
            pass: Self::PASS_DEFAULT,
            depth_read: true,
            depth_write: true,
            colour_write: true,
            colour_read: true,
            stencil_read: true,
            stencil_write: false,
            draw_pois: Some(DrawDescEntity::default()),
            draw_trails: Some(DrawDescEntity::default()),
        }
    }
    pub fn to_map(self) -> DrawDescMap {
        Self {
            depth_read: false,
            depth_write: false,
            stencil_read: false,
            stencil_write: false,
            .. self
        }
    }

    pub fn is_nop(&self) -> bool {
        self.draw_pois.is_none() && self.draw_trails.is_none()
    }
    #[inline(always)]
    pub fn implicit_render_target(&self) -> bool {
        #[cfg(feature = "goggles2-project")]
        if self.goggles.inherit { return true }
        false
    }
    pub fn null_depth_view(&self) -> bool {
        #[cfg(feature = "goggles2-project")]
        if self.goggles.target_depthview.is_some() {
            return false
        }
        #[cfg(feature = "goggles2-project")]
        if self.goggles.target_renderview.is_some() && !self.goggles.buffer_compat {
            return true
        }
        !self.depth_read && !self.depth_write && !self.stencil_read && !self.stencil_write
    }
    pub fn null_render_view(&self) -> bool {
        if !self.colour_write && !self.colour_read {
            return true
        }
        #[cfg(todo)]
        #[cfg(feature = "goggles2-project")]
        if self.goggles.shadowboxing { return true }
        false
    }
    #[inline]
    #[cfg(todo)]
    #[cfg(deleteme)]
    pub fn clear_depth(&self) -> Option<f32> {
        #[cfg(feature = "goggles")]
        if self.goggles.target_depthview.is_some() {
            return None
        }
        #[cfg(feature = "goggles2-project")]
        if self.goggles.inherit {
            return None
        }
        (self.depth_write && self.pass == 0).then_some(Self::CLEAR_DEPTH)
    }
    #[cfg(deleteme)]
    pub fn clear_depth(&self) -> Option<f32> { None }
    pub const CLEAR_DEPTH: f32 = 1.0f32;

    #[inline]
    pub fn pass_is_obscured(&self) -> bool {
        self.pass.get_pass().intersects(Drawing::OBSCURED | Drawing::OBSCURED_SHADOWED)
    }
    pub const PASS_DEFAULT: Drawing = Drawing::SPACE;
}
impl Default for DrawDescSpace {
    fn default() -> Self { Self::empty() }
}
#[derive(Debug, Copy, Clone)]
pub struct DrawDescEntity {
    pub textured: bool,
}
impl DrawDescEntity {
    pub fn new() -> Self {
        Self { textured: true }
    }
}
impl Default for DrawDescEntity {
    fn default() -> Self { Self::new() }
}
#[derive(Debug, Copy, Clone, Default)]
pub struct DrawDescTrail {
    pub entity: DrawDescEntity,
}
#[derive(Debug, Copy, Clone, Default)]
pub struct DrawDescPoi {
    pub entity: DrawDescEntity,
}
#[derive(Debug, Copy, Clone, Default)]
pub struct DrawDescWorldmap {
    pub map: DrawDescMap,
}
#[derive(Debug, Copy, Clone, Default)]
pub struct DrawDescMinimap {
    pub map: DrawDescMap,
}
pub type DrawDescMap = DrawDescSpace;
#[derive(Debug, Copy, Clone)]
#[cfg(feature = "goggles")]
pub struct DrawDescGoggles {
    pub(super) target_depthview: goggles::D3dPtr,
    /// whether target DV is expected to have been written to already
    /// (usually depends on how early/late in frame the draw is)
    pub depth_filled: bool,
    /// draw obscured objects at a different opacity
    pub depth_invert: bool,

    #[cfg(feature = "goggles2-project")]
    pub(super) target_renderview: goggles::D3dPtr,
    #[cfg(feature = "goggles2-project")]
    pub target_viewport: Option<Rect<ScreenSpace>>,
    /// whether target views are both already bound (RTV must be in first slot)
    #[cfg(feature = "goggles2-project")]
    pub inherit: bool,
    #[cfg(feature = "goggles2-project")]
    pub projecting: bool,
    /// if only one of RTV/DV are provided, it was validated to be the same size as our framebuffer
    ///
    /// this won't be true if super/subsampling is used, or when indirectly rendering to surfaces
    /// (e.g. minimap)
    pub buffer_compat: bool,
}
#[cfg(feature = "goggles")]
impl DrawDescGoggles {
    pub fn empty() -> Self {
        Self::with_buffers(None, None, None)
    }

    pub fn with_buffers(
        viewport: Option<Rect<ScreenSpace>>,
        depth_view: Option<&dx11::DepthView>,
        _target: Option<&dx11::RenderTargetView>,
    ) -> Self {
        Self {
            depth_filled: depth_view.is_some(),
            target_depthview: depth_view.map(|v| *v.as_d3d_raw()),
            depth_invert: false,
            #[cfg(feature = "goggles2-project")]
            target_renderview: _target.map(|v| *v.as_d3d_raw()),
            #[cfg(feature = "goggles2-project")]
            target_viewport: viewport,
            #[cfg(feature = "goggles2-project")]
            inherit: false,
            #[cfg(feature = "goggles2-project")]
            projecting: false,
            buffer_compat: false,
        }
    }
    #[cfg(feature = "goggles2-project")]
    pub fn render_view(&self) -> Option<&dx11::RenderTargetView> {
        self.target_renderview.as_ref().map(|raw| unsafe {
            dx11::RenderTargetView::from_d3d_raw_ref(raw)
        })
    }
    #[inline(always)]
    pub(super) fn has_render_view(&self) -> bool {
        match () {
            #[cfg(feature = "goggles2-project")]
            _ => self.target_renderview.is_some(),
            #[cfg(not(feature = "goggles2-project"))]
            _ => false,
        }
    }
    pub fn depth_view(&self) -> Option<&dx11::DepthView> {
        self.target_depthview.as_ref().map(|raw| unsafe {
            dx11::DepthView::from_d3d_raw_ref(raw)
        })
    }

    pub fn to_space(self) -> DrawDescSpace {
        DrawDescSpace {
            depth_read: self.target_depthview.is_some() || !self.has_render_view() || self.buffer_compat,
            depth_write: self.target_depthview.is_some() || !self.has_render_view() || self.buffer_compat,
            colour_read: true,
            colour_write: true,
            goggles: self,
            pass: DrawDescSpace::PASS_DEFAULT,
            stencil_read: false,
            .. Default::default()
        }
    }

    #[cfg_attr(not(feature = "goggles2-project"), inline(always))]
    pub fn is_project(&self) -> bool {
        match () {
            #[cfg(feature = "goggles2-project")]
            _ => self.projecting,
            #[cfg(not(feature = "goggles2-project"))]
            _ => false,
        }
    }
}
#[cfg(feature = "goggles")]
impl Default for DrawDescGoggles {
    fn default() -> Self { Self::empty() }
}

type DrawContextBits = [u16; 1];
type DrawContexts = BitSet<BitArray<DrawContextBits>, <DrawContextBits as BitView>::Store, BitsNative>;
#[derive(Debug, Clone)]
pub struct FrameContext {
    pub frame_count: u32,
    pub frame_index: u32,
    pub prepared: bool,
    pub visible: Drawing,
    pub enabled: Drawing,
    pub drawing: Drawing,
    pub drawn: Drawing,
    /// map entry timestamp
    pub scene_start: Option<Instant>,
    /// anim reference timestamp
    pub frame_start: Option<Instant>,
    pub map_anim: MapOpen,
    pub depth_dirty_ours: bool,
    pub stencil: FrameStencil,
}
impl FrameContext {
    pub fn empty() -> Self {
        Self {
            frame_count: 0,
            frame_index: 0,
            prepared: false,
            visible: Default::default(),
            enabled: Default::default(),
            drawing: Default::default(),
            drawn: Default::default(),
            map_anim: MapOpen::Closed,
            scene_start: None,
            frame_start: None,
            depth_dirty_ours: true,
            stencil: FrameStencil::default(),
        }
    }
    pub fn is_stale_frame(&self) -> bool {
        self.frame_count == self.frame_index
    }
    /// whatever was there is long gone now
    fn discard_frame(&mut self) {
        self.drawn.clear();
    }
    pub fn end_frame(&mut self) {
        self.frame_index = self.frame_count;
        self.frame_start = None;
        #[cfg(deleteme)]
        {
            self.discard_frame();
        }
    }
    pub fn new_frame(&mut self, when: Instant) {
        self.frame_count = self.frame_index.wrapping_add(1);
        self.frame_start = Some(when);
        self.discard_frame();
    }
    pub fn new_scene(&mut self, when: Option<Instant>) -> Option<Instant> {
        let when = when
            .or(self.frame_start)
            .unwrap_or_else(|| Instant::now());
        mem::replace(&mut self.scene_start, Some(when))
    }
    pub fn end_scene(&mut self) {
        self.scene_start = None;
    }
    #[cfg(deleteme)]
    pub fn refresh_frame<W>(&mut self, when: W) where
        W: FnOnce() -> Instant,
    {
        if self.is_stale_frame() {
            self.new_frame(when())
        }
    }
    pub fn schedule_frame(&mut self, when: Instant) {
        if self.frame_start.is_none() {
            self.frame_start = Some(when);
        }
    }
    pub fn time_offset(&self) -> f32 {
        match (&self.frame_start, self.scene_start) {
            (Some(frame), Some(scene)) => frame.saturating_duration_since(scene).as_secs_f32(),
            _ => 0.0f32,
        }
    }

    pub fn prepare_new_frame(&mut self, machine: &mut RenderMachine) {
        self.prepare_visible(machine);
    }
    pub fn prepare_present(&mut self, machine: &mut RenderMachine) {
        if self.drawing.has(LocalContext::MINIMAP) && machine.is_ui_hidden() {
            self.drawn.insert(Drawing::MINIMAP);
        }
    }
    fn prepare_visible(&mut self, machine: &mut RenderMachine) {
        self.visible.clear();
        if let Some(..) = machine.gameplay.gameplay_map() {
            self.map_anim = machine.map_open();
            let vis_space = !matches!(self.map_anim, MapOpen::Open);
            self.visible.set(Drawing::SPACE | Drawing::MINIMAP, vis_space);
            self.visible.set(Drawing::GLOBALMAP, self.map_anim.is_visible());
        } else {
            #[cfg(feature = "goggles2-camera")]
            {
                let cutscene_enable = GogglesEnables::CAMERA_CUTSCENE;
                #[cfg(todo)]
                let cutscene_enable = cutscene_enable | GogglesEnables::CAMERA_ENABLE;
                let can_render_cutscene = machine.goggles.is_enabled(cutscene_enable);
                if can_render_cutscene && machine.is_cutscene() {
                    self.visible.insert(Drawing::SPACE);
                }
            }
            self.map_anim = MapOpen::Closed;
        }
        self.prepare_drawing();
        #[cfg(feature = "goggles")]
        if !machine.goggles.is_enabled(GogglesEnables::LENS_ENABLE) {
            self.drawing.remove(Drawing::PASSES_OBSCURED);
        }
    }
    fn prepare_enabled(&mut self, settings: &PathingSettings) {
        self.enabled.clear();
        let space = &settings.space;
        for ctx in FrameContext::draw_contexts() {
            let vis = match ctx {
                LocalContext::World => space.visible_space() && (
                    Self::alpha_visible(space.trail_alpha())
                    | Self::alpha_visible(space.poi_alpha())
                ),
                LocalContext::Map(map_ctx) => space.visible_map(map_ctx) && (
                    Self::alpha_visible(space.trail_alpha_map(map_ctx))
                    | Self::alpha_visible(space.poi_alpha_map(map_ctx))
                ),
            };
            // TODO: consider thresholds around map open anim overlap edges?
            self.enabled.set(Drawing::from_context(ctx), vis);
        }
        #[cfg(feature = "goggles")]
        if self.enabled.contains(Drawing::SPACE) {
            let enables = space.goggles.enables();
            let obscured_enabled = space.goggles.obscured_alpha() > 0.0;
            if enables.contains(GogglesEnables::LENS_ENABLE) && obscured_enabled {
                self.enabled.insert(Drawing::OBSCURED);
            }
            #[cfg(feature = "goggles2-project")]
            if self.enabled.contains(Drawing::OBSCURED) && enables.contains(GogglesEnables::PROJECT_ENABLE) {
                self.enabled.insert(Drawing::OBSCURED_SHADOWED);
            }
            #[cfg(feature = "goggles2-project")]
            if enables.contains(GogglesEnables::PROJECT_ENABLE | GogglesEnables::PROJECT_SHADOWBOXING) {
                self.enabled.insert(Drawing::SHADOWBOX);
            }
            #[cfg(feature = "goggles2-project")]
            if enables.contains(GogglesEnables::PROJECT_ENABLE | GogglesEnables::PROJECT_REFLECTIONS) {
                self.enabled.insert(Drawing::PASSES_REFLECT);
            }
        }
        self.prepare_drawing();
    }
    fn prepare_drawing(&mut self) {
        let mut visible = self.visible;
        #[cfg(feature = "goggles")]
        if self.visible.contains(Drawing::SPACE) {
            visible.insert(Drawing::PASSES_OBSCURED);
            #[cfg(feature = "goggles2-project")]
            {
                visible.insert(Drawing::PASSES_PROJECT);
            }
        }
        visible &= self.enabled;
        self.drawing = visible;
    }
    fn alpha_visible(alpha: f32) -> bool {
        const NONE_U32: u32 = SpaceSettings::NONE_F32.to_bits();
        const NONE_ZERO: u32 = 0.0f32.to_bits();
        !matches!(alpha.to_bits(), NONE_U32 | NONE_ZERO)
    }

    pub fn tarnish_depth_ours(&mut self, desc: &DrawDescSpace) -> bool {
        #[cfg(feature = "goggles")]
        if desc.goggles.target_depthview.is_some() {
            return false
        }
        let tarnished = desc.depth_write;
        self.depth_dirty_ours |= tarnished;
        tarnished
    }
}
impl FrameContext {
    #[inline]
    #[cfg(deleteme)]
    pub fn drawing_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = LocalContext::REPR_MIN as usize..LocalContext::REPR_END as usize;
        unsafe {
            self.drawing.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn drawing_map_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = MapContext::REPR_MIN as usize..MapContext::REPR_END as usize;
        unsafe {
            self.drawing.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn drawn_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = LocalContext::REPR_MIN as usize..LocalContext::REPR_END as usize;
        unsafe {
            self.drawn.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn visible_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = LocalContext::REPR_MIN as usize..LocalContext::REPR_END as usize;
        unsafe {
            self.visible.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn visible_map_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = MapContext::REPR_MIN as usize..MapContext::REPR_END as usize;
        unsafe {
            self.visible.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn enabled_bits(&self) -> &BitSlice<u16, BitsNative> {
        let range = LocalContext::REPR_MIN as usize..LocalContext::REPR_END as usize;
        unsafe {
            self.enabled.get_unchecked(range)
        }
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn iter_drawing(&self) -> impl ExactSizeIterator<Item = LocalContext> + '_ {
        let bits = self.drawing_bits();
        bits.iter_ones()
            .lazy_map(|i| unsafe {
                LocalContext::from_repr_unchecked(LocalContext::REPR_MIN + i as u8)
            })
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn iter_drawing_map(&self) -> impl ExactSizeIterator<Item = MapContext> + '_ {
        let bits = self.drawing_map_bits();
        bits.iter_ones()
            .lazy_map(|i| unsafe {
                MapContext::from_repr_unchecked(MapContext::REPR_MIN + i as u8)
            })
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn iter_drawn(&self) -> impl ExactSizeIterator<Item = LocalContext> + '_ {
        let bits = self.drawn_bits();
        bits.iter_ones()
            .lazy_map(|i| unsafe {
                LocalContext::from_repr_unchecked(LocalContext::REPR_MIN + i as u8)
            })
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn iter_visible(&self) -> impl ExactSizeIterator<Item = LocalContext> + '_ {
        let bits = self.visible_bits();
        bits.iter_ones()
            .lazy_map(|i| unsafe {
                LocalContext::from_repr_unchecked(LocalContext::REPR_MIN + i as u8)
            })
    }
    #[inline]
    #[cfg(deleteme)]
    pub fn iter_visible_map(&self) -> impl ExactSizeIterator<Item = MapContext> + '_ {
        let bits = self.visible_map_bits();
        bits.iter_ones()
            .lazy_map(|i| unsafe {
                MapContext::from_repr_unchecked(MapContext::REPR_MIN + i as u8)
            })
    }
    #[inline]
    pub fn is_drawing(&self) -> bool {
        !self.drawing.is_empty()
    }
    #[cfg(deleteme)]
    pub fn is_drawing_map(&self) -> bool {
        self.drawing_map_bits().any()
    }
    #[cfg(todo)]
    #[cfg(deleteme)]
    pub fn is_visible(&self) -> bool {
        self.visible_bits().any()
    }
    pub fn is_enabled(&self) -> bool {
        !self.enabled.is_empty()
    }
    #[cfg(deleteme)]
    pub fn is_map_visible(&self) -> bool {
        self.visible_map_bits().any()
    }
    #[cfg(todo)]
    #[cfg(deleteme)]
    pub fn has_drawn(&self) -> bool {
        self.drawn_bits().any()
    }

    #[inline]
    pub fn draw_contexts() -> impl Iterator<Item = LocalContext> {
        IntoIterator::into_iter([
            LocalContext::MINIMAP,
            LocalContext::World,
            LocalContext::GLOBAL,
        ])
    }
    #[inline]
    pub fn draw_map_contexts() -> impl Iterator<Item = MapContext> {
        IntoIterator::into_iter([
            MapContext::Minimap,
            MapContext::Global,
        ])
    }
}
impl Default for FrameContext {
    fn default() -> Self { Self::empty() }
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct DrawStateSpace {
    pub setup: bool,
    pub minimap_scissor: Option<bool>,
    /// TODO: use IDs? (beware pointer comparisons because incremental/parallel linking?)
    pub bound_shader_v: &'static str,
    /// TODO: ditto
    pub bound_shader_p: &'static str,
    pub bound_depth: DrawStateId,
    pub bound_blend: DrawStateId,
    pub bound_target_depth: usize,
    pub bound_target_render: usize,
}
impl DrawStateSpace {
    pub const ID_EMPTY: DrawStateId = 0;
    pub const SHADER_MASK_NAME: &'static str = "mask";
    /// for drawing maps
    pub const DEPTH_IGNORE: DrawStateId = 3;
    pub const DEPTH_ON: DrawStateId = 4;
    /// write [FrameStencil::STENCIL_MASK_MINIMAP]
    pub const DEPTH_MASK_FILL_OPAQUE: DrawStateId = 5;
    /// fill depth instead of [Self::DEPTH_MASK_FILL_OPAQUE] for compatibility with low-bpp buffers
    #[cfg(feature = "goggles")]
    pub const DEPTH_MASK_FILL_FALLBACK: DrawStateId = 6;
    #[cfg(feature = "goggles")]
    pub const DEPTH_READONLY: DrawStateId = 7;
    #[cfg(feature = "goggles")]
    pub const DEPTH_OBSCURED: DrawStateId = 8;
    #[cfg(feature = "goggles")]
    pub const DEPTH_WRITEONLY: DrawStateId = Self::DEPTH_MASK_FILL_FALLBACK;

    pub const BLEND_ALPHA: DrawStateId = 1;
    #[cfg(feature = "goggles2-project")]
    pub const BLEND_SHADOW: DrawStateId = 2;
    pub const BLEND_WRITEONLY: DrawStateId = Self::BLEND_ALPHA;
    pub const BLEND_NOP: DrawStateId = Self::BLEND_ALPHA;

    pub fn set_shader_mask(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) -> bool {
        self.unset_shader_p(context, backend);
        if self.bound_shader_v == Self::SHADER_MASK_NAME {
            return true
        }
        if let Some((shader, layout)) = backend.shaders.vertex.get(Self::SHADER_MASK_NAME) {
            self.bound_shader_v = Self::SHADER_MASK_NAME;
            layout.set(context);
            shader.set(context);
            true
        } else {
            false
        }
    }
    pub fn unset_shader_v(
        &mut self,
        context: &Dx11Context,
        _backend: &RenderBackend,
    ) {
        if self.bound_shader_v.is_empty() { return }
        self.bound_shader_v = Default::default();
        None::<dx11::ShaderV>.set(context);
        None::<dx11::shader::InputLayout>.set(context);
        // TODO: IASetVertexBuffers?
    }
    pub fn unset_shader_p(
        &mut self,
        context: &Dx11Context,
        _backend: &RenderBackend,
    ) {
        if self.bound_shader_p.is_empty() { return }
        self.bound_shader_p = Default::default();
        None::<dx11::ShaderP>.set(context);
    }

    pub fn set_minimap_scissor(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        scissor: Option<&Box2<ScreenSpace>>,
    ) {
        let prev = mem::replace(&mut self.minimap_scissor, Some(scissor.is_some()));
        match scissor {
            _ if prev == self.minimap_scissor => (),
            Some(bounds) =>
                backend
                    .depth_handler
                    .setup_minimap_scissor(context, bounds),
            None => backend
                .depth_handler
                .set_scissor(context, backend.viewport_rect().to_box2()),
            #[cfg(deleteme)]
            None => backend
                .depth_handler
                .set_scissor(context, Box2::from_size(backend.display_size)),
        }
    }
    pub fn unset_scissor(
        &mut self,
        context: &Dx11Context,
    ) {
        if self.minimap_scissor.is_none() { return }
        self.minimap_scissor = None;
        unsafe {
            context.RSSetScissorRects(Some(&[]));
        }
    }

    pub fn set_depth_state(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        id: DrawStateId,
    ) -> bool {
        if self.bound_depth == id { return true }
        let state = match id {
            Self::DEPTH_IGNORE => &backend.depth_handler.depth_stencil_state_map,
            Self::DEPTH_ON => &backend.depth_handler.depth_stencil_state,
            Self::DEPTH_MASK_FILL_OPAQUE => &backend.depth_handler.depth_stencil_state_mask,
            #[cfg(feature = "goggles")]
            Self::DEPTH_MASK_FILL_FALLBACK => &backend.depth_handler.depth_stencil_state_write,
            #[cfg(feature = "goggles")]
            Self::DEPTH_READONLY => &backend.depth_handler.depth_stencil_state_readonly,
            #[cfg(feature = "goggles")]
            Self::DEPTH_OBSCURED => &backend.depth_handler.depth_stencil_state_obscured,
            _ => return false,
        };
        self.bound_depth = id;
        state.set(context);
        true
    }
    pub fn unset_depth_state(
        &mut self,
        context: &Dx11Context,
        _backend: &RenderBackend,
    ) {
        if self.bound_depth == Self::ID_EMPTY { return }
        self.bound_depth = Self::ID_EMPTY;
        dx11::OMDepthState::EMPTY.set(context);
    }
    pub fn set_depth_mask_fill(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        depth_fill_fallback: bool,
    ) -> bool {
        let depth_state = match depth_fill_fallback {
            #[cfg(feature = "goggles")]
            false => Self::DEPTH_MASK_FILL_FALLBACK,
            _ => Self::DEPTH_MASK_FILL_OPAQUE,
        };
        self.set_depth_state(context, backend, depth_state)
    }
    pub const STENCIL_REF: u32 = 0x01;
    pub const STENCIL_REF_MASK: u32 = 0x40;
    pub const STENCIL_CLEAR: u8 = 0;

    /// TODO: unset would be polite
    pub fn set_viewport(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) {
        backend.viewport.set(context);
    }
    /// TODO: unset would be polite
    pub fn set_raster(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) {
        backend.depth_handler.rasterizer_state.set(context);
    }
    /// TODO: unset would be polite
    pub fn set_sampler(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) {
        backend.sampler_state.set(context, Self::TEXTURE_TRAIL_SLOT);
    }
    const TEXTURE_TRAIL_SLOT: u32 = 0;

    pub fn setup(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) {
        if self.setup { return }
        self.set_raster(context, backend);
        self.set_sampler(context, backend);
        self.set_viewport(context, backend);
        self.setup = true;
    }
    pub fn unsetup(
        &mut self,
        context: &Dx11Context,
        _backend: &RenderBackend,
    ) {
        if !self.setup { return }
        None::<dx11::RasterizerState>.set(context);
        None::<dx11::buffer::SamplerState>.set(context, Self::TEXTURE_TRAIL_SLOT);
        dx11::Viewport::EMPTY.set(context);
        self.setup = false;
    }

    pub fn set_blend_state(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        id: DrawStateId,
    ) -> bool {
        if self.bound_blend == id { return true }
        let state = match id {
            Self::BLEND_ALPHA => &backend.blend_state,
            #[cfg(feature = "goggles2-project")]
            Self::BLEND_SHADOW => &backend.blend_state_shadow,
            _ => return false,
        };
        self.bound_blend = id;
        state.set(context);
        true
    }
    pub fn unset_blend_state(
        &mut self,
        context: &Dx11Context,
        _backend: &RenderBackend,
    ) {
        if self.bound_blend == Self::ID_EMPTY { return }
        self.bound_blend = Self::ID_EMPTY;
        dx11::OMBlendState::EMPTY.set(context);
    }

    const PERSPECTIVE_SLOT: u32 = 0;

    pub fn cleanup(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
    ) {
        self.unset_target(context);
        self.unsetup(context, backend);
        self.unset_shader_v(context, backend);
        self.unset_shader_p(context, backend);
        self.unset_depth_state(context, backend);
        self.unset_blend_state(context, backend);
        self.set_minimap_scissor(context, backend, None);
    }

    pub fn set_target(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        desc: &DrawDescSpace,
    ) {
        if !desc.implicit_render_target() {
            let dsview = backend.depth_handler.depth_stencil_view_with(desc);
            let target_depth = nn::nonnull_ptr_mut(dsview.depth.map(|v| *v.as_d3d_raw())) as usize;
            let target_render = nn::nonnull_ptr_mut(dsview.views.map(|v| *v.as_d3d_raw())) as usize;
            let (prev_depth, prev_render) = (
                mem::replace(&mut self.bound_target_depth, target_depth),
                mem::replace(&mut self.bound_target_render, target_render),
            );
            if prev_depth != target_depth || prev_render != target_render {
                dsview.set(context);
            }
        }
    }
    pub fn unset_target(
        &mut self,
        context: &Dx11Context,
    ) {
        if self.bound_target_depth == 0 && self.bound_target_render == 0 {
            return
        }
        self.bound_target_depth = 0;
        self.bound_target_render = 0;
        dx11::RenderTargetViews::with_views(None::<dx11::RenderTargetView>, None::<dx11::DepthView>)
            .set(context);
    }
    pub fn setup_target(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        desc: &DrawDescSpace,
    ) {
        self.setup(context, backend);
        self.set_target(context, backend, desc)
    }
    pub fn setup_depth(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        _machine: &mut RenderMachine,
        desc: &DrawDescSpace,
    ) -> bool {
        let id = if desc.null_depth_view() {
            Self::DEPTH_IGNORE
        } else if !desc.depth_write {
            if !desc.depth_read {
                Self::DEPTH_IGNORE
            } else {
                Self::DEPTH_READONLY
            }
        } else if !desc.depth_read {
            Self::DEPTH_WRITEONLY
        } else { Self::DEPTH_ON };
        self.set_depth_state(context, backend, id)
    }
    pub fn setup_blend(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        machine: &mut RenderMachine,
        desc: &DrawDescSpace,
    ) -> bool {
        let set_blend = match () {
            #[cfg(feature = "goggles2-project")]
            _ if desc.goggles.is_project()  => !machine.goggles.project.project_blend_force,
            _ => true,
        };
        let blend_state = set_blend.then_some(match () {
            #[cfg(feature = "goggles2-project")]
            _ if desc.goggles.is_project() && machine.goggles.project.project_shadow => Self::BLEND_SHADOW,
            _ => Self::BLEND_ALPHA,
        });
        if let Some(id) = blend_state {
            self.set_blend_state(context, backend, id)
        } else { true }
    }
}
type DrawStateId = usize;

#[derive(Debug, Clone)]
pub struct FrameStencil {
    pub minimap: Option<Rect<ScreenSpace>>,
    pub edge_scale: f32,
    pub ui_hidden: bool,
}
impl FrameStencil {
    pub fn empty() -> Self {
        Self {
            minimap: None,
            edge_scale: 0.0f32,
            ui_hidden: false,
        }
    }

    pub const STENCIL_MASK_MINIMAP: u8 = 0x01;
    pub const STENCIL_MASK_EDGE: u8 = Self::STENCIL_MASK_MINIMAP;

    pub fn is_empty(&self) -> bool {
        if self.ui_hidden { return true }
        self.minimap.is_none()
            && self.edge_scale.to_bits() == Self::EMPTY_EDGE_SCALE.to_bits()
    }

    pub fn set_minimap(&mut self, minimap: Option<Rect<ScreenSpace>>) {
        self.minimap = minimap;
    }
    pub fn minimap_bounds(&self) -> Option<Box2<ScreenSpace>> {
        self.minimap.as_ref().map(|b| b.to_box2())
    }

    const EMPTY_EDGE_SCALE: f32 = 0.0f32;
    pub fn set_edge_scale(&mut self, edge_scale: Option<f32>) {
        self.edge_scale = edge_scale.unwrap_or(Self::EMPTY_EDGE_SCALE);
    }
    pub fn edge_scale(&self) -> Option<f32> {
        (self.edge_scale.to_bits() != Self::EMPTY_EDGE_SCALE.to_bits()).then_some(self.edge_scale)
    }

    fn eq_fields(&self, rhs: &Self) -> bool {
        if self.edge_scale.to_bits() != rhs.edge_scale.to_bits() { return false }

        match (self.minimap.as_ref(), rhs.minimap.as_ref()) {
            (None, None) => (),
            (Some(ml), Some(mr)) =>
                if !vec32_eq(ml.origin, mr.origin) || !vec32_eq(ml.size, mr.size) {
                    return false
                },
            _ => return false,
        }

        true
    }

    #[inline]
    pub fn set_minimap_scissor(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        state: &mut DrawStateSpace,
    ) {
        state.set_minimap_scissor(context, backend, self.minimap_bounds().as_ref());
    }
    pub fn apply_masks(
        &mut self,
        context: &Dx11Context,
        backend: &RenderBackend,
        desc: &DrawDescSpace,
        state: &mut DrawStateSpace,
    ) {
        if self.is_empty() { return }
        let depth_mask_fallback = match desc.stencil_write {
            false if !desc.depth_write =>
                return,
            #[cfg(feature = "goggles")]
            _ if desc.goggles.target_depthview.is_some() && !desc.goggles.buffer_compat => return,
            stencil => !stencil,
        };
        let ok = state.set_depth_mask_fill(context, backend, depth_mask_fallback)
            && state.set_blend_state(context, backend, DrawStateSpace::BLEND_NOP)
            && state.set_shader_mask(context, backend);
        if !ok { return }

        state.setup_target(context, backend, desc);
        self.set_minimap_scissor(context, backend, state);
        if let Some(..) = self.minimap {
            backend.depth_handler.fill_clipped(context);
            state.set_minimap_scissor(context, backend, None);
        }

        backend.depth_handler.fill_corners(&context);
    }
}
impl Default for FrameStencil {
    fn default() -> Self { Self::empty() }
}
impl PartialEq for FrameStencil {
    fn eq(&self, rhs: &Self) -> bool {
        match (self.is_empty(), rhs.is_empty()) {
            (false, false) => self.eq_fields(rhs),
            (true, true) => true,
            _ => false,
        }
    }
}
