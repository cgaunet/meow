//! The tasks page: tasks grouped by source, with checkboxes, links and an input.

use std::cell::{Cell, RefCell};

use meow::clock;
use meow::tasks::{Task, TaskGroup, TaskList};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSScrollView, NSTextField, NSView, NSWindow};
use objc2_foundation::{
    NSAttributedString, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};

use super::bubble;
use super::controller::Controller;
use super::flipped::FlippedView;
use super::link_button::LinkButton;
use super::rich_text::{self, Attributes};
use super::theme;
use super::widgets;

/// Space between the task list and the page's edges.
const LIST_INSET: f64 = 20.0;
const ADD_WIDTH: f64 = 64.0;
const ROW_HEIGHT: f64 = 26.0;
/// Each task sits on its own square card.
const CARD_HEIGHT: f64 = 46.0;
const CARD_GAP: f64 = 8.0;
const CARD_PADDING: f64 = 14.0;
const HEADER_HEIGHT: f64 = 30.0;
const GROUP_GAP: f64 = 12.0;
const CHECKBOX_WIDTH: f64 = 22.0;
const CHAT_WIDTH: f64 = 64.0;
const DRAFT_REPLY_WIDTH: f64 = 120.0;
const NO_REPLY_WIDTH: f64 = 96.0;
const INDENT: f64 = 6.0;
const TOGGLE_WIDTH: f64 = 180.0;

pub struct TaskPage {
    pub view: Retained<NSView>,
    scroll: Retained<NSScrollView>,
    list: Retained<FlippedView>,
    input: Retained<widgets::GrowingField>,
    target: Retained<Controller>,
    /// Task ids in the order shown; row buttons carry an index into it.
    shown_ids: RefCell<Vec<String>>,
    /// Whether the completed tasks are listed under the open ones.
    show_completed: Cell<bool>,
    /// The task just ticked, whose row asks whether to draft a reply.
    pending: RefCell<Option<String>>,
    mtm: MainThreadMarker,
}

impl TaskPage {
    pub fn new(mtm: MainThreadMarker, controller: &Retained<Controller>, frame: NSRect) -> Self {
        let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        // The page runs under the bubble's right padding; content keeps clear of it.
        let (width, height) = (frame.size.width - bubble::PADDING, frame.size.height);
        let target: &AnyObject = controller;

        let input = widgets::text_field(mtm, "Add a task…");
        // SAFETY: the controller lives as long as the app, so the weak target stays valid.
        unsafe {
            input.setTarget(Some(target));
            input.setAction(Some(sel!(addTask:)));
        }
        let add = widgets::text_button(
            mtm,
            "Add",
            widgets::ButtonStyle::Primary,
            target,
            sel!(addTask:),
        );
        let composer = widgets::composer(
            mtm,
            &view,
            &input,
            &[],
            &[(&add, ADD_WIDTH, widgets::ButtonStyle::Primary)],
        );

        let bottom = composer + LIST_INSET;
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(mtm),
            rect(
                LIST_INSET,
                bottom,
                width - 2.0 * LIST_INSET,
                height - bottom - LIST_INSET / 2.0,
            ),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setDrawsBackground(false);
        scroll.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let list = FlippedView::new(mtm, rect(0.0, 0.0, scroll.contentSize().width, 0.0));
        list.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        scroll.setDocumentView(Some(&list));
        view.addSubview(&scroll);
        input.keep_clear(&scroll);

        Self {
            view,
            scroll,
            list,
            input,
            target: controller.clone(),
            shown_ids: RefCell::new(Vec::new()),
            show_completed: Cell::new(false),
            pending: RefCell::new(None),
            mtm,
        }
    }

    pub fn focus(&self, window: &NSWindow) {
        window.makeFirstResponder(Some(&self.input));
    }

    pub fn take_input(&self) -> String {
        let text = self.input.stringValue().to_string();
        self.input.set_text("");
        text
    }

    /// The id of the task in row `index`, as last rendered.
    pub fn id_at(&self, index: usize) -> Option<String> {
        self.shown_ids.borrow().get(index).cloned()
    }

    /// Which task's row offers "draft reply" and "no reply", from the next render.
    pub fn set_pending(&self, id: Option<String>) {
        self.pending.replace(id);
    }

    pub fn toggle_completed(&self) {
        self.show_completed.set(!self.show_completed.get());
    }

