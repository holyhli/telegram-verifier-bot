use crate::bot::handlers::stats::{StatsCallbackData, StatsPeriod, StatsView};
pub use crate::services::stats::{ActiveApplicantInfo, ApplicantSummary, QuestionTiming};

const PAGE_SIZE: usize = 10;
const MESSAGE_CHAR_LIMIT: usize = 4096;
const TRUNCATION_SUFFIX: &str = "\n... (truncated)";

/// Pure formatter — takes data, returns formatted HTML text and keyboard rows.
/// No database access, no Telegram API calls.
pub struct StatsFormatter;

impl StatsFormatter {
    /// Format community selection screen.
    /// Returns (message_text, keyboard_rows) where each keyboard row has (label, callback_data).
    pub fn format_community_selection(
        communities: &[(i64, String)],
    ) -> (String, Vec<Vec<(String, String)>>) {
        let text = "📊 <b>Stats</b>\n\nSelect a community:".to_string();

        let keyboard: Vec<Vec<(String, String)>> = communities
            .iter()
            .map(|(id, title)| {
                let cb = StatsCallbackData::SelectCommunity { community_id: *id };
                vec![(title.clone(), cb.encode())]
            })
            .collect();

        (truncate_to_limit(text, MESSAGE_CHAR_LIMIT), keyboard)
    }

    /// Format period selection screen for a given community.
    pub fn format_period_selection(
        community_title: &str,
        community_id: i64,
    ) -> (String, Vec<Vec<(String, String)>>) {
        let text = format!(
            "📊 <b>{}</b>\n\nSelect time period:",
            html_escape(community_title)
        );

        let periods = [
            (StatsPeriod::Today, "Today"),
            (StatsPeriod::ThisWeek, "This Week"),
            (StatsPeriod::ThisMonth, "This Month"),
            (StatsPeriod::AllTime, "All Time"),
        ];

        // 2x2 grid
        let keyboard = vec![
            vec![
                (
                    periods[0].1.to_string(),
                    StatsCallbackData::SelectPeriod {
                        community_id,
                        period: periods[0].0,
                    }
                    .encode(),
                ),
                (
                    periods[1].1.to_string(),
                    StatsCallbackData::SelectPeriod {
                        community_id,
                        period: periods[1].0,
                    }
                    .encode(),
                ),
            ],
            vec![
                (
                    periods[2].1.to_string(),
                    StatsCallbackData::SelectPeriod {
                        community_id,
                        period: periods[2].0,
                    }
                    .encode(),
                ),
                (
                    periods[3].1.to_string(),
                    StatsCallbackData::SelectPeriod {
                        community_id,
                        period: periods[3].0,
                    }
                    .encode(),
                ),
            ],
        ];

        (truncate_to_limit(text, MESSAGE_CHAR_LIMIT), keyboard)
    }

    /// Format active applicants view with pagination.
    pub fn format_active_view(
        community_title: &str,
        community_id: i64,
        period: &StatsPeriod,
        applicants: &[ActiveApplicantInfo],
        page: u32,
        total_pages: u32,
    ) -> (String, Vec<Vec<(String, String)>>) {
        let mut text = format!(
            "📊 <b>{}</b> — Active ({})\n\n🔄 {} applicant{} in progress:\n",
            html_escape(community_title),
            period.label(),
            applicants.len(),
            if applicants.len() == 1 { "" } else { "s" }
        );

        let start = (page.saturating_sub(1) as usize) * PAGE_SIZE;
        let page_items = applicants.iter().skip(start).take(PAGE_SIZE);

        for (i, a) in page_items.enumerate() {
            let idx = start + i + 1;
            let name_str = format_name(&a.name, &a.username);
            text.push_str(&format!(
                "\n{}. {}\n   📍 Question {}/{} — \"{}\"\n   ⏱ {} on this question | Started {} ago\n",
                idx,
                name_str,
                a.current_question_position,
                a.total_questions,
                html_escape(&a.current_question_text),
                format_duration(a.time_on_current_secs),
                format_duration(a.time_started_secs),
            ));
        }

        if applicants.is_empty() {
            text.push_str("\nNo active applicants in this period.");
        }

        let keyboard =
            build_nav_keyboard(community_id, period, &StatsView::Active, page, total_pages);

        (truncate_to_limit(text, MESSAGE_CHAR_LIMIT), keyboard)
    }

