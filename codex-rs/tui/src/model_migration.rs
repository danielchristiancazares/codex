use crate::key_hint;
use crate::markdown_render::render_markdown_text_with_width;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::prelude::Stylize as _;
use ratatui::prelude::Widget;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

use self::layout::ChoiceVisibility;
use self::layout::ModelMigrationLayout;

/// Outcome of the migration prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelMigrationOutcome {
    Accepted,
    Rejected,
    Exit,
}

#[derive(Clone)]
pub(crate) struct ModelMigrationCopy {
    pub heading: Vec<Span<'static>>,
    pub content: Vec<Line<'static>>,
    pub can_opt_out: bool,
    pub markdown: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationMenuOption {
    TryNewModel,
    UseExistingModel,
}

impl MigrationMenuOption {
    fn all() -> [Self; 2] {
        [Self::TryNewModel, Self::UseExistingModel]
    }

    fn label(self) -> &'static str {
        match self {
            Self::TryNewModel => "Try new model",
            Self::UseExistingModel => "Use existing model",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn migration_copy_for_models(
    current_model: &str,
    target_model: &str,
    model_link: Option<String>,
    migration_copy: Option<String>,
    migration_markdown: Option<String>,
    target_display_name: String,
    target_description: Option<String>,
    can_opt_out: bool,
) -> ModelMigrationCopy {
    if let Some(migration_markdown) = migration_markdown {
        return ModelMigrationCopy {
            heading: Vec::new(),
            content: Vec::new(),
            can_opt_out,
            markdown: Some(fill_migration_markdown(
                &migration_markdown,
                current_model,
                target_model,
            )),
        };
    }

    let heading_text = Span::from(format!(
        "Codex just got an upgrade. Introducing {target_display_name}."
    ))
    .bold();
    let description_line: Line<'static>;
    if let Some(migration_copy) = &migration_copy {
        description_line = Line::from(migration_copy.clone());
    } else {
        description_line = target_description
            .filter(|desc| !desc.is_empty())
            .map(Line::from)
            .unwrap_or_else(|| {
                Line::from(format!(
                    "{target_display_name} is recommended for better performance and reliability."
                ))
            });
    }

    let mut content = vec![];
    if migration_copy.is_none() {
        content.push(Line::from(format!(
            "We recommend switching from {current_model} to {target_model}."
        )));
        content.push(Line::from(""));
    }

    if let Some(model_link) = model_link {
        content.push(Line::from(vec![
            format!("{description_line} Learn more about {target_display_name} at ").into(),
            model_link.cyan().underlined(),
        ]));
        content.push(Line::from(""));
    } else {
        content.push(description_line);
        content.push(Line::from(""));
    }

    if can_opt_out {
        content.push(Line::from(format!(
            "You can continue using {current_model} if you prefer."
        )));
    } else {
        content.push(Line::from("Press enter to continue".dim()));
    }

    ModelMigrationCopy {
        heading: vec![heading_text],
        content,
        can_opt_out,
        markdown: None,
    }
}

pub(crate) async fn run_model_migration_prompt(
    tui: &mut Tui,
    copy: ModelMigrationCopy,
) -> std::io::Result<ModelMigrationOutcome> {
    let alt = AltScreenGuard::enter(tui);
    let mut screen = ModelMigrationScreen::new(alt.tui.frame_requester(), copy);

    alt.tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    })?;

    alt.tui.discard_pending_input_before_interactive_screen()?;
    let events = alt.tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            let _ = alt.tui.screen_size_for_event(&event);
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) | TuiEvent::FocusLost => {}
                TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                    let _ = alt.tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    });
                }
            }
        } else {
            screen.accept();
            break;
        }
    }

    Ok(screen.outcome())
}

struct ModelMigrationScreen {
    request_frame: FrameRequester,
    copy: ModelMigrationCopy,
    done: bool,
    outcome: ModelMigrationOutcome,
    highlighted_option: MigrationMenuOption,
    menu_visibility: Cell<ChoiceVisibility>,
}

impl ModelMigrationScreen {
    fn new(request_frame: FrameRequester, copy: ModelMigrationCopy) -> Self {
        Self {
            request_frame,
            copy,
            done: false,
            outcome: ModelMigrationOutcome::Accepted,
            highlighted_option: MigrationMenuOption::TryNewModel,
            menu_visibility: Cell::new(ChoiceVisibility::Hidden),
        }
    }