    /// Open tasks by source, then the completed ones behind a toggle.
    pub fn render(&self, tasks: &TaskList, now: u64) {
        let width = self.scroll.contentSize().width;
        for row in self.list.subviews().iter() {
            row.removeFromSuperview();
        }
        let groups = tasks.groups();
        let completed = tasks.completed();
        let mut ids = Vec::new();
        let mut y = 0.0;
        if groups.is_empty() {
            self.add_placeholder(width, !completed.is_empty());
            y = ROW_HEIGHT + GROUP_GAP;
        }
        for group in &groups {
            y = self.add_group(group, width, y, now, &mut ids) + GROUP_GAP;
        }
        if !completed.is_empty() {
            y = self.add_completed(&completed, width, y, now, &mut ids);
        }
        let height = y.max(self.scroll.contentSize().height);
        self.list.setFrameSize(NSSize::new(width, height));
        self.shown_ids.replace(ids);
    }

    /// Draws a source header and its rows from `y`; returns the y below them.
    fn add_group(
        &self,
        group: &TaskGroup,
        width: f64,
        y: f64,
        now: u64,
        ids: &mut Vec<String>,
    ) -> f64 {
        let header = NSTextField::labelWithAttributedString(&group_header(group), self.mtm);
        header.setFrame(rect(
            INDENT,
            y + 4.0,
            width - 2.0 * INDENT,
            HEADER_HEIGHT - 4.0,
        ));
        header.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        self.list.addSubview(&header);
        let mut y = y + HEADER_HEIGHT;
        for task in &group.tasks {
            self.add_row(ids.len(), task, width, y, now);
            ids.push(task.id.clone());
            y += CARD_HEIGHT + CARD_GAP;
        }
        y
    }

    /// "See completed (3)", and once unfolded the done tasks with when they
    /// were done; returns the y below them.
    fn add_completed(
        &self,
        completed: &[&Task],
        width: f64,
        y: f64,
        now: u64,
        ids: &mut Vec<String>,
    ) -> f64 {
        let target: &AnyObject = &self.target;
        let shown = self.show_completed.get();
        let label = if shown {
            "Hide completed".to_string()
        } else {
            format!("See completed ({})", completed.len())
        };
        let toggle = widgets::text_button(
            self.mtm,
            &label,
            widgets::ButtonStyle::Link,
            target,
            sel!(toggleCompleted:),
        );
        toggle.setFrame(rect(0.0, y, TOGGLE_WIDTH, ROW_HEIGHT));
        self.list.addSubview(&toggle);
        let mut y = y + ROW_HEIGHT + CARD_GAP;
        if shown {
            for task in completed {
                self.add_row(ids.len(), task, width, y, now);
                ids.push(task.id.clone());
                y += CARD_HEIGHT + CARD_GAP;
            }
        }
        y
    }

    fn add_placeholder(&self, width: f64, all_done: bool) {
        let text = if all_done {
            "All done. Nice work."
        } else {
            "No tasks yet. Run a quick action, or add one below."
        };
        let label = NSTextField::labelWithAttributedString(
            &rich_text::attributed(
                text,
                &Attributes::plain(theme::muted(), theme::font(theme::BODY_SIZE)),
            ),
            self.mtm,
        );
        label.setFrame(rect(INDENT, 4.0, width - 2.0 * INDENT, ROW_HEIGHT));
        self.list.addSubview(&label);
    }

