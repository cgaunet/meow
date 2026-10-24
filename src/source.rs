//! Where a piece of news comes from (Slack, GitHub, Linear…), and the
//! pixel-art icon that shows it on the Activity page.
//!
//! An item from a few sources shows each one's icon; past
//! [`MAX_SEPARATE_ICONS`] distinct sources it shows a single stack icon with
//! their number instead.

use serde::{Deserialize, Serialize};

use crate::sprites::{Frame, Rgb};

/// Icons are square, at most this many pixels a side.
pub const ICON_PIXELS: usize = 10;
/// Up to this many distinct sources get an icon each; more are grouped.
pub const MAX_SEPARATE_ICONS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Slack,
    Calendar,
    GitHub,
    Linear,
    Mail,
    Notion,
    /// A scheduled task or a watch.
    Scheduled,
    /// Typed by hand.
    Me,
    #[serde(other)]
    Other,
}

impl Source {
    /// The source a free-form name like "GitHub PR" or "Gmail" stands for.
    pub fn parse(name: &str) -> Self {
        let name = name.trim().to_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
        if has(&["slack"]) {
            Self::Slack
        } else if has(&["github"]) {
            Self::GitHub
        } else if has(&["linear"]) {
            Self::Linear
        } else if has(&["mail", "inbox"]) {
            Self::Mail
        } else if has(&["calendar", "agenda", "meeting", "invitation"]) {
            Self::Calendar
        } else if has(&["notion"]) {
            Self::Notion
        } else if has(&["schedule", "routine", "watch"]) {
            Self::Scheduled
        } else if name == "me" {
            Self::Me
        } else {
            Self::Other
        }
    }

    /// "GitHub", for tooltips.
    pub fn name(self) -> &'static str {
        match self {
            Self::Slack => "Slack",
            Self::Calendar => "Calendar",
            Self::GitHub => "GitHub",
            Self::Linear => "Linear",
            Self::Mail => "Mail",
            Self::Notion => "Notion",
            Self::Scheduled => "Scheduled",
            Self::Me => "Me",
            Self::Other => "Other",
        }
    }

    fn art(self) -> &'static [&'static str] {
        match self {
            Self::Slack => &SLACK,
            Self::Calendar => &CALENDAR,
            Self::GitHub => &GITHUB,
            Self::Linear => &LINEAR,
            Self::Mail => &MAIL,
            Self::Notion => &NOTION,
            Self::Scheduled => &CLOCK,
            Self::Me => &PERSON,
            Self::Other => &OTHER,
        }
    }
}

/// What an item shows for its sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Icons {
    /// One icon per source.
    Each(Vec<Source>),
    /// One stack icon standing for all of these.
    Grouped(Vec<Source>),
}

impl Icons {
    /// Each distinct source in `sources` (first seen first), grouped when
    /// there are more than [`MAX_SEPARATE_ICONS`].
    pub fn for_sources(sources: &[Source]) -> Self {
        let mut distinct: Vec<Source> = Vec::new();
        for &source in sources {
            if !distinct.contains(&source) {
                distinct.push(source);
            }
        }
        if distinct.len() > MAX_SEPARATE_ICONS {
            Self::Grouped(distinct)
        } else {
            Self::Each(distinct)
        }
    }

    /// The icons to draw, left to right.
    pub fn frames(&self) -> Vec<Frame> {
        match self {
            Self::Each(sources) => sources.iter().map(|&s| icon(s)).collect(),
            Self::Grouped(sources) => vec![group_icon(sources.len())],
        }
    }

    /// "Slack, GitHub and Linear", for the tooltip.
    pub fn describe(&self) -> String {
        let (Self::Each(sources) | Self::Grouped(sources)) = self;
        let names: Vec<&str> = sources.iter().map(|s| s.name()).collect();
        match names.split_last() {
            None => String::new(),
            Some((last, [])) => (*last).to_string(),
            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        }
    }
}

/// A speech bubble holding icons, to show above the cat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewsBubble {
    pub frame: Frame,
    /// Column of the tail's tip, on the bottom row.
    pub tip_x: usize,
}

/// Pixels between the outline and the icons, and between icons.
const BUBBLE_PAD: usize = 1;
const TAIL_ROWS: usize = 2;

