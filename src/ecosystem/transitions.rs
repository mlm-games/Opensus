use bevy::prelude::*;

use crate::app::AppState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransitionKind {
    #[default]
    Fade,
    CircleWipe,
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
    pub circle_progress: f32,
    pub block_input: bool,
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
            circle_progress: 0.0,
            block_input: false,
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
        self.block_input = true;
    }

    pub fn begin_to_state_with(&mut self, next: AppState, kind: TransitionKind) {
        self.active = true;
        self.phase = TransitionPhase::Covering;
        self.progress = 0.0;
        self.pending_state = Some(next);
        self.kind = kind;
        self.block_input = true;
    }

    pub fn circle_wipe_progress(&self) -> f32 {
        if self.kind == TransitionKind::CircleWipe {
            match self.phase {
                TransitionPhase::Covering => self.progress,
                TransitionPhase::Uncovering => 1.0 - self.progress,
                TransitionPhase::Idle => 0.0,
            }
        } else {
            0.0
        }
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
        tr.circle_progress = (tr.circle_progress - 2.0 * real.delta_secs()).max(0.0);
        tr.block_input = false;
        return;
    }
    let dt = real.delta_secs() * tr.speed;
    match tr.phase {
        TransitionPhase::Covering => {
            tr.progress = (tr.progress + dt).min(1.0);
            let p = tr.progress;
            update_visuals(&mut tr, p, true);
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
            let p = tr.progress;
            update_visuals(&mut tr, p, false);
            if tr.progress >= 1.0 {
                tr.active = false;
                tr.phase = TransitionPhase::Idle;
                tr.overlay_alpha = 0.0;
                tr.circle_progress = 0.0;
                tr.block_input = false;
            }
        }
        TransitionPhase::Idle => {}
    }
}

fn update_visuals(tr: &mut Transition, t: f32, covering: bool) {
    match tr.kind {
        TransitionKind::Fade => {
            tr.overlay_alpha = if covering { t } else { 1.0 - t };
            tr.circle_progress = 0.0;
        }
        TransitionKind::CircleWipe => {
            tr.circle_progress = if covering { t } else { 1.0 - t };
            tr.overlay_alpha = if covering {
                (t * 1.2).min(1.0)
            } else {
                ((1.0 - t) * 1.2).min(1.0)
            };
        }
    }
}

pub fn input_blocked(tr: Res<Transition>) -> bool {
    tr.block_input
}
