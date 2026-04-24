use bevy::app::{App, Plugin};
use crate::model_settings::resources::ModelSettings;

pub struct ModelSettingsPlugin;

impl Plugin for ModelSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelSettings::load());
    }
}
