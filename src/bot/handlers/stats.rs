use chrono::{DateTime, Duration, Utc};

/// Represents the time period for statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsPeriod {
    Today,
    ThisWeek,
    ThisMonth,
    AllTime,
}

impl StatsPeriod {
    /// Convert period to single character for compact encoding
    pub fn to_char(&self) -> char {
        match self {
            StatsPeriod::Today => 't',
            StatsPeriod::ThisWeek => 'w',
            StatsPeriod::ThisMonth => 'm',
            StatsPeriod::AllTime => 'a',
        }
    }

    /// Parse period from single character
    pub fn from_char(c: char) -> Option<StatsPeriod> {
        match c {
            't' => Some(StatsPeriod::Today),
            'w' => Some(StatsPeriod::ThisWeek),
            'm' => Some(StatsPeriod::ThisMonth),
            'a' => Some(StatsPeriod::AllTime),
            _ => None,
        }
    }

    /// Get the start date (UTC) for this period
    pub fn start_date(&self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            StatsPeriod::Today => {
                // Start of today (00:00:00 UTC)
                now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
            }
            StatsPeriod::ThisWeek => {
                // 7 days ago
                now - Duration::days(7)
            }
            StatsPeriod::ThisMonth => {
                // 30 days ago
                now - Duration::days(30)
            }
            StatsPeriod::AllTime => {
                // Unix epoch
                DateTime::UNIX_EPOCH
            }
        }
    }

    /// Get human-readable label
    pub fn label(&self) -> &str {
        match self {
            StatsPeriod::Today => "Today",
            StatsPeriod::ThisWeek => "This Week",
            StatsPeriod::ThisMonth => "This Month",
            StatsPeriod::AllTime => "All Time",
        }
    }
}

/// Represents the view type for statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsView {
    Active,
    Summary,
}

impl StatsView {
    /// Convert view to single character for compact encoding
    pub fn to_char(&self) -> char {
        match self {
            StatsView::Active => 'c',
            StatsView::Summary => 's',
        }
    }

    /// Parse view from single character
    pub fn from_char(c: char) -> Option<StatsView> {
        match c {
            'c' => Some(StatsView::Active),
            's' => Some(StatsView::Summary),
            _ => None,
        }
    }
}

/// Callback data for stats navigation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsCallbackData {
    /// User selects a community
    SelectCommunity { community_id: i64 },
    /// User selects a time period for a community
    SelectPeriod {
        community_id: i64,
        period: StatsPeriod,
    },
    /// User navigates to a specific view and page
    Navigate {
        community_id: i64,
        period: StatsPeriod,
        view: StatsView,
        page: u32,
    },
}

impl StatsCallbackData {
    /// Encode callback data to compact string format
    /// Format:
    /// - SelectCommunity: `sc:{community_id}`
    /// - SelectPeriod: `sp:{community_id}:{period_char}`
    /// - Navigate: `sn:{community_id}:{period_char}:{view_char}:{page}`
    pub fn encode(&self) -> String {
        match self {
            StatsCallbackData::SelectCommunity { community_id } => {
                format!("sc:{}", community_id)
            }
            StatsCallbackData::SelectPeriod {
                community_id,
                period,
            } => {
                format!("sp:{}:{}", community_id, period.to_char())
            }
            StatsCallbackData::Navigate {
                community_id,
                period,
                view,
                page,
            } => {
                format!(
                    "sn:{}:{}:{}:{}",
                    community_id,
                    period.to_char(),
                    view.to_char(),
                    page
                )
            }
        }
    }

