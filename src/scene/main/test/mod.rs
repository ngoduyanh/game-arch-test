use anyhow::Context;

use crate::{exec::main_ctx::MainContext, scene::SceneContainer};

use self::headless::Headless;

pub mod headless;
pub mod timeout_delay;

pub fn new(main_ctx: &mut MainContext) -> anyhow::Result<SceneContainer> {
    let mut container = SceneContainer::new();
    let node = &main_ctx
        .test_manager
        .as_ref()
        .expect("TestManager must exist in test mode")
        .root
        .clone();
    container.push_all(
        timeout_delay::new(main_ctx, node).context("unable to create TimeoutDelay scene")?,
    );
    container.push_all(Headless::new(main_ctx, node).context("unable to create Headless scene")?);
    main_ctx
        .test_manager
        .as_ref()
        .expect("TestManager must exist in test mode")
        .finish_init();
    Ok(container)
}