use bevy::prelude::*;

use crate::app::AppState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransitionKind {
    #[default]
    Fade,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransitionPhase {
    Idle,
    Covering,
    Uncovering,
}

impl Default for TransitionPhase {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Resource)]
pub struct Transition {
    pub active: bool,
    pub kind: TransitionKind,
    pub phase: TransitionPhase,
    pub progress: f32,
    pub speed: f32,
    pub pending_state: Option<AppState>,
    pub overlay_alpha: f32,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            active: false,
            kind: TransitionKind::Fade,
            phase: TransitionPhase::Idle,
            progress: 0.0,
            speed: 2.5,
            pending_state: None,
            overlay_alpha: 0.0,
        }
    }
}

impl Transition {
    pub fn begin_to_state(&mut self, next: AppState) {
        self.active = true;
        self.phase = TransitionPhase::Covering;
        self.progress = 0.0;
        self.pending_state = Some(next);
        self.kind = TransitionKind::Fade;
    }
}

pub struct Transitions;

impl Transitions {
    pub fn change_scene_with(transition: &mut Transition, next: AppState) {
        transition.begin_to_state(next);
    }
}

pub struct TransitionsPlugin;
impl Plugin for TransitionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Transition>()
            .add_systems(Update, tick_transition);
    }
}

fn tick_transition(
    real: Res<Time<Real>>,
    mut tr: ResMut<Transition>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !tr.active {
        tr.overlay_alpha = 0.0;
        return;
    }
    let dt = real.delta_secs() * tr.speed;
    match tr.phase {
        TransitionPhase::Covering => {
            tr.progress = (tr.progress + dt).min(1.0);
            tr.overlay_alpha = tr.progress;
            if tr.progress >= 1.0 {
                if let Some(s) = tr.pending_state.take() {
                    next_state.set(s);
                }
                tr.phase = TransitionPhase::Uncovering;
                tr.progress = 0.0;
            }
        }
        TransitionPhase::Uncovering => {
            tr.progress = (tr.progress + dt).min(1.0);
            tr.overlay_alpha = 1.0 - tr.progress;
            if tr.progress >= 1.0 {
                tr.active = false;
                tr.phase = TransitionPhase::Idle;
                tr.overlay_alpha = 0.0;
            }
        }
        TransitionPhase::Idle => {}
    }
}
