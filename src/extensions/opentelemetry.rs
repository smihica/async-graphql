use std::collections::HashMap;
use std::sync::Arc;

use async_graphql_parser::types::ExecutableDocument;
use async_graphql_value::Variables;
use opentelemetry::trace::{SpanKind, TraceContextExt, Tracer};
use opentelemetry::{Context as OpenTelemetryContext, Key};

use crate::extensions::{Extension, ExtensionContext, ExtensionFactory, ResolveInfo};
use crate::{ServerError, ValidationResult};

const REQUEST_CTX: usize = 0;
const PARSE_CTX: usize = 1;
const VALIDATION_CTX: usize = 2;
const EXECUTE_CTX: usize = 3;

#[inline]
fn resolve_ctx_id(resolver_id: usize) -> usize {
    resolver_id + 10
}

const KEY_SOURCE: Key = Key::from_static_str("graphql.source");
const KEY_VARIABLES: Key = Key::from_static_str("graphql.variables");
const KEY_PARENT_TYPE: Key = Key::from_static_str("graphql.parentType");
const KEY_RETURN_TYPE: Key = Key::from_static_str("graphql.returnType");
const KEY_RESOLVE_ID: Key = Key::from_static_str("graphql.resolveId");
const KEY_ERROR: Key = Key::from_static_str("graphql.error");
const KEY_COMPLEXITY: Key = Key::from_static_str("graphql.complexity");
const KEY_DEPTH: Key = Key::from_static_str("graphql.depth");

/// OpenTelemetry extension configuration for each request.
#[derive(Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub struct OpenTelemetryConfig {
    /// Use a context as the parent node of the entire query.
    parent: spin::Mutex<Option<OpenTelemetryContext>>,
}

impl OpenTelemetryConfig {
    /// Use a context as the parent of the entire query.
    pub fn parent_context(mut self, cx: OpenTelemetryContext) -> Self {
        *self.parent.get_mut() = Some(cx);
        self
    }
}

/// OpenTelemetry extension
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub struct OpenTelemetry<T> {
    tracer: Arc<T>,
}

impl<T> OpenTelemetry<T> {
    /// Use `tracer` to create an OpenTelemetry extension.
    pub fn new(tracer: T) -> OpenTelemetry<T>
    where
        T: Tracer + Send + Sync,
    {
        Self {
            tracer: Arc::new(tracer),
        }
    }
}

impl<T: Tracer + Send + Sync> ExtensionFactory for OpenTelemetry<T> {
    fn create(&self) -> Box<dyn Extension> {
        Box::new(OpenTelemetryExtension {
            tracer: self.tracer.clone(),
            contexts: Default::default(),
        })
    }
}

struct OpenTelemetryExtension<T> {
    tracer: Arc<T>,
    contexts: HashMap<usize, OpenTelemetryContext>,
}

impl<T> OpenTelemetryExtension<T> {
    fn enter_context(&mut self, id: usize, cx: OpenTelemetryContext) {
        let _ = cx.clone().attach();
        self.contexts.insert(id, cx);
    }

    fn exit_context(&mut self, id: usize) -> Option<OpenTelemetryContext> {
        if let Some(cx) = self.contexts.remove(&id) {
            let _ = cx.clone().attach();
            Some(cx)
        } else {
            None
        }
    }
}

impl<T: Tracer + Send + Sync> Extension for OpenTelemetryExtension<T> {
    fn start(&mut self, ctx: &ExtensionContext<'_>) {
        let request_cx = ctx
            .data_opt::<OpenTelemetryConfig>()
            .and_then(|cfg| cfg.parent.lock().take())
            .unwrap_or_else(|| {
                OpenTelemetryContext::current_with_span(
                    self.tracer
                        .span_builder("request")
                        .with_kind(SpanKind::Server)
                        .start(&*self.tracer),
                )
            });
        self.enter_context(REQUEST_CTX, request_cx);
    }

    fn end(&mut self, _ctx: &ExtensionContext<'_>) {
        self.exit_context(REQUEST_CTX);
    }

