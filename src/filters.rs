// The shape of a task filter as it is kept: condition rows and groups of
// rows with their own And / Or. Matching and the Filter panel are in
// src/app/parity.rs.

use serde::{Deserialize, Serialize};

/// A field a filter row can test.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FilterField {
    Status,
    Priority,
    Assignee,
    DueDate,
    Tags,
    StartDate,
    Created,
    Updated,
    CreatedBy,
    HasSubtasks,
}

/// One condition: a field, is / is not, and the values picked (any of them
/// matches). A row without a field or values doesn't count yet.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterRow {
    pub field: Option<FilterField>,
    pub not: bool,
    pub values: Vec<String>,
}

/// How the conditions at one level combine: all of them, or any of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Join {
    #[default]
    And,
    Or,
}

impl Join {
    pub fn is_and(&self) -> bool {
        *self == Join::And
    }

    pub fn flipped(self) -> Join {
        match self {
            Join::And => Join::Or,
            Join::Or => Join::And,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Join::And => "And",
            Join::Or => "Or",
        }
    }

    /// Combines the results of the conditions that count (`None` is an
    /// unfinished row or an empty group and is left out). When nothing
    /// counts, everything matches. Stops at the first result that decides.
    pub fn combine(self, results: impl Iterator<Item = Option<bool>>) -> bool {
        let mut it = results.flatten().peekable();
        if it.peek().is_none() {
            return true;
        }
        match self {
            Join::And => it.all(|x| x),
            Join::Or => it.any(|x| x),
        }
    }
}

/// Rows combined with their own And / Or, standing as one condition at the
/// top level. Groups don't nest further.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterGroup {
    #[serde(skip_serializing_if = "Join::is_and")]
    pub join: Join,
    pub rows: Vec<FilterRow>,
}
