//! The results chart: cumulative WPM, instantaneous raw WPM, and errors.
//!
//! The interesting problem here is the right-hand "Errors" axis. `Chart` has
//! exactly one `x_axis` and one `y_axis` — there is no secondary axis — so the
//! error series is normalised into the WPM range and its scale is drawn by hand
//! in a gutter reserved outside the widget. Doing that needs the plot's inner
//! rectangle, which ratatui keeps private; see [`plot_rect`] for how it is
//! recovered and why it is done that way.

use std::cell::RefCell;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType, Paragraph, Widget};

use clackr_core::Summary;

use crate::term::caps::Fidelity;
use crate::ui::palette::Palette;

/// Narrower than this and the plot carries less information than the columns it
/// costs, so the hero takes the whole row instead.
pub const MIN_WIDTH: u16 = 28;

/// Rows the plot needs before the vertical axis title is worth drawing in full.
const Y_TITLE: &str = "Words per Minute";
/// What it degrades to when the plot is shorter than [`Y_TITLE`].
const Y_TITLE_SHORT: &str = "wpm";
const ERR_TITLE: &str = "Errors";

/// At most this many ticks on the faked error axis. Four keeps the labels
/// integral for any realistic error count without crowding a 10-row plot.
const MAX_ERROR_TICKS: u32 = 4;

/// Whether the chart earns its space at all.
pub fn visible(area: Rect, fidelity: Fidelity) -> bool {
    match fidelity {
        // At `Plain` the chart is a single summary line; below 20 rows even
        // that is worth less than the row it occupies.
        Fidelity::Plain => area.height >= 20,
        _ => true,
    }
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    summary: &Summary,
    palette: &Palette,
    fidelity: Fidelity,
) {
    if area.is_empty() {
        return;
    }
    // Two points is the minimum for a line; one sample is a dot in an empty
    // box, which reads as a bug rather than as a result.
    if fidelity == Fidelity::Plain || summary.series.len() < 2 {
        draw_text_summary(frame, area, summary, palette);
        return;
    }
    draw_plot(frame, area, summary, palette, fidelity);
}

/// The `Plain` chart: one line of the numbers the plot would have shown.
fn draw_text_summary(frame: &mut Frame, area: Rect, summary: &Summary, palette: &Palette) {
    let raw: Vec<f64> = summary.series.iter().map(|s| s.raw_wpm).collect();
    if raw.is_empty() {
        return;
    }
    let avg = raw.iter().sum::<f64>() / raw.len() as f64;
    let peak = raw.iter().copied().fold(f64::MIN, f64::max);
    let low = raw.iter().copied().fold(f64::MAX, f64::min);
    // Deliberately ASCII: `Plain` is also the tier a non-UTF-8 terminal lands
    // on, so the separator cannot be a middle dot.
    let text = format!("raw  avg {avg:.0}  peak {peak:.0}  low {low:.0}");
    frame.render_widget(Paragraph::new(Line::styled(text, palette.sub)), area);
}

