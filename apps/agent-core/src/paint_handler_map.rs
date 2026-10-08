//! Handler adapters for the Paint T3.1 tools.

use std::collections::BTreeMap;
use std::sync::Arc;

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_tool_bus::{CallContext, ToolBusError, ToolHandler, ToolOutput};
use serde_json::{Map, Value};

use super::{
    PaintHandlerContext, TOOL_CANVAS_CAPTURE_PIXELS, TOOL_CANVAS_DRAW_RECTANGLE,
    TOOL_CANVAS_RESOLVE_POINT, TOOL_COLOR_SELECT_FOREGROUND, TOOL_DOCUMENT_NEW, TOOL_LAYER_SELECT,
    TOOL_TOOL_SELECT,
};

pub fn build_handler_map<P>(
    context: &Arc<PaintHandlerContext<P>>,
) -> BTreeMap<String, Arc<dyn ToolHandler>>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    let mut handlers: BTreeMap<String, Arc<dyn ToolHandler>> = BTreeMap::new();
    handlers.insert(
        TOOL_DOCUMENT_NEW.to_owned(),
        Arc::new(DocumentNewHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_TOOL_SELECT.to_owned(),
        Arc::new(ToolSelectHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_COLOR_SELECT_FOREGROUND.to_owned(),
        Arc::new(ColorSelectHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_LAYER_SELECT.to_owned(),
        Arc::new(LayerSelectHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_CANVAS_RESOLVE_POINT.to_owned(),
        Arc::new(ResolvePointHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_CANVAS_DRAW_RECTANGLE.to_owned(),
        Arc::new(DrawRectangleHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_CANVAS_CAPTURE_PIXELS.to_owned(),
        Arc::new(CapturePixelsHandler {
            context: Arc::clone(context),
        }),
    );
    handlers
}

struct DocumentNewHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for DocumentNewHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.document_new_output(call.task_id(), arguments)
    }
}

struct ToolSelectHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for ToolSelectHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.select_tool_output(arguments)
    }
}

struct ColorSelectHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for ColorSelectHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.select_foreground_color_output(arguments)
    }
}

struct LayerSelectHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for LayerSelectHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.select_layer_output(arguments)
    }
}

struct ResolvePointHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for ResolvePointHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.resolve_point_output(arguments)
    }
}

struct DrawRectangleHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for DrawRectangleHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context
            .draw_rectangle_output(call.task_id(), arguments)
    }
}

struct CapturePixelsHandler<P> {
    context: Arc<PaintHandlerContext<P>>,
}

impl<P> ToolHandler for CapturePixelsHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.capture_pixels_output()
    }
}
