use std::{borrow::Cow, sync::Arc};

use crate::{
    enclose,
    exec::main_ctx::MainContext,
    graphics::context::DrawContext,
    scene::SceneContainer,
    test::tree::ParentTestNode,
    ui::{
        acquire_widget_id,
        event::{UICursorEvent, UIFocusEvent, UIPropagatingEvent},
        utils::geom::{UIRect, UISize},
        EventContext, UISizeConstraint, Widget, WidgetId,
    },
    utils::mutex::Mutex,
};

pub mod stack;

pub fn new(
    main_ctx: &mut MainContext,
    node: &Arc<ParentTestNode>,
) -> anyhow::Result<SceneContainer> {
    let node = node.new_child_parent("ui");
    stack::test(main_ctx, &node)?;
    Ok(SceneContainer::new())
}

type TestWidgetId = usize;

#[allow(clippy::type_complexity)]
pub struct GenericTestWidget<T: Send + Sync> {
    pub canonical_id: WidgetId,
    pub test_id: TestWidgetId,
    pub bounds: Mutex<UIRect>,
    pub layout_callback:
        Box<dyn Fn(&GenericTestWidget<T>, &UISizeConstraint) -> UISize + Send + Sync>,
    pub draw_callback: Box<dyn Fn(&GenericTestWidget<T>, &mut DrawContext) + Send + Sync>,
    pub handle_focus_event_callback:
        Box<dyn Fn(&GenericTestWidget<T>, &mut EventContext, UIFocusEvent) + Send + Sync>,
    pub handle_cursor_event_callback: Box<
        dyn Fn(&GenericTestWidget<T>, &mut EventContext, UICursorEvent) -> Option<UICursorEvent>
            + Send
            + Sync,
    >,
    pub handle_propagating_event_callback: Box<
        dyn Fn(
                &GenericTestWidget<T>,
                &mut EventContext,
                UIPropagatingEvent,
            ) -> Option<UIPropagatingEvent>
            + Send
            + Sync,
    >,
    pub data: T,
}

#[allow(clippy::type_complexity)]
pub struct GenericTestWidgetBuilder<T: Send + Sync> {
    test_id: TestWidgetId,
    data: T,
    layout_callback:
        Option<Box<dyn Fn(&GenericTestWidget<T>, &UISizeConstraint) -> UISize + Send + Sync>>,
    draw_callback: Option<Box<dyn Fn(&GenericTestWidget<T>, &mut DrawContext) + Send + Sync>>,
    handle_focus_event_callback:
        Option<Box<dyn Fn(&GenericTestWidget<T>, &mut EventContext, UIFocusEvent) + Send + Sync>>,
    handle_cursor_event_callback: Option<
        Box<
            dyn Fn(&GenericTestWidget<T>, &mut EventContext, UICursorEvent) -> Option<UICursorEvent>
                + Send
                + Sync,
        >,
    >,
    handle_propagating_event_callback: Option<
        Box<
            dyn Fn(
                    &GenericTestWidget<T>,
                    &mut EventContext,
                    UIPropagatingEvent,
                ) -> Option<UIPropagatingEvent>
                + Send
                + Sync,
        >,
    >,
}

impl<T: Send + Sync> Widget for GenericTestWidget<T> {
    fn id(&self) -> crate::ui::WidgetId {
        self.canonical_id
    }

    fn layout(&self, size_constraints: &UISizeConstraint) -> UISize {
        (self.layout_callback)(self, size_constraints)
    }

    fn set_position(&self, position: crate::ui::utils::geom::UIPos) {
        self.bounds.lock().pos = position;
    }

    fn get_bounds(&self) -> UIRect {
        *self.bounds.lock()
    }

    fn draw(&self, ctx: &mut DrawContext) {
        (self.draw_callback)(self, ctx)
    }

    fn handle_focus_event(&self, ctx: &mut EventContext, event: UIFocusEvent) {
        (self.handle_focus_event_callback)(self, ctx, event)
    }

    fn handle_cursor_event(
        &self,
        ctx: &mut EventContext,
        event: UICursorEvent,
    ) -> Option<UICursorEvent> {
        (self.handle_cursor_event_callback)(self, ctx, event)
    }

    fn handle_propagating_event(
        &self,
        ctx: &mut EventContext,
        event: UIPropagatingEvent,
    ) -> Option<UIPropagatingEvent> {
        (self.handle_propagating_event_callback)(self, ctx, event)
    }
}

impl<T: Send + Sync> GenericTestWidgetBuilder<T> {
    pub fn new(test_id: TestWidgetId, data: T) -> Self {
        Self {
            test_id,
            data,
            handle_propagating_event_callback: None,
            handle_cursor_event_callback: None,
            handle_focus_event_callback: None,
            draw_callback: None,
            layout_callback: None,
        }
    }

    pub fn handle_propagating_event<F>(mut self, callback: F) -> Self
    where
        F: Fn(
                &GenericTestWidget<T>,
                &mut EventContext,
                UIPropagatingEvent,
            ) -> Option<UIPropagatingEvent>
            + Send
            + Sync
            + 'static,
    {
        self.handle_propagating_event_callback = Some(Box::new(callback));
        self
    }