    /// Parse callback data from string
    /// Returns None if format is invalid or parsing fails
    pub fn parse(data: &str) -> Option<StatsCallbackData> {
        if data.is_empty() {
            return None;
        }

        let parts: Vec<&str> = data.split(':').collect();

        match parts.get(0).copied() {
            Some("sc") => {
                // SelectCommunity: sc:{community_id}
                if parts.len() != 2 {
                    return None;
                }
                let community_id = parts[1].parse::<i64>().ok()?;
                Some(StatsCallbackData::SelectCommunity { community_id })
            }
            Some("sp") => {
                // SelectPeriod: sp:{community_id}:{period_char}
                if parts.len() != 3 {
                    return None;
                }
                let community_id = parts[1].parse::<i64>().ok()?;
                let period_char = parts[2].chars().next()?;
                let period = StatsPeriod::from_char(period_char)?;
                Some(StatsCallbackData::SelectPeriod {
                    community_id,
                    period,
                })
            }
            Some("sn") => {
                // Navigate: sn:{community_id}:{period_char}:{view_char}:{page}
                if parts.len() != 5 {
                    return None;
                }
                let community_id = parts[1].parse::<i64>().ok()?;
                let period_char = parts[2].chars().next()?;
                let period = StatsPeriod::from_char(period_char)?;
                let view_char = parts[3].chars().next()?;
                let view = StatsView::from_char(view_char)?;
                let page = parts[4].parse::<u32>().ok()?;
                Some(StatsCallbackData::Navigate {
                    community_id,
                    period,
                    view,
                    page,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_stats_period_to_char() {
        assert_eq!(StatsPeriod::Today.to_char(), 't');
        assert_eq!(StatsPeriod::ThisWeek.to_char(), 'w');
        assert_eq!(StatsPeriod::ThisMonth.to_char(), 'm');
        assert_eq!(StatsPeriod::AllTime.to_char(), 'a');
    }

    #[test]
    fn test_stats_period_from_char() {
        assert_eq!(StatsPeriod::from_char('t'), Some(StatsPeriod::Today));
        assert_eq!(StatsPeriod::from_char('w'), Some(StatsPeriod::ThisWeek));
        assert_eq!(StatsPeriod::from_char('m'), Some(StatsPeriod::ThisMonth));
        assert_eq!(StatsPeriod::from_char('a'), Some(StatsPeriod::AllTime));
        assert_eq!(StatsPeriod::from_char('x'), None);
    }

    #[test]
    fn test_stats_period_label() {
        assert_eq!(StatsPeriod::Today.label(), "Today");
        assert_eq!(StatsPeriod::ThisWeek.label(), "This Week");
        assert_eq!(StatsPeriod::ThisMonth.label(), "This Month");
        assert_eq!(StatsPeriod::AllTime.label(), "All Time");
    }

    #[test]
    fn test_stats_period_start_date() {
        let now = Utc::now();

        // Today should be start of today
        let today_start = StatsPeriod::Today.start_date();
        assert!(today_start <= now);
        assert_eq!(today_start.hour(), 0);
        assert_eq!(today_start.minute(), 0);
        assert_eq!(today_start.second(), 0);

        // ThisWeek should be roughly 7 days ago
        let week_start = StatsPeriod::ThisWeek.start_date();
        let diff = now - week_start;
        assert!(diff.num_days() >= 6 && diff.num_days() <= 8);

        // ThisMonth should be roughly 30 days ago
        let month_start = StatsPeriod::ThisMonth.start_date();
        let diff = now - month_start;
        assert!(diff.num_days() >= 29 && diff.num_days() <= 31);

        // AllTime should be epoch
        let epoch = StatsPeriod::AllTime.start_date();
        assert_eq!(epoch, DateTime::UNIX_EPOCH);
    }

    #[test]
    fn test_stats_view_to_char() {
        assert_eq!(StatsView::Active.to_char(), 'c');
        assert_eq!(StatsView::Summary.to_char(), 's');
    }

    #[test]
    fn test_stats_view_from_char() {
        assert_eq!(StatsView::from_char('c'), Some(StatsView::Active));
        assert_eq!(StatsView::from_char('s'), Some(StatsView::Summary));
        assert_eq!(StatsView::from_char('x'), None);
    }

    #[test]
    fn test_callback_data_select_community_encode() {
        let data = StatsCallbackData::SelectCommunity { community_id: 42 };
        assert_eq!(data.encode(), "sc:42");
    }

    #[test]
    fn test_callback_data_select_period_encode() {
        let data = StatsCallbackData::SelectPeriod {
            community_id: 42,
            period: StatsPeriod::ThisWeek,
        };
        assert_eq!(data.encode(), "sp:42:w");
    }

    #[test]
    fn test_callback_data_navigate_encode() {
        let data = StatsCallbackData::Navigate {
            community_id: 42,
            period: StatsPeriod::AllTime,
            view: StatsView::Summary,
            page: 3,
        };
        assert_eq!(data.encode(), "sn:42:a:s:3");
    }

    #[test]
    fn test_callback_data_select_community_parse() {
        let encoded = "sc:42";
        let parsed = StatsCallbackData::parse(encoded).expect("should parse");
        assert_eq!(
            parsed,
            StatsCallbackData::SelectCommunity { community_id: 42 }
        );
    }

    #[test]
    fn test_callback_data_select_period_parse() {
        let encoded = "sp:42:w";
        let parsed = StatsCallbackData::parse(encoded).expect("should parse");
        assert_eq!(
            parsed,
            StatsCallbackData::SelectPeriod {
                community_id: 42,
                period: StatsPeriod::ThisWeek,
            }
        );
    }

    #[test]
    fn test_callback_data_navigate_parse() {
        let encoded = "sn:42:a:s:3";
        let parsed = StatsCallbackData::parse(encoded).expect("should parse");
        assert_eq!(
            parsed,
            StatsCallbackData::Navigate {
                community_id: 42,
                period: StatsPeriod::AllTime,
                view: StatsView::Summary,
                page: 3,
            }
        );
    }

    #[test]
    fn test_callback_data_roundtrip() {
        let cases = vec![
            StatsCallbackData::SelectCommunity { community_id: 42 },
            StatsCallbackData::SelectPeriod {
                community_id: 42,
                period: StatsPeriod::ThisWeek,
            },
            StatsCallbackData::Navigate {
                community_id: 42,
                period: StatsPeriod::AllTime,
                view: StatsView::Summary,
                page: 3,
            },
        ];
        for case in cases {
            let encoded = case.encode();
            let parsed = StatsCallbackData::parse(&encoded).expect("should parse");
            assert_eq!(case, parsed);
        }
    }

    #[test]
    fn test_callback_data_fits_64_bytes() {
        // Worst case: large community_id, all-time, summary, large page
        let worst = StatsCallbackData::Navigate {
            community_id: 9999999999,
            period: StatsPeriod::AllTime,
            view: StatsView::Summary,
            page: 999,
        };
        let encoded = worst.encode();
        assert!(
            encoded.len() <= 64,
            "callback data exceeds 64 bytes: {} (len={})",
            encoded,
            encoded.len()
        );
    }

    #[test]
    fn test_callback_data_invalid_returns_none() {
        assert!(StatsCallbackData::parse("").is_none());
        assert!(StatsCallbackData::parse("invalid").is_none());
        assert!(StatsCallbackData::parse("sc:").is_none());
        assert!(StatsCallbackData::parse("sc:notanumber").is_none());
        assert!(StatsCallbackData::parse("sp:42").is_none());
        assert!(StatsCallbackData::parse("sp:42:x").is_none());
        assert!(StatsCallbackData::parse("sn:42:t:c").is_none());
        assert!(StatsCallbackData::parse("sn:42:t:c:notapage").is_none());
    }
}