    /// A card holding a checkbox, the title (opens the task), and a button to chat about it.
    fn add_row(&self, index: usize, task: &Task, width: f64, y: f64, now: u64) {
        let target: &AnyObject = &self.target;
        let card = FlippedView::new(self.mtm, rect(0.0, y, width, CARD_HEIGHT));
        card.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        let fill = if task.is_done() {
            theme::card_done()
        } else {
            theme::card()
        };
        widgets::paint(&card, Some(&fill), None);
        self.list.addSubview(&card);

        let pending = self.pending.borrow().as_deref() == Some(task.id.as_str());
        // The controller is an app-lifetime target that implements `toggleTask:`.
        let ticked = task.is_done() || pending;
        let checkbox = widgets::checkbox(self.mtm, target, sel!(toggleTask:), ticked);
        checkbox.setTag(index as isize);
        checkbox.setFrame(rect(CARD_PADDING, 0.0, CHECKBOX_WIDTH, CARD_HEIGHT));
        card.addSubview(&checkbox);

        let x = CARD_PADDING + CHECKBOX_WIDTH + INDENT;
        let chat_x = if pending {
            width - CARD_PADDING - DRAFT_REPLY_WIDTH - INDENT - NO_REPLY_WIDTH
        } else {
            width - CARD_PADDING - CHAT_WIDTH
        };
        let title = LinkButton::new(
            self.mtm,
            rect(x, 0.0, chat_x - x - INDENT, CARD_HEIGHT),
            row_title(task, false, now),
            row_title(task, true, now),
            target,
            sel!(openTask:),
        );
        title.setTag(index as isize);
        let mut tip = match &task.url {
            Some(url) => format!("Open {url}"),
            None => "Chat about this task".to_string(),
        };
        if task.auto_done {
            tip.push_str("\nTicked automatically: resolved at the source.");
        }
        title.setToolTip(Some(&NSString::from_str(&tip)));
        title.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        card.addSubview(&title);

        if pending {
            return self.add_reply_choice(&card, index, chat_x);
        }
        let chat = widgets::text_button(
            self.mtm,
            "Chat",
            widgets::ButtonStyle::Quiet,
            target,
            sel!(chatAboutTask:),
        );
        chat.setTag(index as isize);
        chat.setToolTip(Some(&NSString::from_str(
            "Chat with the cat about this task",
        )));
        chat.setFrame(rect(
            chat_x,
            (CARD_HEIGHT - ROW_HEIGHT) / 2.0,
            CHAT_WIDTH,
            ROW_HEIGHT,
        ));
        chat.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        card.addSubview(&chat);
    }

    /// In place of "Chat" on a task just ticked: "Draft reply" and "No reply".
    fn add_reply_choice(&self, card: &FlippedView, index: usize, x: f64) {
        let target: &AnyObject = &self.target;
        let buttons = [
            (
                "Draft reply",
                widgets::ButtonStyle::Primary,
                sel!(draftDoneReply:),
                DRAFT_REPLY_WIDTH,
                "Claude drafts the reply telling whoever asked that it is done (never sent)",
            ),
            (
                "No reply",
                widgets::ButtonStyle::Quiet,
                sel!(noDoneReply:),
                NO_REPLY_WIDTH,
                "Seen together or nothing to answer (chosen after a few seconds)",
            ),
        ];
        let mut x = x;
        for (label, style, action, width, tip) in buttons {
            let button = widgets::text_button(self.mtm, label, style, target, action);
            button.setTag(index as isize);
            button.setToolTip(Some(&NSString::from_str(tip)));
            button.setFrame(rect(x, (CARD_HEIGHT - ROW_HEIGHT) / 2.0, width, ROW_HEIGHT));
            button.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
            card.addSubview(&button);
            x += width + INDENT;
        }
    }
}

/// "GITHUB  2 open": the source in bold, its open count dimmed.
fn group_header(group: &TaskGroup) -> Retained<NSAttributedString> {
    let result = NSMutableAttributedString::new();
    let source = Attributes::plain(
        theme::highlight(),
        theme::label_font(theme::LABEL_SIZE + 1.0),
    );
    result.appendAttributedString(&rich_text::attributed(
        &group.source.to_uppercase(),
        &source,
    ));
    let count = match group.open_count() {
        0 => "  all done".to_string(),
        n => format!("  {n} open"),
    };
    let dim = Attributes::plain(theme::muted(), theme::font(theme::SMALL_SIZE));
    result.appendAttributedString(&rich_text::attributed(&count, &dim));
    result.into_super()
}

/// The task title, faded and struck through once done, with when it was done.
/// Hovering underlines it in the accent color, to show it can be clicked.
fn row_title(task: &Task, hover: bool, now: u64) -> Retained<NSAttributedString> {
    let color = if hover {
        theme::accent()
    } else if task.is_done() {
        theme::faint()
    } else {
        theme::ink()
    };
    let attributes = Attributes {
        strike: task.is_done() && !hover,
        underline: hover,
        ..Attributes::plain(color, theme::font(theme::BODY_SIZE))
    };
    let title = NSMutableAttributedString::new();
    title.appendAttributedString(&rich_text::attributed(&task.title, &attributes));
    if task.auto_done {
        // Ticked by the background check, not by hand.
        let mark = Attributes::plain(theme::faint(), theme::font(theme::SMALL_SIZE));
        title.appendAttributedString(&rich_text::attributed(" · auto", &mark));
    }
    if let Some(done_at) = task.done_at {
        let mark = Attributes::plain(theme::faint(), theme::font(theme::SMALL_SIZE));
        let when = format!(" · done {}", clock::when_label(done_at, now));
        title.appendAttributedString(&rich_text::attributed(&when, &mark));
    }
    title.into_super()
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