fn draw_plot(
    frame: &mut Frame,
    area: Rect,
    summary: &Summary,
    palette: &Palette,
    fidelity: Fidelity,
) {
    let n = summary.series.len();
    // Second `i` covers `[i, i+1)`, so the sample belongs at `i + 1` seconds
    // elapsed — which is also what makes the last x label the test duration.
    let x = |i: usize| (summary.series[i].second + 1) as f64;
    let wpm_points: Vec<(f64, f64)> = (0..n).map(|i| (x(i), summary.series[i].wpm)).collect();
    let raw_points: Vec<(f64, f64)> = (0..n).map(|i| (x(i), summary.series[i].raw_wpm)).collect();

    let y_top = nice_ceil(
        wpm_points
            .iter()
            .chain(&raw_points)
            .map(|p| p.1)
            .fold(0.0, f64::max),
    );
    let (x_min, x_max) = (wpm_points[0].0, wpm_points[n - 1].0);

    let errors = ErrorAxis::of(summary);
    // Errors are counts, not WPM, so they get mapped onto the WPM axis and the
    // scale that undoes the mapping is printed in the gutter. When a test was
    // flawless there is nothing to map and the gutter disappears entirely,
    // which is the clean single-axis chart the reference screenshot shows.
    let error_points: Vec<(f64, f64)> = errors
        .map(|e| {
            summary
                .series
                .iter()
                .enumerate()
                .filter(|(_, s)| s.errors > 0)
                .map(|(i, s)| (x(i), f64::from(s.errors) * y_top / f64::from(e.top)))
                .collect()
        })
        .unwrap_or_default();

    let gutter_width = errors.map_or(0, |e| e.gutter_width(fidelity));
    let title_width = u16::from(fidelity == Fidelity::Full);
    let [title_rect, chart_rect, gutter_rect] = Layout::horizontal([
        Constraint::Length(title_width),
        Constraint::Min(0),
        Constraint::Length(gutter_width),
    ])
    .areas(area);
    if chart_rect.is_empty() {
        return;
    }

    let block = match fidelity {
        Fidelity::Full => Some(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(palette.sub_alt),
        ),
        Fidelity::Reduced => Some(Block::bordered().border_style(palette.sub_alt)),
        Fidelity::Plain => None,
    };
    let inner = match &block {
        Some(b) => b.inner(chart_rect),
        None => chart_rect,
    };
    if inner.is_empty() {
        return;
    }

    let y_labels = y_label_texts(y_top);
    let x_labels = x_label_texts(x_min, x_max, inner.width, fidelity);

    let (line_marker, point_marker) = match fidelity {
        // Braille gives 2x4 sub-cell resolution, which is what makes a
        // piecewise-linear polyline read as a smooth curve past ~60 columns.
        Fidelity::Full => (Marker::Braille, Marker::Dot),
        _ => (Marker::HalfBlock, Marker::Dot),
    };

    // Draw order is significant: raw first so the accent-coloured net line wins
    // wherever the two overlap, and the error scatter last so a spike is never
    // hidden under a line. A different marker also puts the scatter on its own
    // canvas layer, which is what keeps it on top.
    let mut datasets = vec![
        Dataset::default()
            .data(&raw_points)
            .marker(line_marker)
            .graph_type(GraphType::Line)
            .style(palette.sub),
        Dataset::default()
            .data(&wpm_points)
            .marker(line_marker)
            .graph_type(GraphType::Line)
            .style(palette.main),
    ];
    if !error_points.is_empty() {
        datasets.push(
            Dataset::default()
                .data(&error_points)
                .marker(point_marker)
                .graph_type(GraphType::Scatter)
                .style(palette.error),
        );
    }

    let mut y_axis = Axis::default()
        .style(palette.sub_alt)
        .bounds([0.0, y_top])
        .labels(styled(&y_labels, palette.sub));
    if fidelity == Fidelity::Reduced {
        // No column is reserved for a vertical title at this tier, so ratatui's
        // own horizontal title is the honest fallback.
        y_axis = y_axis.title(Line::styled(Y_TITLE_SHORT, palette.sub));
    }

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(palette.sub_alt)
                .bounds([x_min, x_max])
                .labels(styled(&x_labels, palette.sub)),
        )
        .y_axis(y_axis)
        // No dataset is named, so this only documents intent — the reference
        // screenshot has no legend and the plot is too small to spare the box.
        .legend_position(None);

    let chart = match block {
        Some(b) => chart.block(b),
        None => chart,
    };
    frame.render_widget(chart, chart_rect);

    let geometry = PlotGeometry::new(inner, &y_labels, &x_labels);
    let Some(plot) = plot_rect(geometry) else {
        return;
    };
    let buf = frame.buffer_mut();
    if let Some(e) = errors {
        draw_error_axis(buf, gutter_rect, plot, e, palette, fidelity);
    }
    if title_width > 0 {
        draw_vertical(buf, title_rect.x, plot, pick_y_title(plot), palette.sub);
    }
}

fn pick_y_title(plot: Rect) -> &'static str {
    if usize::from(plot.height) >= Y_TITLE.chars().count() {
        Y_TITLE
    } else {
        Y_TITLE_SHORT
    }
}

/// The normalised error axis: how many integer ticks, and the value at the top.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ErrorAxis {
    ticks: u32,
    top: u32,
}

