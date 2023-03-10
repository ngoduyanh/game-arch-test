use anyhow::Context;

use crate::{exec::main_ctx::MainContext, scene::SceneContainer};

use self::{freq_profile::FreqProfile, update_delay_test::UpdateDelayTest, vsync::VSync};

pub mod close;
pub mod error;
pub mod freq_profile;
pub mod update_delay_test;
pub mod vsync;

pub fn new(main_ctx: &mut MainContext) -> anyhow::Result<SceneContainer> {
    let mut container = SceneContainer::new();
    container.push(VSync::new(main_ctx).context("unable to initialize VSync scene")?);
    container.push(FreqProfile::new());
    container.push(UpdateDelayTest::new());
    container.push_event_handler(close::handle_event);
    container.push_event_handler(error::handle_event);
    Ok(container)
}
