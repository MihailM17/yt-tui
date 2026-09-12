use crate::app::Video;

/// Mock home feed mirroring the screenshot layout (3x3 grid).
/// Replace with Innertube/RSS fetch later; same struct.
pub fn mock_videos() -> Vec<Video> {
    vec![
        v("mock1", "Grian's Reaction To Lifesteal's Flooded Nether", "FadedPhoenix", false, "63K views", "3 days ago", "1:43", 4, 11),
        v("mock2", "DHH on drama with Theo | Lex Fridman Podcast Clips", "Lex Clips", true, "47K views", "1 day ago", "3:23", 18, 22),
        v("mock3", "I Connected a Slushie Machine to my Computer", "Linus Tech Tips", true, "983K views", "16 hours ago", "24:15", 90, 33),
        v("mock4", "Realistic Week in the Life as a Software Engineer | 2 Jobs & Setup Upgrades", "Onners", false, "622 views", "19 hours ago", "25:47", 150, 44),
        v("mock5", "I Built a Hands Free Infinite Farm with no Raids! (Scrap Mechanic Chapter 2)", "kAN Gaming", true, "37K views", "14 hours ago", "37:39", 70, 55),
        v("mock6", "The Art of The Amazing Digital Circus", "GLITCH", true, "3.3M views", "14 hours ago", "2:59", 0, 66),
        v("mock7", "Remove silences across the timeline", "PewDiePie", true, "1.2M views", "2 days ago", "19:17", 170, 77),
        v("mock8", "The Best Camera for Vlogging in 2025?", "Bald and Bankrupt", true, "542K views", "1 day ago", "18:09", 8, 88),
        v("mock9", "Night is Not Black - Gameplay Trailer", "IGN", true, "1.1M views", "1 day ago", "13:12", 110, 99),
        v("mock10", "Operating Systems are Worse Than Ever", "Some Channel", false, "12K views", "5 hours ago", "11:02", 200, 111),
        v("mock11", "Scrap Mechanic Survival - Automated Farm Tour", "ScrapMan", false, "88K views", "4 days ago", "15:44", 60, 122),
        v("mock12", "DaVinci Resolve: Cut Page in 10 Minutes", "Video game development", false, "5K views", "1 week ago", "9:31", 210, 133),
    ]
}

#[allow(clippy::too_many_arguments)]
fn v(
    id: &str,
    title: &str,
    channel: &str,
    verified: bool,
    views: &str,
    age: &str,
    duration: &str,
    hue: u8,
    seed: u64,
) -> Video {
    Video {
        id: id.to_string(),
        title: title.to_string(),
        channel: channel.to_string(),
        verified,
        views: views.to_string(),
        age: age.to_string(),
        duration: duration.to_string(),
        hue,
        seed,
    }
}

pub fn chips() -> Vec<String> {
    [
        "All",
        "Gaming",
        "Podcasts",
        "Steven Universe",
        "Operating systems",
        "Scrap Mechanic",
        "Music",
        "AI",
        "DaVinci Resolve",
        "Video game development",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// (name, has_new_uploads)
pub fn subs() -> Vec<(String, bool)> {
    vec![
        ("ScrapMan".into(), true),
        ("Alpharad Replay".into(), true),
        ("kAN Gaming".into(), false),
        ("Grian".into(), true),
        ("jakkuh".into(), true),
        ("Two Much Grian".into(), true),
        ("SourceMaster_".into(), true),
    ]
}