impl ErrorAxis {
    fn of(summary: &Summary) -> Option<Self> {
        let max = summary.series.iter().map(|s| s.errors).max().unwrap_or(0);
        if max == 0 {
            return None;
        }
        // Round the top up to a multiple of the tick count so every label is a
        // whole number of errors — "1.5 errors" would be nonsense.
        let ticks = max.min(MAX_ERROR_TICKS);
        Some(Self {
            ticks,
            top: max.div_ceil(ticks) * ticks,
        })
    }

    /// Columns reserved to the right of the chart: a leading space, the widest
    /// label, and at `Full` a trailing space plus the vertical title column.
    fn gutter_width(self, fidelity: Fidelity) -> u16 {
        let digits = self.top.to_string().len() as u16;
        match fidelity {
            Fidelity::Full => digits + 3,
            _ => digits + 2,
        }
    }
}

fn draw_error_axis(
    buf: &mut Buffer,
    gutter: Rect,
    plot: Rect,
    axis: ErrorAxis,
    palette: &Palette,
    fidelity: Fidelity,
) {
    if gutter.is_empty() || plot.height < 2 {
        return;
    }
    // Mirror ratatui's own y-label spacing (`i * (height - 1) / (n - 1)`) so a
    // tick on the right lands on the same row arithmetic as one on the left.
    for i in 0..=axis.ticks {
        let dy = (u32::from(plot.height - 1) * i / axis.ticks) as u16;
        let value = axis.top * i / axis.ticks;
        put(
            buf,
            gutter.x + 1,
            plot.bottom() - 1 - dy,
            &value.to_string(),
            palette.sub,
        );
    }
    if fidelity == Fidelity::Full {
        draw_vertical(buf, gutter.right() - 1, plot, ERR_TITLE, palette.sub);
    }
}

/// One character per row, vertically centred against `plot`.
fn draw_vertical(buf: &mut Buffer, x: u16, plot: Rect, text: &str, style: Style) {
    let chars: Vec<char> = text.chars().collect();
    let Some(spare) = usize::from(plot.height).checked_sub(chars.len()) else {
        return;
    };
    let top = plot.y + (spare / 2) as u16;
    for (i, c) in chars.iter().enumerate() {
        put(buf, x, top + i as u16, c.encode_utf8(&mut [0u8; 4]), style);
    }
}

/// `Buffer::set_string` clips horizontally but indexes rows unchecked, so every
/// hand-placed label goes through this.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    if buf.area.contains((x, y).into()) {
        buf.set_string(x, y, text, style);
    }
}

fn styled(texts: &[String], style: Style) -> Vec<Line<'static>> {
    texts
        .iter()
        .map(|t| Line::styled(t.clone(), style))
        .collect()
}

/// Rounds the WPM axis top up to a round number so the mid label is whole.
fn nice_ceil(max: f64) -> f64 {
    let step = if max <= 50.0 {
        10.0
    } else if max <= 200.0 {
        20.0
    } else {
        50.0
    };
    (max / step).ceil().max(1.0) * step
}

fn y_label_texts(y_top: f64) -> Vec<String> {
    vec![
        "0".to_string(),
        format!("{:.0}", y_top / 2.0),
        format!("{y_top:.0}"),
    ]
}

fn x_label_texts(x_min: f64, x_max: f64, width: u16, fidelity: Fidelity) -> Vec<String> {
    let span = (x_max - x_min).max(0.0) as usize + 1;
    let wanted = match fidelity {
        Fidelity::Full => 5,
        _ => 3,
    };
    // Each label needs roughly six columns before they start colliding.
    let count = wanted.min(span).min(usize::from(width / 6)).max(2);
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64;
            format!("{:.0}", x_min + t * (x_max - x_min))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Plot geometry
// ---------------------------------------------------------------------------

/// Everything about a chart that can move its plot area.
///
/// `area` is the rect the `Chart` widget draws into *after* any block has taken
/// its border, because that is what ratatui's own layout works from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct PlotGeometry {
    area: Rect,
    y_label_width: u16,
    y_label_count: u16,
    /// Only the *first* x label can push the plot right; ratatui centres the
    /// rest inside the plot.
    x_first_label_width: u16,
    x_label_count: u16,
}