    pub fn handle_cursor_event<F>(mut self, callback: F) -> Self
    where
        F: Fn(&GenericTestWidget<T>, &mut EventContext, UICursorEvent) -> Option<UICursorEvent>
            + Send
            + Sync
            + 'static,
    {
        self.handle_cursor_event_callback = Some(Box::new(callback));
        self
    }

    pub fn handle_focus_event<F>(mut self, callback: F) -> Self
    where
        F: Fn(&GenericTestWidget<T>, &mut EventContext, UIFocusEvent) + Send + Sync + 'static,
    {
        self.handle_focus_event_callback = Some(Box::new(callback));
        self
    }

    pub fn draw<F>(mut self, callback: F) -> Self
    where
        F: Fn(&GenericTestWidget<T>, &mut DrawContext) + Send + Sync + 'static,
    {
        self.draw_callback = Some(Box::new(callback));
        self
    }

    pub fn layout<F>(mut self, callback: F) -> Self
    where
        F: Fn(&GenericTestWidget<T>, &UISizeConstraint) -> UISize + Send + Sync + 'static,
    {
        self.layout_callback = Some(Box::new(callback));
        self
    }

    pub fn build(self) -> Arc<GenericTestWidget<T>> {
        Arc::new(GenericTestWidget {
            test_id: self.test_id,
            canonical_id: acquire_widget_id(),
            data: self.data,
            bounds: Mutex::new(UIRect::ZERO),
            layout_callback: self.layout_callback.expect("layout callback not specified"),
            draw_callback: self.draw_callback.unwrap_or_else(|| Box::new(|_, _| {})),
            handle_focus_event_callback: self
                .handle_focus_event_callback
                .unwrap_or_else(|| Box::new(|_, _, _| {})),
            handle_cursor_event_callback: self
                .handle_cursor_event_callback
                .unwrap_or_else(|| Box::new(|_, _, e| Some(e))),
            handle_propagating_event_callback: self
                .handle_propagating_event_callback
                .unwrap_or_else(|| Box::new(|_, _, e| Some(e))),
        })
    }
}

#[derive(Default)]
pub struct TestWidgetBuilder {
    pref_size: UISize,
    mouse_passthrough: bool,
    consume_propagate: bool,
}

impl TestWidgetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pref_size(mut self, width: f32, height: f32) -> Self {
        self.pref_size = UISize::new(width, height);
        self
    }

    pub fn mouse_passthrough(mut self, passthrough: bool) -> Self {
        self.mouse_passthrough = passthrough;
        self
    }

    pub fn consume_propagate(mut self, consume: bool) -> Self {
        self.consume_propagate = consume;
        self
    }

    #[allow(unused_mut)]
    pub fn build(
        self,
        test_id: TestWidgetId,
        test_log_name: impl Into<Cow<'static, str>>,
        print_focus_event: bool,
        print_propagate_event: bool,
        print_cursor_event: bool,
    ) -> Arc<GenericTestWidget<()>> {
        let Self {
            pref_size,
            mouse_passthrough,
            consume_propagate,
        } = self;
        let test_log_name = test_log_name.into();

        GenericTestWidgetBuilder::new(test_id, ())
            .layout(move |slf, size| {
                let width = pref_size.width.clamp(size.min.width, size.max.width);
                let height = pref_size.height.clamp(size.min.height, size.max.height);
                let size = UISize::new(width, height);
                slf.bounds.lock().size = size;
                size
            })
            .draw(enclose!((test_log_name) move |slf, ctx| {
                let log = ctx.get_test_log(&test_log_name);
                log.push_str(slf.test_id.to_string().as_str());
                log.push('\n');
            }))
            .handle_propagating_event(enclose!((test_log_name) move |slf, ctx, event| {
                let log = ctx.main_ctx.get_test_log(&test_log_name);
                log.push_str("propagating - ");
                if print_propagate_event {
                    log.push_str(format!("{event:?} - ").as_str());
                }
                log.push_str(slf.test_id.to_string().as_str());
                log.push('\n');

                (!consume_propagate).then_some(event)
            }))
            .handle_focus_event(enclose!((test_log_name) move |slf, ctx, event| {
                let log = ctx.main_ctx.get_test_log(&test_log_name);
                log.push_str("focus - ");
                if print_focus_event {
                    log.push_str(format!("{event:?} - ").as_str());
                }
                log.push_str(slf.test_id.to_string().as_str());
                log.push('\n');
            }))
            .handle_cursor_event(enclose!((test_log_name) move |slf, ctx, event| {
                let log = ctx.main_ctx.get_test_log(&test_log_name);
                log.push_str("cursor - ");
                if print_cursor_event {
                    log.push_str(format!("{event:?} - ").as_str());
                }
                log.push_str(slf.test_id.to_string().as_str());
                log.push('\n');

                mouse_passthrough.then_some(event)
            }))
            .build()
    }
}
