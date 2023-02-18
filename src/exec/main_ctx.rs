use std::{
    borrow::Cow,
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{bail, Context};
use tracing_appender::non_blocking::WorkerGuard;
use winit::{
    event::Event,
    event_loop::{EventLoop, EventLoopProxy},
};

use crate::{
    display::Display,
    events::{GameEvent, GameUserEvent},
    graphics::{context::DrawContext, wrappers::vertex_array::VertexArrayHandle},
    scene::main::RootScene,
    test::TestManager,
    ui::{EventContext, Widget},
    utils::{args::args, error::ResultExt},
};

use super::{
    dispatch::{DispatchId, DispatchList},
    executor::GameServerExecutor,
    server::{draw, GameServerChannel, GameServerSendChannel, ServerChannels},
    task::{CancellationToken, TaskExecutor},
};

pub struct MainContext {
    pub focused_widget: Option<Arc<dyn Widget>>,
    pub prev_focused_widget: Option<Arc<dyn Widget>>,
    pub test_logs: HashMap<Cow<'static, str>, String>,
    pub test_manager: Option<Arc<TestManager>>,
    pub executor: GameServerExecutor,
    pub dummy_vao: VertexArrayHandle,
    pub task_executor: TaskExecutor,
    pub channels: ServerChannels,
    pub dispatch_list: DispatchList,
    pub event_loop_proxy: EventLoopProxy<GameUserEvent>,
    pub display: Display,
}

impl MainContext {
    pub fn new(
        executor: GameServerExecutor,
        display: Display,
        event_loop_proxy: EventLoopProxy<GameUserEvent>,
        mut channels: ServerChannels,
    ) -> anyhow::Result<Self> {
        let mut slf = Self {
            executor,
            test_manager: args()
                .test
                .then(|| TestManager::new(event_loop_proxy.clone())),
            dummy_vao: VertexArrayHandle::new(&mut channels.draw, "dummy vertex array")?,
            task_executor: TaskExecutor::new(),
            display,
            event_loop_proxy,
            dispatch_list: DispatchList::new(),
            channels,
            test_logs: HashMap::new(),
            prev_focused_widget: None,
            focused_widget: None,
        };

        if let Some(test_manager) = slf.test_manager.as_ref() {
            let test_manager = test_manager.clone();
            slf.set_timeout(Duration::from_secs(30), move |_, _, _| {
                test_manager.set_timeout_func();
                Ok(())
            })
            .context("unable to set test timeout")?;
        }

        Ok(slf)
    }

    pub fn set_focus_widget(&mut self, new_widget: Option<Arc<dyn Widget>>) {
        if self.focused_widget.is_some() {
            tracing::warn!("two widgets tried to be focused in one mouse press event");
            return;
        }

        self.focused_widget = new_widget;
        if self.prev_focused_widget.as_ref().map(|w| w.id())
            == self.focused_widget.as_ref().map(|w| w.id())
        {
            return;
        }

        if let Some(widget) = self.prev_focused_widget.take() {
            widget.focus_changed(&mut EventContext { main_ctx: self }, false);
        }

        if let Some(widget) = self.focused_widget.clone() {
            widget.focus_changed(&mut EventContext { main_ctx: self }, true);
        }
    }

    pub fn get_test_log(&mut self, name: &str) -> &mut String {
        if !self.test_logs.contains_key(name) {
            self.test_logs
                .insert(Cow::Owned(name.to_owned()), String::new());
        }

        self.test_logs.get_mut(name).unwrap()
    }

    pub fn pop_test_log(&mut self, name: &str) -> String {
        self.test_logs.remove(name).unwrap_or_default()
    }

    pub fn handle_event(
        &mut self,
        root_scene: &mut RootScene,
        event: GameEvent,
    ) -> anyhow::Result<()> {
        match event {
            Event::UserEvent(GameUserEvent::Dispatch(msg)) => {
                for (id, d) in self.dispatch_list.handle_dispatch_msg(msg).into_iter() {
                    d(self, root_scene, id)?;
                }
            }

            Event::UserEvent(GameUserEvent::Execute(callback)) => {
                callback(self, root_scene).log_error();
            }

            Event::UserEvent(GameUserEvent::Error(e)) => {
                tracing::error!("GameUserEvent::Error caught: {}", e);
            }

            event => {
                root_scene.handle_event(self, event);
            }
        };
        Ok(())
    }

    pub fn set_timeout<F>(
        &mut self,
        timeout: Duration,
        callback: F,
    ) -> anyhow::Result<(DispatchId, CancellationToken)>
    where
        F: FnOnce(&mut MainContext, &mut RootScene, DispatchId) -> anyhow::Result<()> + 'static,
    {
        let cancel_token = CancellationToken::new();
        let id = self.dispatch_list.push(callback, cancel_token.clone());
        self.channels.update.set_timeout(timeout, id)?;
        Ok((id, cancel_token))
    }

    pub fn execute_blocking_task<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.task_executor.execute(f)
    }

    #[allow(irrefutable_let_patterns)]
    pub fn execute_draw_sync<F, R>(&mut self, callback: F) -> anyhow::Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut DrawContext, &mut Option<RootScene>) -> anyhow::Result<R> + Send + 'static,
    {
        if let Some(server) = self.executor.main_runner.base.container.draw.as_mut() {
            callback(&mut server.context, &mut server.root_scene)
        } else {
            self.channels
                .draw
                .send(draw::RecvMsg::ExecuteSync(Box::new(
                    move |context, root_scene| Box::new(callback(context, root_scene)),
                )))?;
            if let draw::SendMsg::ExecuteSyncReturn(result) = self.channels.draw.recv()? {
                Ok(result
                    .downcast::<anyhow::Result<R>>()
                    .map(|bx| *bx)
                    .map_err(|_| {
                        anyhow::format_err!("unable to downcast callback return value")
                    })??)
            } else {
                bail!("unexpected response message from thread");
            }
        }
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<GameUserEvent>,
        mut root_scene: RootScene,
        guard: Option<WorkerGuard>,
    ) -> ! {
        use winit::event_loop::ControlFlow;
        event_loop.run(move |event, _target, control_flow| {
            // guarantee drop order
            fn unused<T>(_: &T) {}
            unused(&root_scene);
            unused(&self);
            unused(&guard);
            match event {
                Event::MainEventsCleared => {
                    self.executor
                        .main_runner
                        .base
                        .run_single(true)
                        .expect("error running main runner");
                }

                Event::UserEvent(GameUserEvent::Exit(code)) => {
                    control_flow.set_exit_with_code(code)
                }

                event => self
                    .handle_event(&mut root_scene, event)
                    .expect("error handling events"),
            }

            match *control_flow {
                ControlFlow::ExitWithCode(_) => {
                    self.executor.stop();
                }

                _ => {
                    *control_flow = if self.executor.main_runner.base.container.does_run() {
                        ControlFlow::Poll
                    } else {
                        ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100))
                    }
                }
            };
        })
    }
}