    fn finish_with(&mut self, outcome: ModelMigrationOutcome) {
        self.outcome = outcome;
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn accept(&mut self) {
        self.finish_with(ModelMigrationOutcome::Accepted);
    }

    fn reject(&mut self) {
        self.finish_with(ModelMigrationOutcome::Rejected);
    }

    fn exit(&mut self) {
        self.finish_with(ModelMigrationOutcome::Exit);
    }

    fn confirm_selection(&mut self) {
        if self.copy.can_opt_out {
            match self.highlighted_option {
                MigrationMenuOption::TryNewModel => self.accept(),
                MigrationMenuOption::UseExistingModel => self.reject(),
            }
        } else {
            self.accept();
        }
    }

    fn highlight_option(&mut self, option: MigrationMenuOption) {
        if self.highlighted_option != option {
            self.highlighted_option = option;
            self.request_frame.schedule_frame();
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if is_ctrl_exit_combo(key_event) {
            self.exit();
            return;
        }

        if self.copy.can_opt_out {
            if self.menu_visibility.get() == ChoiceVisibility::Hidden {
                return;
            }
            self.handle_menu_key(key_event.code);
        } else if matches!(key_event.code, KeyCode::Esc | KeyCode::Enter) {
            self.accept();
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn outcome(&self) -> ModelMigrationOutcome {
        self.outcome
    }
}

impl WidgetRef for &ModelMigrationScreen {
    fn render_ref(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        Clear.render(area, buf);

        let copy = self.copy_column(area.width);
        if !self.copy.can_opt_out {
            self.menu_visibility.set(ChoiceVisibility::Visible);
            copy.render(area, buf);
            return;
        }

        let layout = ModelMigrationLayout::new(area, copy.desired_height(area.width));
        self.menu_visibility.set(layout.choice_visibility());
        copy.render(layout.copy_area, buf);
        self.render_menu(layout, buf);
    }
}

impl ModelMigrationScreen {
    fn handle_menu_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlight_option(MigrationMenuOption::TryNewModel);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlight_option(MigrationMenuOption::UseExistingModel);
            }
            KeyCode::Char('1') => {
                self.highlight_option(MigrationMenuOption::TryNewModel);
                self.accept();
            }
            KeyCode::Char('2') => {
                self.highlight_option(MigrationMenuOption::UseExistingModel);
                self.reject();
            }
            KeyCode::Enter | KeyCode::Esc => self.confirm_selection(),
            _ => {}
        }
    }

    fn heading_line(&self) -> Line<'static> {
        let mut heading = vec![Span::raw("> ")];
        heading.extend(self.copy.heading.iter().cloned());
        Line::from(heading)
    }

    fn render_content(&self, column: &mut ColumnRenderable) {
        self.render_lines(&self.copy.content, column);
    }

    fn render_lines(&self, lines: &[Line<'static>], column: &mut ColumnRenderable) {
        for line in lines {
            column.push(
                Paragraph::new(line.clone())
                    .wrap(Wrap { trim: false })
                    .inset(Insets::tlbr(
                        /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
                    )),
            );
        }
    }

    fn render_markdown_content(
        &self,
        markdown: &str,
        area_width: u16,
        column: &mut ColumnRenderable,
    ) {
        let horizontal_inset = 2;
        let content_width = area_width.saturating_sub(horizontal_inset);
        let wrap_width = (content_width > 0).then_some(content_width as usize);
        let rendered = render_markdown_text_with_width(markdown, wrap_width);
        for line in rendered.lines {
            column.push(
                Paragraph::new(line)
                    .wrap(Wrap { trim: false })
                    .inset(Insets::tlbr(
                        /*top*/ 0,
                        horizontal_inset,
                        /*bottom*/ 0,
                        /*right*/ 0,
                    )),
            );
        }
    }

    fn copy_column(&self, width: u16) -> ColumnRenderable<'static> {
        let mut column = ColumnRenderable::new();
        column.push("");
        if let Some(markdown) = self.copy.markdown.as_ref() {
            self.render_markdown_content(markdown, width, &mut column);
        } else {
            column.push(self.heading_line());
            column.push(Line::from(""));
            self.render_content(&mut column);
        }
        column
    }

