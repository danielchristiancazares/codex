//! Session headers, onboarding guidance, and transcript cards.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::style::StatusTone;
use crate::style::status_style;
use crate::width::display_width;

pub(crate) const SESSION_HEADER_MAX_INNER_WIDTH: usize = 56; // Just an eyeballed value

#[derive(Debug)]
struct TooltipHistoryCell {
    tip: String,
    cwd: PathBuf,
}

impl TooltipHistoryCell {
    fn new(tip: String, cwd: &Path) -> Self {
        Self {
            tip,
            cwd: cwd.to_path_buf(),
        }
    }
}

impl HistoryCell for TooltipHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let indent = "  ";
        let indent_width = display_width(indent);
        let wrap_width = usize::from(width.max(1))
            .saturating_sub(indent_width)
            .max(1);
        let mut lines: Vec<Line<'static>> = Vec::new();
        append_markdown(
            &format!("**Tip:** {}", self.tip),
            Some(wrap_width),
            Some(self.cwd.as_path()),
            &mut lines,
        );

        prefix_lines(lines, indent.into(), indent.into())
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(format!("Tip: {}", self.tip))]
    }
}

#[derive(Debug)]
pub struct SessionInfoCell(CompositeHistoryCell);

impl HistoryCell for SessionInfoCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.0.display_lines(width)
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.0.desired_height(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.0.transcript_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.0.raw_lines()
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "keep local preferences separate while the legacy Config parameter is still required"
)]
pub(crate) fn new_session_info(
    config: &Config,
    local_settings: &crate::local_settings::LocalSettings,
    requested_model: &str,
    session: &ThreadSessionState,
    is_first_event: bool,
    tooltip_override: Option<String>,
    auth_plan: Option<PlanType>,
    show_fast_status: bool,
) -> SessionInfoCell {
    // Header box rendered as history (so it appears at the very top)
    let header = SessionHeaderHistoryCell::new(
        session.model.clone(),
        session.reasoning_effort.clone(),
        show_fast_status,
        config.cwd.to_path_buf(),
        CODEX_CLI_VERSION,
    )
    .with_yolo_mode(has_yolo_permissions(
        session.approval_policy,
        &session.permission_profile,
    ));
    let mut parts: Vec<Box<dyn HistoryCell>> = vec![Box::new(header)];

    if is_first_event {
        let help_lines = vec![Line::from(vec![
            "  Start with a task, or try ".dim(),
            "/init".cyan(),
            ", ".dim(),
            "/model".cyan(),
            ", ".dim(),
            "/permissions".cyan(),
            ".".dim(),
        ])];

        parts.push(Box::new(PlainHistoryCell { lines: help_lines }));
    } else {
        if local_settings.tui.show_tooltips
            && let Some(tooltips) = tooltip_override
                .or_else(|| tooltips::get_tooltip(auth_plan, show_fast_status))
                .map(|tip| TooltipHistoryCell::new(tip, &config.cwd))
        {
            parts.push(Box::new(tooltips));
        }
        if requested_model != session.model.as_str() {
            let lines = vec![
                "model changed:".magenta().bold().into(),
                format!("requested: {requested_model}").into(),
                format!("used: {}", session.model).into(),
            ];
            parts.push(Box::new(PlainHistoryCell { lines }));
        }
    }

    SessionInfoCell(CompositeHistoryCell { parts })
}

pub(crate) fn is_yolo_mode(config: &Config) -> bool {
    has_yolo_permissions(
        AskForApproval::from(config.permissions.approval_policy.value()),
        &config.permissions.effective_permission_profile(),
    )
}

pub(crate) fn has_yolo_permissions(
    approval_policy: AskForApproval,
    permission_profile: &PermissionProfile,
) -> bool {
    approval_policy == AskForApproval::Never
        && matches!(
            permission_profile,
            PermissionProfile::Disabled
                | PermissionProfile::Managed {
                    file_system: ManagedFileSystemPermissions::Unrestricted,
                    network: NetworkSandboxPolicy::Enabled,
                }
        )
}
#[derive(Debug)]
pub(crate) struct SessionHeaderHistoryCell {
    version: &'static str,
    model: String,
    model_style: Style,
    reasoning_effort: Option<ReasoningEffortConfig>,
    show_fast_status: bool,
    directory: PathBuf,
    yolo_mode: bool,
}

impl SessionHeaderHistoryCell {
    pub(crate) fn new(
        model: String,
        reasoning_effort: Option<ReasoningEffortConfig>,
        show_fast_status: bool,
        directory: PathBuf,
        version: &'static str,
    ) -> Self {
        Self::new_with_style(
            model,
            Style::default(),
            reasoning_effort,
            show_fast_status,
            directory,
            version,
        )
    }