impl PlotGeometry {
    fn new(area: Rect, y_labels: &[String], x_labels: &[String]) -> Self {
        Self {
            area,
            y_label_width: y_labels
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0) as u16,
            y_label_count: y_labels.len() as u16,
            x_first_label_width: x_labels.first().map_or(0, |l| l.chars().count()) as u16,
            x_label_count: x_labels.len() as u16,
        }
    }
}

thread_local! {
    /// A single-entry cache. The geometry only changes on resize, so one slot
    /// covers every frame in between; probing costs two throwaway renders.
    static PLOT_CACHE: RefCell<Option<(PlotGeometry, Option<Rect>)>> =
        const { RefCell::new(None) };
}

/// The rectangle the `Chart` actually plots into.
///
/// ratatui computes this in a private `Chart::layout`, and re-deriving its
/// arithmetic here would break silently — off by one row, labels drifting away
/// from the data — the first time upstream changed it. So the authoritative
/// answer is *measured*: render a sentinel chart with data points pinned to the
/// four corners of the value range into a scratch buffer, render the same chart
/// with no data at all, and take the bounding box of the cells that differ.
/// Those cells are exactly the plotted corners, so their bounding box is
/// exactly the plot. `plot_rect_fast` (test-only, hence not linked) reproduces
/// it arithmetically and is
/// pinned to this by a test, which is what turns a ratatui upgrade that moves
/// the geometry into a loud failure instead of a quiet misalignment.
fn plot_rect(geometry: PlotGeometry) -> Option<Rect> {
    PLOT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached, rect)) = *cache
            && cached == geometry
        {
            return rect;
        }
        let rect = probe_plot_rect(geometry);
        *cache = Some((geometry, rect));
        rect
    })
}

fn probe_plot_rect(geometry: PlotGeometry) -> Option<Rect> {
    let area = geometry.area;
    if area.is_empty() {
        return None;
    }

    // Corner points of the value range. `Marker::Block` is one cell per point,
    // so each corner paints exactly the corner cell of the plot.
    const CORNERS: [(f64, f64); 4] = [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)];
    let empty: [(f64, f64); 0] = [];

    let render = |data: &[(f64, f64)]| {
        let mut buf = Buffer::empty(area);
        Chart::new(vec![
            Dataset::default()
                .data(data)
                .marker(Marker::Block)
                .graph_type(GraphType::Scatter),
        ])
        .x_axis(Axis::default().bounds([0.0, 1.0]).labels(sentinel_labels(
            geometry.x_label_count,
            geometry.x_first_label_width,
        )))
        .y_axis(Axis::default().bounds([0.0, 1.0]).labels(sentinel_labels(
            geometry.y_label_count,
            geometry.y_label_width,
        )))
        .legend_position(None)
        .render(area, &mut buf);
        buf
    };

    let baseline = render(&empty);
    let probed = render(&CORNERS);

    let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0u16, 0u16);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if probed[(x, y)].symbol() != baseline[(x, y)].symbol() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0 != u16::MAX).then(|| Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// Labels whose *widths* match the real ones; only the widths and the count
/// affect the layout.
fn sentinel_labels(count: u16, first_width: u16) -> Vec<Line<'static>> {
    (0..count)
        .map(|i| {
            let w = if i == 0 { first_width } else { 1 };
            Line::from("0".repeat(usize::from(w)))
        })
        .collect()
}

