use super::App;
use crate::anim::AnimationCommand;

impl App {
    pub fn animation_command(&mut self, command: AnimationCommand) {
        self.engine.command(command);
        let action: &'static str = match command {
            AnimationCommand::Next => "next",
            AnimationCommand::Previous => "previous",
            AnimationCommand::ToggleFavorite => "favorite toggled",
            AnimationCommand::ToggleDisabled => "visibility toggled",
            AnimationCommand::ResetPolicy => "policy reset",
            AnimationCommand::ToggleLock => "scene lock toggled",
            AnimationCommand::WeightUp => "weight increased",
            AnimationCommand::WeightDown => "weight decreased",
        };
        self.status = format!("animation {} · {}", action, self.engine.label());
    }
}
