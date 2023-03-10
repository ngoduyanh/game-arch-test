use anyhow::Context;
use winit::event::{Event, WindowEvent};

use crate::{
    events::{GameEvent, GameUserEvent},
    exec::main_ctx::MainContext,
    scene::main::RootScene,
    utils::error::ResultExt,
};

pub fn handle_event<'a>(
    ctx: &mut MainContext,
    _: &RootScene,
    event: GameEvent<'a>,
) -> Option<GameEvent<'a>> {
    match &event {
        Event::WindowEvent {
            window_id,
            event: WindowEvent::CloseRequested,
        } if ctx.display.get_window_id() == *window_id => {
            ctx.event_loop_proxy
                .send_event(GameUserEvent::Exit(0))
                .map_err(|e| anyhow::format_err!("{}", e))
                .context("unable to send event to event loop")
                .log_warn();
        }

        _ => {}
    }

    Some(event)
}
