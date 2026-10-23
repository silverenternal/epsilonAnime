use super::*;

impl Animation for Adaptive {
    fn update(&mut self, ctx: &FrameCtx<'_>) {
        self.update_scene_selection(ctx);
        self.update_clock(ctx);
        self.transition = (self.transition - ctx.dt * 1.8).max(0.0);
        if let Some(sub) = &mut self.sub {
            sub.update(ctx);
            return;
        }
        self.update_trail(ctx);
    }

    fn sample(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> f32 {
        self.mixed_field(x, y, ctx).intensity
    }

    fn field(&self, x: f32, y: f32, ctx: &FrameCtx<'_>) -> FieldSample {
        self.mixed_field(x, y, ctx)
    }

    fn label(&self) -> &'static str {
        MODE_NAMES[self.mode]
    }

    fn command(&mut self, command: AnimationCommand) {
        match command {
            AnimationCommand::Next => {
                self.set_manual_mode(self.policy.step(MODE_NAMES[self.mode], &MODE_PROFILES, 1))
            }
            AnimationCommand::Previous => {
                self.set_manual_mode(self.policy.step(MODE_NAMES[self.mode], &MODE_PROFILES, -1))
            }
            AnimationCommand::ToggleFavorite => {
                let id = MODE_NAMES[self.mode];
                if !self.policy.favorites.remove(id) {
                    self.policy.favorites.insert(id.into());
                }
                self.policy.save();
            }
            AnimationCommand::ToggleDisabled => {
                let id = MODE_NAMES[self.mode];
                if !self.policy.disabled.remove(id) {
                    self.policy.disabled.insert(id.into());
                    self.policy.favorites.remove(id);
                }
                self.policy.save();
                self.set_manual_mode(self.policy.step(MODE_NAMES[self.mode], &MODE_PROFILES, 1));
            }
            AnimationCommand::ResetPolicy => self.policy.reset(),
            AnimationCommand::ToggleLock => self.locked = !self.locked,
            AnimationCommand::WeightUp => {
                self.policy.adjust_weight(MODE_NAMES[self.mode], 0.25);
                self.policy.save();
            }
            AnimationCommand::WeightDown => {
                self.policy.adjust_weight(MODE_NAMES[self.mode], -0.25);
                self.policy.save();
            }
        }
    }
}

impl Adaptive {
    fn set_manual_mode(&mut self, mode: usize) {
        if mode != self.mode {
            self.previous_mode = Some(self.mode);
            self.transition = 1.0;
            self.mode = mode;
            self.flash = 0.35;
            self.prepare_mode();
        }
    }
}