    /// Format summary view with pagination.
    pub fn format_summary_view(
        community_title: &str,
        community_id: i64,
        period: &StatsPeriod,
        summaries: &[ApplicantSummary],
        page: u32,
        total_pages: u32,
    ) -> (String, Vec<Vec<(String, String)>>) {
        let mut text = format!(
            "📊 <b>{}</b> — Summary ({})\n",
            html_escape(community_title),
            period.label(),
        );

        let start = (page.saturating_sub(1) as usize) * PAGE_SIZE;
        let page_items = summaries.iter().skip(start).take(PAGE_SIZE);

        for (i, s) in page_items.enumerate() {
            let idx = start + i + 1;
            let name_str = format_name(&s.name, &s.username);
            let status_icon = match s.status.as_str() {
                "approved" => "✅",
                "rejected" => "❌",
                "banned" => "🚫",
                "expired" => "⏰",
                "submitted" => "📋",
                _ => "❓",
            };

            text.push_str(&format!(
                "\n{}. {} — {} {}\n",
                idx,
                name_str,
                status_icon,
                capitalize(&s.status),
            ));

            for qt in &s.question_timings {
                let dur = qt
                    .duration_secs
                    .map(|d| format_duration(d))
                    .unwrap_or_else(|| "—".to_string());
                let warning = if qt.duration_secs.unwrap_or(0) > 600 {
                    " ⚠️"
                } else {
                    ""
                };
                text.push_str(&format!(
                    "   Q{} ({}): {}{}\n",
                    qt.position,
                    html_escape(&qt.question_text),
                    dur,
                    warning,
                ));
            }

            let total_dur = s
                .total_time_secs
                .map(|d| format_duration(d))
                .unwrap_or_else(|| "—".to_string());
            text.push_str(&format!(
                "   Total: {} | Retries: {}\n",
                total_dur, s.total_retries,
            ));
        }

        if summaries.is_empty() {
            text.push_str("\nNo applicants in this period.");
        }

        let keyboard =
            build_nav_keyboard(community_id, period, &StatsView::Summary, page, total_pages);

        (truncate_to_limit(text, MESSAGE_CHAR_LIMIT), keyboard)
    }
}

// --- Private helpers ---

fn format_duration(secs: i64) -> String {
    let secs = secs.unsigned_abs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{}m", m)
        } else {
            format!("{}m {}s", m, s)
        }
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{}h", h)
        } else {
            format!("{}h {}m", h, m)
        }
    }
}

fn format_name(name: &Option<String>, username: &Option<String>) -> String {
    match (name, username) {
        (Some(n), Some(u)) => format!("{} (@{})", n, u),
        (Some(n), None) => n.clone(),
        (None, Some(u)) => format!("@{}", u),
        (None, None) => "Unknown".to_string(),
    }
}

fn truncate_to_limit(text: String, limit: usize) -> String {
    if text.len() <= limit {
        return text;
    }
    let cut = limit - TRUNCATION_SUFFIX.len();
    // Find a safe UTF-8 boundary
    let safe_cut = text
        .char_indices()
        .take_while(|(i, _)| *i <= cut)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let mut result = text[..safe_cut].to_string();
    result.push_str(TRUNCATION_SUFFIX);
    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn build_nav_keyboard(
    community_id: i64,
    period: &StatsPeriod,
    current_view: &StatsView,
    page: u32,
    total_pages: u32,
) -> Vec<Vec<(String, String)>> {
    let mut nav_row: Vec<(String, String)> = Vec::new();

    // Prev button (only if page > 1)
    if page > 1 {
        nav_row.push((
            "◀ Prev".to_string(),
            StatsCallbackData::Navigate {
                community_id,
                period: *period,
                view: *current_view,
                page: page - 1,
            }
            .encode(),
        ));
    }

    // Toggle button: switch between Active and Summary
    let (toggle_label, toggle_view) = match current_view {
        StatsView::Active => ("Active | Summary", StatsView::Summary),
        StatsView::Summary => ("Active | Summary", StatsView::Active),
    };
    nav_row.push((
        toggle_label.to_string(),
        StatsCallbackData::Navigate {
            community_id,
            period: *period,
            view: toggle_view,
            page: 1,
        }
        .encode(),
    ));

    // Next button (only if page < total_pages)
    if page < total_pages {
        nav_row.push((
            "Next ▶".to_string(),
            StatsCallbackData::Navigate {
                community_id,
                period: *period,
                view: *current_view,
                page: page + 1,
            }
            .encode(),
        ));
    }

    vec![nav_row]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(72), "1m 12s");
        assert_eq!(format_duration(1380), "23m");
        assert_eq!(format_duration(930), "15m 30s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(7380), "2h 3m");
        assert_eq!(format_duration(86400), "24h");
    }

    #[test]
    fn test_format_name_variants() {
        assert_eq!(
            format_name(&Some("John".into()), &Some("johndoe".into())),
            "John (@johndoe)"
        );
        assert_eq!(format_name(&Some("John".into()), &None), "John");
        assert_eq!(format_name(&None, &Some("johndoe".into())), "@johndoe");
        assert_eq!(format_name(&None, &None), "Unknown");
    }

    #[test]
    fn test_truncate_within_limit() {
        let text = "Hello world".to_string();
        assert_eq!(truncate_to_limit(text.clone(), 100), text);
    }

    #[test]
    fn test_truncate_exceeds_limit() {
        let text = "A".repeat(4100);
        let result = truncate_to_limit(text, 4096);
        assert!(result.len() <= 4096);
        assert!(result.ends_with("... (truncated)"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(html_escape("no special"), "no special");
    }
}