/// `icons` in a dark bubble with a light outline, its tail on the left (for
/// a cat facing right) or the right. The bubble fits the icons: smaller ones
/// make it smaller, and sit centered on the tallest.
pub fn news_bubble(icons: &Icons, tail_left: bool) -> NewsBubble {
    let frames = icons.frames();
    let inner_w = frames.iter().map(|icon| icon.width).sum::<usize>()
        + frames.len().saturating_sub(1) * BUBBLE_PAD;
    let inner_h = frames
        .iter()
        .map(|icon| icon.height)
        .max()
        .unwrap_or(ICON_PIXELS);
    let width = inner_w + 2 * (1 + BUBBLE_PAD);
    let body_h = inner_h + 2 * (1 + BUBBLE_PAD);
    let height = body_h + TAIL_ROWS;
    let mut pixels = vec![None; width * height];
    let mut set = |x: usize, y: usize, rgb: Option<Rgb>| pixels[y * width + x] = rgb;
    for y in 0..body_h {
        for x in 0..width {
            let edge_x = x == 0 || x == width - 1;
            let edge_y = y == 0 || y == body_h - 1;
            let color = match (edge_x, edge_y) {
                (true, true) => None,
                (true, false) | (false, true) => Some(WHITE),
                (false, false) => Some(INK),
            };
            set(x, y, color);
        }
    }
    // A two-row tail under the body: "ww" then "w", leaning out.
    let tip_x = if tail_left { 2 } else { width - 3 };
    let next = if tail_left { tip_x + 1 } else { tip_x - 1 };
    set(tip_x, body_h - 1, Some(INK));
    set(next, body_h - 1, Some(INK));
    set(tip_x, body_h, Some(WHITE));
    set(next, body_h, Some(WHITE));
    set(tip_x, body_h + 1, Some(WHITE));
    let mut left = 1 + BUBBLE_PAD;
    for icon in &frames {
        let top = 1 + BUBBLE_PAD + (inner_h - icon.height) / 2;
        for y in 0..icon.height {
            for x in 0..icon.width {
                if let Some(rgb) = icon.get(x, y) {
                    set(left + x, top + y, Some(rgb));
                }
            }
        }
        left += icon.width + BUBBLE_PAD;
    }
    NewsBubble {
        frame: Frame {
            width,
            height,
            pixels,
        },
        tip_x,
    }
}

/// The icon of one source.
pub fn icon(source: Source) -> Frame {
    let art = source.art();
    let pixels = art.iter().flat_map(|row| row.chars().map(color)).collect();
    Frame {
        width: art.len(),
        height: art.len(),
        pixels,
    }
}

/// Three stacked cards, the front one showing `count` (9+ above nine).
pub fn group_icon(count: usize) -> Frame {
    let mut pixels = vec![None; ICON_PIXELS * ICON_PIXELS];
    let mut fill = |x0: usize, y0: usize, x1: usize, y1: usize, rgb: Rgb| {
        for y in y0..y1 {
            for x in x0..x1 {
                pixels[y * ICON_PIXELS + x] = Some(rgb);
            }
        }
    };
    fill(4, 0, 10, 6, ORANGE);
    fill(2, 1, 8, 8, BLUE);
    fill(0, 3, 7, 10, WHITE);
    let glyph = DIGITS.get(count).copied().unwrap_or(PLUS);
    for (dy, row) in glyph.iter().enumerate() {
        for (dx, cell) in row.chars().enumerate() {
            if cell == 'k' {
                fill(2 + dx, 4 + dy, 3 + dx, 5 + dy, INK);
            }
        }
    }
    Frame {
        width: ICON_PIXELS,
        height: ICON_PIXELS,
        pixels,
    }
}

// Sweetie 16, the app's palette.
const INK: Rgb = [0x1a, 0x1c, 0x2c];
const WHITE: Rgb = [0xf4, 0xf4, 0xf4];
const RED: Rgb = [0xb1, 0x3e, 0x53];
const ORANGE: Rgb = [0xef, 0x7d, 0x57];
const YELLOW: Rgb = [0xff, 0xcd, 0x75];
const LIME: Rgb = [0xa7, 0xf0, 0x70];
const BLUE: Rgb = [0x41, 0xa6, 0xf6];
const CYAN: Rgb = [0x73, 0xef, 0xf7];
const GREY: Rgb = [0x94, 0xb0, 0xc2];
const SLATE: Rgb = [0x56, 0x6c, 0x86];

// Slack's own colors, outside the palette: its logo is known by them.
const SLACK_BLUE: Rgb = [0x36, 0xc5, 0xf0];
const SLACK_GREEN: Rgb = [0x2e, 0xb6, 0x7d];
const SLACK_YELLOW: Rgb = [0xec, 0xb2, 0x2e];
const SLACK_RED: Rgb = [0xe0, 0x1e, 0x5a];

fn color(cell: char) -> Option<Rgb> {
    match cell {
        'k' => Some(INK),
        'w' => Some(WHITE),
        'r' => Some(RED),
        'o' => Some(ORANGE),
        'y' => Some(YELLOW),
        'l' => Some(LIME),
        'b' => Some(BLUE),
        'c' => Some(CYAN),
        's' => Some(GREY),
        'd' => Some(SLATE),
        'B' => Some(SLACK_BLUE),
        'G' => Some(SLACK_GREEN),
        'Y' => Some(SLACK_YELLOW),
        'R' => Some(SLACK_RED),
        _ => None,
    }
}

