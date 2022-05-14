use bevy::prelude::*;

pub fn window_system(
    mut windows: ResMut<Windows>,
    mouse_input: Res<Input<MouseButton>>,
    key_input: Res<Input<KeyCode>>,
) {
    let window = windows.primary_mut();

    if key_input.just_pressed(KeyCode::Space) {
        window.set_cursor_lock_mode(true);
        window.set_cursor_visibility(false);
    }

    if key_input.just_pressed(KeyCode::Escape) {
        window.set_cursor_lock_mode(false);
        window.set_cursor_visibility(true);
    }

    if window.cursor_locked() {
        let size = Vec2::new(window.width() as f32, window.height() as f32);
        window.set_cursor_position(size / 2.0);
    }
}
