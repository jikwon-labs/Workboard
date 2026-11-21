// Each hub's sidebar filter, kept apart per hub: coming back to a hub (Back,
// a search result, a form made from a + menu) finds the filter it was left
// on, whatever another hub showed meanwhile. Opening an app from the bar
// starts every hub afresh (`App::open_app`).

/// The filters of every hub.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct HubFilters {
    pub docs: DocFilter,
    pub dash: OwnFilter,
    pub wb: OwnFilter,
    pub forms: OwnFilter,
    pub teams: TeamsPage,
    pub sheets: SheetsTab,
}

/// The Docs sidebar's filters. The number is the sidebar's key for it
/// (Favorites came last, so it keeps the last one).
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DocFilter {
    #[default]
    All = 0,
    Mine = 1,
    Shared = 2,
    Private = 3,
    Meetings = 4,
    Wikis = 5,
    Archived = 6,
    Favorites = 7,
}

impl DocFilter {
    pub const ALL: [DocFilter; 8] = [
        DocFilter::All,
        DocFilter::Mine,
        DocFilter::Shared,
        DocFilter::Private,
        DocFilter::Meetings,
        DocFilter::Wikis,
        DocFilter::Archived,
        DocFilter::Favorites,
    ];

    pub fn key(self) -> u8 {
        self as u8
    }

    pub fn from_key(k: u8) -> Self {
        Self::ALL.get(k as usize).copied().unwrap_or_default()
    }
}

/// The Dashboards, Whiteboards and Forms sidebars' filters: All, My,
/// Favorites.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OwnFilter {
    #[default]
    All = 0,
    Mine = 1,
    Favorites = 2,
}

impl OwnFilter {
    pub const ALL: [OwnFilter; 3] = [OwnFilter::All, OwnFilter::Mine, OwnFilter::Favorites];

    pub fn key(self) -> u8 {
        self as u8
    }

    pub fn from_key(k: u8) -> Self {
        Self::ALL.get(k as usize).copied().unwrap_or_default()
    }
}

/// What the Teams hub shows: one of its sidebar's filters, or the page of
/// the team in `App::team_open`.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TeamsPage {
    #[default]
    All,
    People,
    OrgChart,
    Analytics,
    Mine,
    Team,
}

impl TeamsPage {
    const FILTERS: [TeamsPage; 5] = [
        TeamsPage::All,
        TeamsPage::People,
        TeamsPage::OrgChart,
        TeamsPage::Analytics,
        TeamsPage::Mine,
    ];

    /// The sidebar's key for the filter shown; None on a team's page.
    pub fn key(self) -> Option<u8> {
        Self::FILTERS
            .iter()
            .position(|p| *p == self)
            .map(|i| i as u8)
    }

    pub fn from_key(k: u8) -> Self {
        Self::FILTERS.get(k as usize).copied().unwrap_or_default()
    }
}

/// The Timesheets tabs.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SheetsTab {
    #[default]
    Mine,
    All,
}