/// The arithmetic twin of [`probe_plot_rect`], transcribed from ratatui's
/// private `Chart::layout`. It exists purely so the pin test below can prove
/// the two agree; the probe stays authoritative at runtime precisely because
/// this transcription is the part that silently rots on an upgrade.
#[cfg(test)]
fn plot_rect_fast(geometry: PlotGeometry) -> Option<Rect> {
    let area = geometry.area;
    if area.is_empty() {
        return None;
    }
    let has_x = geometry.x_label_count > 0;
    let has_y = geometry.y_label_count > 0;

    let mut x = area.left();
    let mut y = area.bottom() - 1;

    if has_x && y > area.top() {
        y -= 1; // the x label row
    }

    let mut left_of_axis = if has_y { geometry.y_label_width } else { 0 };
    if has_x {
        // The first x label hangs left of the y axis, minus the axis column it
        // is allowed to sit under.
        let overhang = geometry
            .x_first_label_width
            .saturating_sub(u16::from(has_y));
        left_of_axis = left_of_axis.max(overhang);
    }
    // ratatui caps the gutter at a third of the width so labels can never eat
    // the plot.
    x += left_of_axis.min(area.width / 3);

    if has_x && y > area.top() {
        y -= 1; // the x axis line
    }
    if has_y && x + 1 < area.right() {
        x += 1; // the y axis line
    }

    Some(Rect::new(
        x,
        area.top(),
        area.right().saturating_sub(x),
        y.saturating_sub(area.top()) + 1,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use std::time::Duration;

    use clackr_core::stats::{CharCounts, SecondSample};

    use crate::ui::results::tests::render;

    /// `Summary::fixture` is flawless, so it never exercises the error axis.
    /// This one has errors in three of its ten seconds.
    pub(crate) fn fixture_with_errors() -> Summary {
        const KEYS: [u32; 10] = [5, 6, 4, 5, 7, 3, 5, 6, 4, 6];
        const ERRORS: [u32; 10] = [0, 0, 2, 0, 0, 3, 0, 0, 1, 0];

        let incorrect: u32 = ERRORS.iter().sum();
        let counts = CharCounts {
            correct_word_chars: 38,
            all_correct_chars: 45,
            incorrect,
            extra: 2,
            missed: 1,
            spaces: 9,
            correct_spaces: 8,
        };
        let mut cumulative = 0u32;
        let series: Vec<SecondSample> = KEYS
            .iter()
            .zip(ERRORS)
            .enumerate()
            .map(|(i, (&keys, errors))| {
                cumulative += keys - errors;
                SecondSample {
                    second: i as u32,
                    keys,
                    errors,
                    raw_wpm: f64::from(keys) * 12.0,
                    wpm: f64::from(cumulative) * 12.0 / (i as f64 + 1.0),
                }
            })
            .collect();
        let duration = Duration::from_secs(KEYS.len() as u64);

        Summary {
            params: clackr_core::TestParams::default(),
            wpm: clackr_core::stats::wpm(counts.net_chars(), duration),
            raw: clackr_core::stats::wpm(counts.raw_chars(), duration),
            acc: clackr_core::stats::accuracy(KEYS.iter().sum::<u32>() - incorrect, incorrect),
            consistency: clackr_core::stats::consistency(&series, Default::default()),
            counts,
            duration,
            series,
        }
    }

    /// The tripwire. If a ratatui upgrade moves the plot, the arithmetic path
    /// and the probe stop agreeing and this fails loudly — instead of the
    /// hand-placed error labels silently drifting a row off the data.
    #[test]
    fn the_fast_path_agrees_with_the_probe() {
        let mut checked = 0;
        for (w, h) in [
            (20, 6),
            (28, 8),
            (30, 10),
            (40, 12),
            (60, 16),
            (80, 18),
            (100, 20),
            (120, 24),
            (200, 40),
            (1, 1),
            (3, 3),
            (5, 2),
        ] {
            for (yw, yc) in [(0, 0), (1, 2), (2, 3), (3, 3), (5, 3)] {
                for (xw, xc) in [(0, 0), (1, 2), (2, 5), (4, 5)] {
                    let geometry = PlotGeometry {
                        // A non-zero origin catches anything that confuses an
                        // offset with a width.
                        area: Rect::new(3, 2, w, h),
                        y_label_width: yw,
                        y_label_count: yc,
                        x_first_label_width: xw,
                        x_label_count: xc,
                    };
                    assert_eq!(
                        plot_rect_fast(geometry),
                        probe_plot_rect(geometry),
                        "{geometry:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= 30, "only {checked} combinations");
    }

    #[test]
    fn a_flawless_test_has_no_error_axis_and_no_gutter() {
        assert_eq!(ErrorAxis::of(&Summary::fixture()), None);
        let text = render(120, 40, &Summary::fixture(), Fidelity::Full);
        assert!(!text.contains("Errors"), "{text}");
    }

    #[test]
    fn errors_produce_integer_ticks_and_a_titled_gutter() {
        let summary = fixture_with_errors();
        // Three errors in the worst second, so three ticks of one error each.
        assert_eq!(
            ErrorAxis::of(&summary),
            Some(ErrorAxis { ticks: 3, top: 3 })
        );
        let text = render(120, 40, &summary, Fidelity::Full);
        for line in ERR_TITLE.chars() {
            assert!(text.contains(line), "{text}");
        }
        assert!(text.contains("Words per Minute".chars().next().unwrap()));
    }

    #[test]
    fn the_error_top_is_always_a_whole_multiple_of_the_tick_count() {
        for max in 1u32..200 {
            let mut summary = Summary::fixture();
            summary.series[0].errors = max;
            let axis = ErrorAxis::of(&summary).unwrap();
            assert!(axis.ticks <= MAX_ERROR_TICKS);
            assert_eq!(axis.top % axis.ticks, 0, "max {max}");
            assert!(axis.top >= max, "max {max} top {}", axis.top);
        }
    }

    /// The point of probing the plot rect: the hand-placed error ticks have to
    /// land on the same rows ratatui chose for its own y labels. If the probe
    /// were a row out, the two scales would disagree and every error reading
    /// would be wrong.
    #[test]
    fn the_faked_error_ticks_share_rows_with_ratatuis_own_y_labels() {
        let text = render(120, 40, &fixture_with_errors(), Fidelity::Full);
        let lines: Vec<&str> = text.lines().collect();
        let find = |p: &dyn Fn(&&str) -> bool| lines.iter().position(p).expect("row");

        // Top of the WPM axis and top of the error axis are the same row...
        assert_eq!(
            find(&|l: &&str| l.contains("│100│")),
            find(&|l: &&str| l.ends_with(" 3")),
            "{text}"
        );
        // ...and so are the two zeroes at the bottom.
        assert_eq!(
            find(&|l: &&str| l.contains("│0  │")),
            find(&|l: &&str| l.ends_with(" 0")),
            "{text}"
        );
    }

    #[test]
    fn reduced_fidelity_labels_the_error_axis_but_does_not_title_it() {
        let text = render(100, 30, &fixture_with_errors(), Fidelity::Reduced);
        assert!(text.lines().any(|l| l.ends_with(" 3")), "{text}");
        assert!(!text.contains("Error"), "{text}");
    }

    #[test]
    fn plain_fidelity_is_one_ascii_line_of_numbers() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Plain);
        assert!(text.contains("peak"), "{text}");
        assert!(text.is_ascii(), "{text}");
    }

    #[test]
    fn plain_fidelity_hides_the_chart_below_twenty_rows() {
        assert!(!visible(Rect::new(0, 0, 120, 19), Fidelity::Plain));
        assert!(visible(Rect::new(0, 0, 120, 20), Fidelity::Plain));
        assert!(visible(Rect::new(0, 0, 120, 8), Fidelity::Full));
    }

    #[test]
    fn full_fidelity_draws_braille() {
        let text = render(120, 40, &Summary::fixture(), Fidelity::Full);
        assert!(
            text.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)),
            "{text}"
        );
    }

    #[test]
    fn reduced_fidelity_uses_half_blocks_and_no_braille() {
        let text = render(100, 30, &Summary::fixture(), Fidelity::Reduced);
        assert!(
            !text.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)),
            "{text}"
        );
        assert!(
            text.contains('▀') || text.contains('▄') || text.contains('█'),
            "{text}"
        );
    }

    #[test]
    fn a_single_sample_falls_back_to_text_rather_than_a_dot_in_a_box() {
        let mut summary = Summary::fixture();
        summary.series.truncate(1);
        let text = render(120, 40, &summary, Fidelity::Full);
        assert!(text.contains("peak"), "{text}");
    }

    #[test]
    fn axis_tops_are_round_numbers() {
        assert_eq!(nice_ceil(0.0), 10.0);
        assert_eq!(nice_ceil(61.2), 80.0);
        assert_eq!(nice_ceil(50.0), 50.0);
        assert_eq!(nice_ceil(201.0), 250.0);
    }
}