    pub(crate) fn new_with_style(
        model: String,
        model_style: Style,
        reasoning_effort: Option<ReasoningEffortConfig>,
        show_fast_status: bool,
        directory: PathBuf,
        version: &'static str,
    ) -> Self {
        Self {
            version,
            model: crate::model_catalog::model_display_name(&model).to_string(),
            model_style,
            reasoning_effort,
            show_fast_status,
            directory,
            yolo_mode: false,
        }
    }

    pub(crate) fn with_yolo_mode(mut self, yolo_mode: bool) -> Self {
        self.yolo_mode = yolo_mode;
        self
    }

    fn format_directory(&self, max_width: Option<usize>) -> String {
        Self::format_directory_inner(&self.directory, max_width)
    }

    pub(crate) fn format_directory_inner(directory: &Path, max_width: Option<usize>) -> String {
        let formatted = if let Some(rel) = relativize_to_home(directory) {
            if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display())
            }
        } else {
            directory.display().to_string()
        };

        if let Some(max_width) = max_width {
            if max_width == 0 {
                return String::new();
            }
            if display_width(formatted.as_str()) > max_width {
                return crate::text_formatting::center_truncate_path(&formatted, max_width);
            }
        }

        formatted
    }

    fn reasoning_label(&self) -> Option<&str> {
        self.reasoning_effort
            .as_ref()
            .map(ReasoningEffortConfig::as_str)
    }
}

impl HistoryCell for SessionHeaderHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let content_width = usize::from(width).min(SESSION_HEADER_MAX_INNER_WIDTH + 4);

        let make_row = |spans: Vec<Span<'static>>| Line::from(spans);

        let title_spans: Vec<Span<'static>> = vec![
            Span::styled("  >_ ", crate::style::accent_style()),
            Span::from("Codex").bold(),
            Span::styled(
                format!(" v{}", self.version),
                crate::style::secondary_style(),
            ),
        ];

        const CHANGE_MODEL_HINT_COMMAND: &str = "/model";
        const CHANGE_MODEL_HINT_EXPLANATION: &str = " to change";
        let reasoning_label = self.reasoning_label();
        let model_spans: Vec<Span<'static>> = {
            let mut spans = vec![
                "  ".into(),
                Span::styled(crate::text_formatting::format_model_status_label(&self.model), self.model_style),
            ];
            if let Some(reasoning) = reasoning_label {
                spans.push(Span::from(" "));
                spans.push(Span::from(reasoning.to_owned()));
            }
            if self.show_fast_status {
                spans.push(" · ".dim());
                spans.push(Span::styled("fast", self.model_style.magenta()));
            }
            spans.push("  ".into());
            spans.push(Span::styled(
                CHANGE_MODEL_HINT_COMMAND,
                crate::style::key_hint_style(),
            ));
            spans.push(Span::styled(
                CHANGE_MODEL_HINT_EXPLANATION,
                crate::style::secondary_style(),
            ));
            spans
        };

        let dir_max_width = content_width.saturating_sub(2);
        let dir = self.format_directory(Some(dir_max_width));
        let dir_spans = vec!["  ".into(), Span::from(dir)];

        let access_spans = if self.yolo_mode {
            vec![
                "  ".into(),
                Span::styled(
                    "[!] Unrestricted access",
                    status_style(StatusTone::Attention),
                ),
                "  ".into(),
                Span::styled("/permissions", crate::style::key_hint_style()),
                Span::styled(" to change", crate::style::secondary_style()),
            ]
        } else {
            vec![
                "  Guarded access".into(),
                "  ".into(),
                Span::styled("/permissions", crate::style::key_hint_style()),
                Span::styled(" to review", crate::style::secondary_style()),
            ]
        };

        vec![
            make_row(title_spans),
            make_row(model_spans),
            make_row(dir_spans),
            make_row(access_spans),
        ]
        .into_iter()
        .map(|line| truncate_line_with_ellipsis_if_overflow(line, content_width))
        .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(format!("OpenAI Codex (v{})", self.version)),
            Line::from(format!(
                "model: {}{}",
                self.model,
                self.reasoning_label()
                    .map(|reasoning| format!(" {reasoning}"))
                    .unwrap_or_default()
            )),
            Line::from(format!(
                "directory: {}",
                self.format_directory(/*max_width*/ None)
            )),
        ];
        if self.yolo_mode {
            lines.push(Line::from("permissions: [!] unrestricted access"));
        }
        lines
    }
}