    fn render_menu(&self, layout: ModelMigrationLayout, buf: &mut ratatui::buffer::Buffer) {
        if let Some(area) = layout.compact_options_area {
            let spans = MigrationMenuOption::all()
                .into_iter()
                .enumerate()
                .flat_map(|(idx, option)| {
                    let separator = (idx > 0).then(|| Span::from("   "));
                    let label = match option {
                        MigrationMenuOption::TryNewModel => "1 Try new",
                        MigrationMenuOption::UseExistingModel => "2 Use existing",
                    };
                    let option = if self.highlighted_option == option {
                        Span::from(label).cyan().bold()
                    } else {
                        Span::from(label)
                    };
                    separator.into_iter().chain(std::iter::once(option))
                })
                .collect::<Vec<_>>();
            Line::from(spans).render(area, buf);
            return;
        }

        if let Some(area) = layout.instruction_area {
            Paragraph::new("Choose how you'd like Codex to proceed.")
                .wrap(Wrap { trim: false })
                .render(left_inset(area), buf);
        }
        for (idx, (option, area)) in MigrationMenuOption::all()
            .into_iter()
            .zip(layout.option_areas)
            .enumerate()
        {
            if let Some(area) = area {
                selection_option_row(
                    idx,
                    option.label().to_string(),
                    self.highlighted_option == option,
                )
                .render(area, buf);
            }
        }
        if let Some(area) = layout.guidance_area {
            let guidance = if layout.full_guidance {
                Line::from(vec![
                    "Use ".dim(),
                    key_hint::plain(KeyCode::Up).into(),
                    "/".dim(),
                    key_hint::plain(KeyCode::Down).into(),
                    " to move, press ".dim(),
                    key_hint::plain(KeyCode::Enter).into(),
                    " to confirm".dim(),
                ])
            } else {
                Line::from(vec![
                    key_hint::plain(KeyCode::Enter).into(),
                    " confirm".dim(),
                ])
            };
            guidance.render(left_inset(area), buf);
        }
    }
}

fn left_inset(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    ratatui::layout::Rect::new(
        area.x.saturating_add(2),
        area.y,
        area.width.saturating_sub(2),
        area.height,
    )
}

// Render the prompt on the terminal's alternate screen so exiting or cancelling
// does not leave a large blank region in the normal scrollback. This does not
// change the prompt's appearance – only where it is drawn.
struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}

fn is_ctrl_exit_combo(key_event: KeyEvent) -> bool {
    key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
}

fn fill_migration_markdown(template: &str, current_model: &str, target_model: &str) -> String {
    template
        .replace("{model_from}", current_model)
        .replace("{model_to}", target_model)
}

#[cfg(test)]
mod tests {
    use super::ModelMigrationCopy;
    use super::ModelMigrationScreen;
    use super::migration_copy_for_models;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::WidgetRef;