    fn parse_start(
        &mut self,
        _ctx: &ExtensionContext<'_>,
        query_source: &str,
        variables: &Variables,
    ) {
        if let Some(parent_cx) = self.contexts.get(&REQUEST_CTX).cloned() {
            let mut attributes = Vec::with_capacity(2);
            attributes.push(KEY_SOURCE.string(query_source.to_string()));
            attributes.push(KEY_VARIABLES.string(serde_json::to_string(variables).unwrap()));
            let parse_span = self
                .tracer
                .span_builder("parse")
                .with_kind(SpanKind::Server)
                .with_attributes(attributes)
                .with_parent_context(parent_cx)
                .start(&*self.tracer);
            let parse_cx = OpenTelemetryContext::current_with_span(parse_span);
            self.enter_context(PARSE_CTX, parse_cx);
        }
    }

    fn parse_end(&mut self, _ctx: &ExtensionContext<'_>, _document: &ExecutableDocument) {
        self.exit_context(PARSE_CTX);
    }

    fn validation_start(&mut self, _ctx: &ExtensionContext<'_>) {
        if let Some(parent_cx) = self.contexts.get(&REQUEST_CTX).cloned() {
            let span = self
                .tracer
                .span_builder("validation")
                .with_kind(SpanKind::Server)
                .with_parent_context(parent_cx)
                .start(&*self.tracer);
            let validation_cx = OpenTelemetryContext::current_with_span(span);
            self.enter_context(VALIDATION_CTX, validation_cx);
        }
    }

    fn validation_end(&mut self, _ctx: &ExtensionContext<'_>, result: &ValidationResult) {
        if let Some(validation_cx) = self.exit_context(VALIDATION_CTX) {
            let span = validation_cx.span();
            span.set_attribute(KEY_COMPLEXITY.i64(result.complexity as i64));
            span.set_attribute(KEY_DEPTH.i64(result.depth as i64));
        }
    }

    fn execution_start(&mut self, _ctx: &ExtensionContext<'_>) {
        let span = match self.contexts.get(&REQUEST_CTX).cloned() {
            Some(parent_cx) => self
                .tracer
                .span_builder("execute")
                .with_kind(SpanKind::Server)
                .with_parent_context(parent_cx)
                .start(&*self.tracer),
            None => self
                .tracer
                .span_builder("execute")
                .with_kind(SpanKind::Server)
                .start(&*self.tracer),
        };
        let execute_cx = OpenTelemetryContext::current_with_span(span);
        self.enter_context(EXECUTE_CTX, execute_cx);
    }

    fn execution_end(&mut self, _ctx: &ExtensionContext<'_>) {
        self.exit_context(EXECUTE_CTX);
    }

    fn resolve_start(&mut self, _ctx: &ExtensionContext<'_>, info: &ResolveInfo<'_>) {
        let parent_cx = match info.resolve_id.parent {
            Some(parent_id) if parent_id > 0 => self.contexts.get(&resolve_ctx_id(parent_id)),
            _ => self.contexts.get(&EXECUTE_CTX),
        }
        .cloned();

        if let Some(parent_cx) = parent_cx {
            let mut attributes = Vec::with_capacity(3);
            attributes.push(KEY_RESOLVE_ID.i64(info.resolve_id.current as i64));
            attributes.push(KEY_PARENT_TYPE.string(info.parent_type.to_string()));
            attributes.push(KEY_RETURN_TYPE.string(info.return_type.to_string()));
            let span = self
                .tracer
                .span_builder(&info.path_node.to_string())
                .with_kind(SpanKind::Server)
                .with_parent_context(parent_cx)
                .with_attributes(attributes)
                .start(&*self.tracer);
            let resolve_cx = OpenTelemetryContext::current_with_span(span);
            self.enter_context(resolve_ctx_id(info.resolve_id.current), resolve_cx);
        }
    }

    fn resolve_end(&mut self, _ctx: &ExtensionContext<'_>, info: &ResolveInfo<'_>) {
        self.exit_context(resolve_ctx_id(info.resolve_id.current));
    }

    fn error(&mut self, _ctx: &ExtensionContext<'_>, err: &ServerError) {
        if let Some(parent_cx) = self.contexts.get(&EXECUTE_CTX).cloned() {
            parent_cx
                .span()
                .add_event("error".to_string(), vec![KEY_ERROR.string(err.to_string())]);
        }
    }
}
