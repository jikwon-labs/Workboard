// Condition filters: the choices for each field, matching a task against a
// row, a group or the whole filter, and applying a saved filter.

use super::*;

pub const DUE_FILTERS: [&str; 6] = [
    "Any time",
    "Overdue",
    "Today",
    "This week",
    "Next week",
    "No due date",
];

/// Choices for the start date filter (index = stored value).
pub const START_FILTERS: [&str; 6] = [
    "Any time",
    "Started",
    "Today",
    "This week",
    "Next week",
    "No start date",
];

/// Choices for the date created / updated filters (index = stored value).
pub const AGE_FILTERS: [&str; 5] = [
    "Any time",
    "Today",
    "Yesterday",
    "Last 7 days",
    "Last 30 days",
];

impl FilterField {
    pub const ALL: [FilterField; 10] = [
        FilterField::Status,
        FilterField::Priority,
        FilterField::Assignee,
        FilterField::DueDate,
        FilterField::Tags,
        FilterField::StartDate,
        FilterField::Created,
        FilterField::Updated,
        FilterField::CreatedBy,
        FilterField::HasSubtasks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FilterField::Status => "Status",
            FilterField::Priority => "Priority",
            FilterField::Assignee => "Assignee",
            FilterField::DueDate => "Due date",
            FilterField::Tags => "Tags",
            FilterField::StartDate => "Start date",
            FilterField::Created => "Date created",
            FilterField::Updated => "Date updated",
            FilterField::CreatedBy => "Created by",
            FilterField::HasSubtasks => "Subtasks",
        }
    }

    /// Fields whose values exclude each other (one pick, not several).
    pub fn single(self) -> bool {
        matches!(
            self,
            FilterField::DueDate
                | FilterField::StartDate
                | FilterField::Created
                | FilterField::Updated
                | FilterField::HasSubtasks
        )
    }
}

impl FilterRow {
    pub fn active(&self) -> bool {
        self.field.is_some() && !self.values.is_empty()
    }

    /// The row's result, or None while it doesn't count yet.
    pub fn test(&self, t: &Task, data: &Data) -> Option<bool> {
        self.active().then(|| self.matches(t, data))
    }

    pub fn matches(&self, t: &Task, data: &Data) -> bool {
        let Some(f) = self.field else { return true };
        if self.values.is_empty() {
            return true;
        }
        let hit = match f {
            FilterField::Status => self.values.iter().any(|v| *v == data.status_of(t).name),
            FilterField::Priority => self
                .values
                .iter()
                .any(|v| v.parse::<u8>().ok() == Some(t.priority)),
            FilterField::Assignee => self.values.iter().any(|v| {
                if v == "none" {
                    t.assignees.is_empty()
                } else {
                    v.parse::<u64>().is_ok_and(|id| t.assignees.contains(&id))
                }
            }),
            FilterField::DueDate => self
                .values
                .iter()
                .any(|v| due_matches(v.parse().unwrap_or(0), t, data)),
            FilterField::Tags => self.values.iter().any(|v| t.tags.contains(v)),
            FilterField::StartDate => self
                .values
                .iter()
                .any(|v| start_matches(v.parse().unwrap_or(0), t.start, data)),
            FilterField::Created => self
                .values
                .iter()
                .any(|v| age_matches(v.parse().unwrap_or(0), t.created)),
            FilterField::Updated => self
                .values
                .iter()
                .any(|v| age_matches(v.parse().unwrap_or(0), t.updated)),
            // the creator is whoever wrote the first activity line ("created this task")
            FilterField::CreatedBy => self
                .values
                .iter()
                .any(|v| v.parse::<u64>().ok() == t.activity.first().map(|a| a.author)),
            FilterField::HasSubtasks => {
                let has = data
                    .tasks
                    .iter()
                    .any(|x| x.parent == Some(t.id) && !x.archived);
                self.values.iter().any(|v| (v == "yes") == has)
            }
        };
        hit != self.not
    }
}

impl FilterGroup {
    /// Rows that count (a field and at least one value).
    pub fn count(&self) -> usize {
        self.rows.iter().filter(|r| r.active()).count()
    }

    /// The group's result, or None when none of its rows count yet.
    pub fn test(&self, t: &Task, data: &Data) -> Option<bool> {
        (self.count() > 0).then(|| self.join.combine(self.rows.iter().map(|r| r.test(t, data))))
    }
}

fn start_matches(which: u8, start: Option<NaiveDate>, data: &Data) -> bool {
    let td = today();
    let week_start = data.week_start(td);
    match which {
        1 => start.is_some_and(|d| d <= td),
        2 => start == Some(td),
        3 => start.is_some_and(|d| d >= week_start && d < week_start + Duration::days(7)),
        4 => start.is_some_and(|d| {
            d >= week_start + Duration::days(7) && d < week_start + Duration::days(14)
        }),
        5 => start.is_none(),
        _ => true,
    }
}