    fn render_screen(screen: &ModelMigrationScreen, width: u16, height: u16) -> String {
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(screen, frame.area());
        }
        terminal.flush().expect("flush");
        terminal.backend().to_string()
    }

    #[test]
    fn prompt_snapshot() {
        let width: u16 = 60;
        let height: u16 = 28;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5.1-codex-mini",
                "gpt-5.1-codex-max",
                /*model_link*/ None,
                Some(
                    "Upgrade to gpt-5.2-codex for the latest and greatest agentic coding model."
                        .to_string(),
                ),
                /*migration_markdown*/ None,
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                /*can_opt_out*/ true,
            ),
        );

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!("model_migration_prompt", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_family() {
        let backend = VT100Backend::new(/*width*/ 65, /*height*/ 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 65, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5",
                "gpt-5.1",
                Some("https://www.codex.com/models/gpt-5.1".to_string()),
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-5.1".to_string(),
                Some("Broad world knowledge with strong general reasoning.".to_string()),
                /*can_opt_out*/ false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_family", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex() {
        let backend = VT100Backend::new(/*width*/ 60, /*height*/ 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex",
                "gpt-5.1-codex-max",
                Some("https://www.codex.com/models/gpt-5.1-codex-max".to_string()),
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                /*can_opt_out*/ false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex_mini() {
        let backend = VT100Backend::new(/*width*/ 60, /*height*/ 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex-mini",
                "gpt-5.1-codex-mini",
                Some("https://www.codex.com/models/gpt-5.1-codex-mini".to_string()),
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-5.1-codex-mini".to_string(),
                Some("Optimized for codex. Cheaper, faster, but less capable.".to_string()),
                /*can_opt_out*/ false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex_mini", terminal.backend());
    }

    #[test]
    fn escape_key_accepts_prompt() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                Some("https://www.codex.com/models/gpt-new".to_string()),
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                /*can_opt_out*/ true,
            ),
        );

        let _ = render_screen(&screen, /*width*/ 60, /*height*/ 10);
        // Simulate pressing Escape
        screen.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(screen.is_done());
        // Esc should not be treated as Exit – it accepts like Enter.
        assert!(matches!(
            screen.outcome(),
            super::ModelMigrationOutcome::Accepted
        ));
    }

    #[test]
    fn selecting_use_existing_model_rejects_upgrade() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                Some("https://www.codex.com/models/gpt-new".to_string()),
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                /*can_opt_out*/ true,
            ),
        );

        let _ = render_screen(&screen, /*width*/ 60, /*height*/ 10);
        screen.handle_key(KeyEvent::new(
            KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
        screen.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(screen.is_done());
        assert!(matches!(
            screen.outcome(),
            super::ModelMigrationOutcome::Rejected
        ));
    }

    #[test]
    fn markdown_prompt_keeps_long_url_tail_visible_when_narrow() {
        let long_url = "https://example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/with/a/very/long/path/tail42";
        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            ModelMigrationCopy {
                heading: Vec::new(),
                content: Vec::new(),
                can_opt_out: false,
                markdown: Some(long_url.to_string()),
            },
        );

        let backend = VT100Backend::new(/*width*/ 40, /*height*/ 16);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 40, 16));

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        let rendered = terminal.backend().to_string();
        assert!(
            rendered.contains("tail42"),
            "expected wrapped markdown URL tail to remain visible, got:\n{rendered}"
        );
    }

    #[test]
    fn hidden_menu_ignores_confirmation_keys_but_ctrl_c_exits() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                /*model_link*/ None,
                /*migration_copy*/ None,
                /*migration_markdown*/ None,
                "gpt-new".to_string(),
                /*target_description*/ None,
                /*can_opt_out*/ true,
            ),
        );
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 0,
        );
        let mut buffer = Buffer::empty(area);
        (&screen).render_ref(area, &mut buffer);

        for code in [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('1'),
            KeyCode::Char('2'),
        ] {
            screen.handle_key(KeyEvent::new(code, crossterm::event::KeyModifiers::NONE));
            assert!(!screen.is_done());
        }

        screen.handle_key(KeyEvent::new(
            KeyCode::Char('c'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        assert!(screen.is_done());
        assert_eq!(screen.outcome(), super::ModelMigrationOutcome::Exit);
    }

    #[test]
    fn constrained_opt_out_menu_keeps_both_choices_visible() {
        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                /*model_link*/ None,
                Some("A deliberately long migration explanation that must yield to both actionable outcomes even in a short terminal.".to_string()),
                /*migration_markdown*/ None,
                "gpt-new".to_string(),
                /*target_description*/ None,
                /*can_opt_out*/ true,
            ),
        );

        let rendered = render_screen(&screen, /*width*/ 60, /*height*/ 10);

        assert!(rendered.contains("Try new model"), "{rendered}");
        assert!(rendered.contains("Use existing model"), "{rendered}");
        assert_snapshot!("model_migration_constrained_opt_out", rendered);
    }

    #[test]
    fn long_markdown_cannot_displace_opt_out_menu() {
        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            ModelMigrationCopy {
                heading: Vec::new(),
                content: Vec::new(),
                can_opt_out: true,
                markdown: Some(
                    "Long migration copy ".repeat(40)
                        + "\n\nAdditional details that should be clipped before the choices.",
                ),
            },
        );

        let rendered = render_screen(&screen, /*width*/ 60, /*height*/ 10);

        assert!(rendered.contains("Try new model"), "{rendered}");
        assert!(rendered.contains("Use existing model"), "{rendered}");
        assert_snapshot!("model_migration_long_markdown_constrained", rendered);
    }
}
use std::cell::Cell;

#[path = "model_migration_layout.rs"]
mod layout;