// Art cells: see `color`; `.` is transparent.
/// The pinwheel: four bars, each with a dot beside its outer end, around an
/// empty cross. A one-pixel cross needs an odd size, so it is nine pixels a
/// side; to fit the gap between each bar and its dot, the dots are one thick.
const SLACK: [&str; ICON_PIXELS - 1] = [
    "..BB.GG..",
    ".....GG..",
    "BBBB.GG.G",
    "BBBB.GG.G",
    ".........",
    "R.RR.YYYY",
    "R.RR.YYYY",
    "..RR.....",
    "..RR.YY..",
];
const CALENDAR: [&str; ICON_PIXELS] = [
    "..s....s..",
    "rrsrrrrsrr",
    "rrrrrrrrrr",
    "wwwwwwwwww",
    "wkwwkwwkww",
    "wwwwwwwwww",
    "wkwwkwwkww",
    "wwwwwwwwww",
    "wkwwkwwrrw",
    "wwwwwwwrrw",
];
const GITHUB: [&str; ICON_PIXELS] = [
    ".w......w.",
    ".ww....ww.",
    ".wwwwwwww.",
    "wwwwwwwwww",
    "wwkwwwwkww",
    "wwwwwwwwww",
    ".wwwwwwww.",
    "..wwwwww..",
    "w..w..w...",
    ".ww...w...",
];
/// The mark: a disc whose lower left is cut by three parallel diagonal gaps
/// into strips, beside one large piece. Strips and gaps one pixel wide would
/// make a checkerboard, so the strips are two wide and the gaps one, and the
/// smallest strip is a one-pixel line along the rim.
const LINEAR: [&str; ICON_PIXELS] = [
    "...wwww...",
    ".w.wwwwww.",
    ".ww.wwwww.",
    "w.ww.wwwww",
    "ww.ww.wwww",
    ".ww.ww.www",
    "w.ww.ww.ww",
    ".w.ww.ww..",
    "..w.ww.ww.",
    "...w.ww...",
];
const MAIL: [&str; ICON_PIXELS] = [
    "..........",
    "rwwwwwwwwr",
    "rrwwwwwwrr",
    "rwrwwwwrwr",
    "rwwrwwrwwr",
    "rwwwrrwwwr",
    "rwwwwwwwwr",
    "rwwwwwwwwr",
    "rwwwwwwwwr",
    "..........",
];
const NOTION: [&str; ICON_PIXELS] = [
    "wwwwwwwww.",
    "wkkwwwkkwd",
    "wwkkwwwkwd",
    "wwkwkwwkwd",
    "wwkwwkwkwd",
    "wwkwwwkkwd",
    "wwkwwwwkwd",
    "wkkkwwwkwd",
    "wwwwwwwwwd",
    ".ddddddddd",
];
const CLOCK: [&str; ICON_PIXELS] = [
    "...yyyy...",
    ".yyyyyyyy.",
    ".yyyykyyy.",
    "yyyyykyyyy",
    "yyyyykyyyy",
    "yyyyykkkyy",
    "yyyyyyyyyy",
    ".yyyyyyyy.",
    ".yyyyyyyy.",
    "...yyyy...",
];
const PERSON: [&str; ICON_PIXELS] = [
    "...oooo...",
    "..oooooo..",
    "..oooooo..",
    "...oooo...",
    "..........",
    "..oooooo..",
    ".oooooooo.",
    ".oooooooo.",
    ".oooooooo.",
    ".oooooooo.",
];
const OTHER: [&str; ICON_PIXELS] = [
    "..........",
    ".ssssssss.",
    "ssssssssss",
    "ssssssssss",
    "skksskkskk",
    "skksskkskk",
    "ssssssssss",
    "ssssssssss",
    ".ssssssss.",
    "..........",
];

