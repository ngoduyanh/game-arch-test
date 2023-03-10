use std::sync::Arc;

use anyhow::Context;

use crate::{
    events::GameEvent,
    exec::{main_ctx::MainContext, server::draw::ServerSendChannelExt},
    graphics::context::DrawContext,
    utils::args::args,
};

use self::handle_resize::HandleResize;

use super::{Scene, SceneContainer};

pub mod content;
pub mod core;
pub mod handle_resize;
pub mod test;
pub mod utility;

#[derive(Clone)]
pub struct RootScene {
    container: Arc<SceneContainer>,
}

impl RootScene {
    pub fn new(main_ctx: &mut MainContext) -> anyhow::Result<Self> {
        let mut container = SceneContainer::new();
        container.push(HandleResize::new());
        container.push_all(core::new(main_ctx).context("unable to initialize handle core scene")?);
        if args().test {
            container.push_all(test::new(main_ctx).context("unable to initialize test scene")?);
        } else {
            container
                .push_all(content::new(main_ctx).context("unable to initialize content scene")?);
        }
        container.push_all(utility::new(main_ctx).context("unable to initialize utility scene")?);
        let slf = Self {
            container: Arc::new(container),
        };

        let draw_self = slf.clone();
        main_ctx
            .channels
            .draw
            .execute(move |_, root_scene_opt| {
                *root_scene_opt = Some(draw_self);
            })
            .context("unable to share root scene with draw server")?;

        Ok(slf)
    }

    pub fn handle_event(&self, ctx: &mut MainContext, event: GameEvent) {
        self.container.clone().handle_event(ctx, self, event);
    }

    pub fn draw(&self, draw_ctx: &mut DrawContext) {
        self.container.clone().draw(draw_ctx);
    }
}

#[test]
fn test_sync() {
    use crate::assert_sync;

    assert_sync!(RootScene);
}