fn age_matches(which: u8, ts: i64) -> bool {
    let day = local_date(ts);
    let age = (today() - day).num_days();
    match which {
        1 => age == 0,
        2 => age == 1,
        3 => age < 7,
        4 => age < 30,
        _ => true,
    }
}

fn due_matches(due: u8, t: &Task, data: &Data) -> bool {
    let td = today();
    let week_start = data.week_start(td);
    match due {
        1 => t.is_overdue() && !data.is_closed(t),
        2 => t.due == Some(td),
        3 => t.due.map_or(false, |d| {
            d >= week_start && d < week_start + Duration::days(7)
        }),
        4 => t.due.map_or(false, |d| {
            d >= week_start + Duration::days(7) && d < week_start + Duration::days(14)
        }),
        5 => t.due.is_none(),
        _ => true,
    }
}

/// Field conditions from the Filter panel: the rows and groups of the
/// condition builder, combined with `join`, plus fixed fields kept for code
/// that sets them directly (those must all match).
///
/// Saved as part of a location's view settings (`listview::ViewPrefs`,
/// under the names it always had there): the rows, join, groups and
/// Archived. The fixed fields are not saved.
#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskFilter {
    #[serde(rename = "filter_rows", default)]
    pub rows: Vec<FilterRow>,
    /// How the top-level rows and groups combine.
    #[serde(rename = "filter_join", default, skip_serializing_if = "Join::is_and")]
    pub join: Join,
    /// Groups of rows with their own And / Or (one level deep).
    #[serde(
        rename = "filter_groups",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub groups: Vec<FilterGroup>,
    #[serde(skip)]
    pub statuses: Vec<String>,
    #[serde(skip)]
    pub priorities: Vec<u8>,
    #[serde(skip)]
    pub assignees: Vec<u64>,
    #[serde(skip)]
    pub unassigned: bool,
    #[serde(skip)]
    pub tags: Vec<String>,
    #[serde(skip)]
    pub due: u8,
    #[serde(default)]
    pub archived: bool,
}

impl TaskFilter {
    /// Whether the saved part (rows, join, groups, Archived) differs.
    pub fn saved_differs(&self, other: &TaskFilter) -> bool {
        self.rows != other.rows
            || self.join != other.join
            || self.groups != other.groups
            || self.archived != other.archived
    }

    /// Takes the saved part from `saved`, keeping the fixed fields.
    pub fn take_saved(&mut self, saved: &TaskFilter) {
        self.rows = saved.rows.clone();
        self.join = saved.join;
        self.groups = saved.groups.clone();
        self.archived = saved.archived;
    }
}

impl TaskFilter {
    /// Conditions of the builder that count: rows, including those in groups.
    pub fn conditions(&self) -> usize {
        self.rows.iter().filter(|r| r.active()).count()
            + self.groups.iter().map(|g| g.count()).sum::<usize>()
    }

    pub fn count(&self) -> usize {
        self.conditions()
            + [
                !self.statuses.is_empty(),
                !self.priorities.is_empty(),
                !self.assignees.is_empty() || self.unassigned,
                !self.tags.is_empty(),
                self.due != 0,
                self.archived,
            ]
            .iter()
            .filter(|x| **x)
            .count()
    }

    pub fn matches(&self, t: &Task, data: &Data) -> bool {
        if t.archived != self.archived {
            return false;
        }
        let top = self
            .rows
            .iter()
            .map(|r| r.test(t, data))
            .chain(self.groups.iter().map(|g| g.test(t, data)));
        if !self.join.combine(top) {
            return false;
        }
        if !self.statuses.is_empty() && !self.statuses.iter().any(|s| *s == data.status_of(t).name)
        {
            return false;
        }
        if !self.priorities.is_empty() && !self.priorities.contains(&t.priority) {
            return false;
        }
        if (!self.assignees.is_empty() || self.unassigned)
            && !(self.assignees.iter().any(|a| t.assignees.contains(a))
                || (self.unassigned && t.assignees.is_empty()))
        {
            return false;
        }
        if !self.tags.is_empty() && !self.tags.iter().any(|g| t.tags.contains(g)) {
            return false;
        }
        due_matches(self.due, t, data)
    }
}

impl SavedFilter {
    /// Whether the builder shows exactly this saved filter.
    pub fn is_shown(&self, f: &TaskFilter) -> bool {
        self.join == f.join && self.rows == f.rows && self.groups == f.groups
    }

    /// Puts this filter's conditions in the builder (the other settings stay).
    pub fn apply_to(&self, f: &mut TaskFilter) {
        f.join = self.join;
        f.rows = self.rows.clone();
        f.groups = self.groups.clone();
    }
}