/// 3×5 digits for the group icon; there are never fewer than
/// `MAX_SEPARATE_ICONS + 1` sources in a group.
const DIGITS: [[&str; 5]; 10] = [
    ["kkk", "k.k", "k.k", "k.k", "kkk"],
    [".k.", "kk.", ".k.", ".k.", "kkk"],
    ["kkk", "..k", "kkk", "k..", "kkk"],
    ["kkk", "..k", ".kk", "..k", "kkk"],
    ["k.k", "k.k", "kkk", "..k", "..k"],
    ["kkk", "k..", "kkk", "..k", "kkk"],
    ["kkk", "k..", "kkk", "k.k", "kkk"],
    ["kkk", "..k", ".k.", ".k.", ".k."],
    ["kkk", "k.k", "kkk", "k.k", "kkk"],
    ["kkk", "k.k", "kkk", "..k", "kkk"],
];
const PLUS: [&str; 5] = ["...", ".k.", "kkk", ".k.", "..."];

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Source; 9] = [
        Source::Slack,
        Source::Calendar,
        Source::GitHub,
        Source::Linear,
        Source::Mail,
        Source::Notion,
        Source::Scheduled,
        Source::Me,
        Source::Other,
    ];

    #[test]
    fn free_form_names_map_to_sources() {
        assert_eq!(Source::parse("GitHub PR review"), Source::GitHub);
        assert_eq!(Source::parse(" Gmail "), Source::Mail);
        assert_eq!(Source::parse("Email"), Source::Mail);
        assert_eq!(Source::parse("Linear"), Source::Linear);
        assert_eq!(Source::parse("Slack thread"), Source::Slack);
        assert_eq!(Source::parse("Google Calendar"), Source::Calendar);
        assert_eq!(Source::parse("Me"), Source::Me);
        assert_eq!(Source::parse("Claude"), Source::Other);
    }

    #[test]
    fn every_icon_is_square_and_fully_drawn() {
        for source in ALL {
            let art = source.art();
            assert!(art.len() <= ICON_PIXELS, "{source:?} is too big");
            assert!(art.iter().all(|row| row.len() == art.len()));
            let frame = icon(source);
            assert_eq!(frame.pixels.len(), frame.width * frame.height);
            assert!(
                frame.pixels.iter().filter(|p| p.is_some()).count() > 20,
                "{source:?} has too few pixels"
            );
        }
    }

    #[test]
    fn a_few_sources_get_one_icon_each_in_order() {
        let icons = Icons::for_sources(&[Source::Slack, Source::GitHub, Source::Slack]);
        assert_eq!(icons, Icons::Each(vec![Source::Slack, Source::GitHub]));
        assert_eq!(icons.frames().len(), 2);
        assert_eq!(icons.describe(), "Slack and GitHub");
        assert_eq!(Icons::for_sources(&[]).frames().len(), 0);
    }

    #[test]
    fn many_sources_share_one_group_icon_with_their_count() {
        let sources = [Source::Slack, Source::GitHub, Source::Linear, Source::Mail];
        let icons = Icons::for_sources(&sources);
        assert!(matches!(&icons, Icons::Grouped(s) if s.len() == 4));
        assert_eq!(icons.frames(), vec![group_icon(4)]);
        assert_eq!(icons.describe(), "Slack, GitHub, Linear and Mail");
        assert_ne!(group_icon(4), group_icon(5), "the count shows");
        assert_eq!(group_icon(12), group_icon(10), "past nine it is a plus");
    }

    #[test]
    fn the_news_bubble_grows_with_its_icons_and_points_down_at_the_cat() {
        let one = news_bubble(&Icons::for_sources(&[Source::GitHub]), true);
        let three = Icons::for_sources(&[Source::GitHub, Source::Linear, Source::Mail]);
        let three = news_bubble(&three, false);
        assert_eq!((one.frame.width, one.frame.height), (14, 16));
        assert_eq!(three.frame.width, 3 * ICON_PIXELS + 2 + 4);
        assert_eq!(one.tip_x, 2);
        assert_eq!(three.tip_x, three.frame.width - 3);
        for bubble in [&one, &three] {
            let bottom = bubble.frame.height - 1;
            let tail: Vec<usize> = (0..bubble.frame.width)
                .filter(|&x| bubble.frame.get(x, bottom).is_some())
                .collect();
            assert_eq!(tail, [bubble.tip_x], "only the tip on the last row");
        }
        let grouped =
            Icons::for_sources(&[Source::Slack, Source::GitHub, Source::Linear, Source::Mail]);
        assert_eq!(news_bubble(&grouped, true).frame.width, one.frame.width);
        // The first icon's top-left pixel sits inside the padding.
        assert_eq!(
            one.frame.get(2, 2),
            icon(Source::GitHub).get(0, 0).or(Some(INK))
        );
    }

    #[test]
    fn a_smaller_icon_makes_a_smaller_bubble() {
        assert_eq!(icon(Source::Slack).width, ICON_PIXELS - 1);
        let alone = news_bubble(&Icons::for_sources(&[Source::Slack]), true);
        assert_eq!((alone.frame.width, alone.frame.height), (13, 15));
        let mixed = news_bubble(&Icons::for_sources(&[Source::Slack, Source::GitHub]), true);
        assert_eq!(mixed.frame.width, (ICON_PIXELS - 1) + ICON_PIXELS + 1 + 4);
        assert_eq!(mixed.frame.height, 16, "as tall as the taller icon");
    }

    #[test]
    fn unknown_saved_sources_read_as_other() {
        let sources: Vec<Source> = serde_json::from_str(r#"["github","fax"]"#).unwrap();
        assert_eq!(sources, [Source::GitHub, Source::Other]);
    }
}
