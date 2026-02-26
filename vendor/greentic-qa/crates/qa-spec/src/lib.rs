#![allow(missing_docs)]

pub mod answers;
pub mod answers_schema;
pub mod compose;
pub mod computed;
pub mod examples;
pub mod expr;
pub mod frontend;
pub mod i18n;
pub mod progress;
pub mod render;
pub mod runner;
pub mod secrets;
pub mod spec;
pub mod store;
pub mod template;
pub mod validate;
pub mod visibility;

pub use answers::{AnswerSet, Meta, ProgressState, ValidationError, ValidationResult};
pub use answers_schema::generate as answers_schema;
pub use compose::{IncludeError, expand_includes};
pub use computed::{apply_computed_answers, build_expression_context};
pub use examples::generate as example_answers;
pub use expr::Expr;
pub use frontend::{DefaultQaFrontend, QaFrontend};
pub use i18n::{I18nText, ResolvedI18nMap, resolve_i18n_text, resolve_i18n_text_with_locale};
pub use progress::{ProgressContext, next_question};
pub use render::{
    RenderPayload, RenderProgress, RenderQuestion, RenderStatus, build_render_payload,
    build_render_payload_with_i18n, render_card, render_json_ui, render_text,
};
pub use runner::{
    QaPlanV1, execute_plan_effects, normalize_answers, plan_next, plan_submit_all,
    plan_submit_patch,
};
pub use secrets::{SecretAccessResult, SecretAction, evaluate};
pub use spec::{FormSpec, IncludeSpec, QAFlowSpec, QuestionSpec, QuestionType, StepId, StepSpec};
pub use store::{StoreContext, StoreError, StoreOp, StoreTarget};
pub use template::{
    ResolutionMode, TemplateContext, TemplateEngine, TemplateError, register_default_helpers,
};
pub use validate::validate;
pub use visibility::{VisibilityMap, VisibilityMode, resolve_visibility};
