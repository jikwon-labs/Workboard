// A doc's Version history: earlier texts of the doc, who wrote them and
// when, kept inside the doc (`Doc::versions`) so they sync like any other
// item inside an item. A version is made when someone stops editing a doc
// (`DOC_VERSION_GAP` without a change, or leaving the doc), just before a
// teammate's change replaces text being written, and when an earlier
// version is restored. The iPhone app (WorkboardKit `DocVersions.swift`)
// follows the same rules.

use super::*;

/// Edits by one person closer together than this make one version.
pub const DOC_VERSION_GAP: i64 = 5 * 60;
/// The most versions a doc keeps (the newest ones).
pub const DOC_VERSIONS_KEPT: usize = 50;
/// Versions older than this go (the newest one always stays).
pub const DOC_VERSION_DAYS: i64 = 30;

/// One earlier text of a doc (see `Doc::versions`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DocVersion {
    pub id: u64,
    /// Who wrote it; 0 for text from before the doc had versions (or
    /// written by an app that doesn't keep them).
    #[serde(default)]
    pub by: u64,
    /// When it was written (seconds).
    pub at: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// Made by Restore this version: when the version it brought back was
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored: Option<i64>,
}

impl Doc {
    /// The versions, newest first.
    pub fn versions_newest(&self) -> Vec<&DocVersion> {
        let mut v: Vec<&DocVersion> = self.versions.iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse((x.at, x.id)));
        v
    }

    pub fn latest_version(&self) -> Option<&DocVersion> {
        self.versions.iter().max_by_key(|x| (x.at, x.id))
    }

    /// Whether the doc's text is `title` and `body`.
    pub fn reads(&self, title: &str, body: &str) -> bool {
        self.title == title && self.body == body
    }

    /// Drop versions past `DOC_VERSIONS_KEPT` and older than
    /// `DOC_VERSION_DAYS` before `now`, keeping the newest one.
    fn trim_versions(&mut self, now: i64) {
        let keep: Vec<u64> = self
            .versions_newest()
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                *i == 0 || (*i < DOC_VERSIONS_KEPT && v.at >= now - DOC_VERSION_DAYS * 24 * 3600)
            })
            .map(|(_, v)| v.id)
            .collect();
        self.versions.retain(|v| keep.contains(&v.id));
    }
}

impl Data {
    /// Keep doc `doc`'s text as it is now as a version written by `by` at
    /// `at` (see `keep_doc_text`).
    pub fn keep_doc_version(&mut self, doc: u64, by: u64, at: i64) -> bool {
        let Some(d) = self.docs.iter().find(|d| d.id == doc) else {
            return false;
        };
        let (title, body) = (d.title.clone(), d.body.clone());
        self.keep_doc_text(doc, by, at, &title, &body)
    }

    /// Keep `title` and `body` as a version of doc `doc` written by `by` at
    /// `at`. Nothing happens when the newest version already reads the
    /// same. A newer edit by the same person within `DOC_VERSION_GAP` of
    /// their last version updates that version instead of adding one.
    /// Returns whether the doc's versions changed.
    pub fn keep_doc_text(&mut self, doc: u64, by: u64, at: i64, title: &str, body: &str) -> bool {
        let Some(i) = self.docs.iter().position(|d| d.id == doc) else {
            return false;
        };
        let d = &self.docs[i];
        if d.latest_version()
            .is_some_and(|v| v.title == title && v.body == body)
        {
            return false;
        }
        let (title, body) = (title.to_string(), body.to_string());
        let grouped = d
            .latest_version()
            .filter(|v| {
                by != 0 && v.by == by && v.restored.is_none() && (at - v.at).abs() < DOC_VERSION_GAP
            })
            .map(|v| v.id);
        if let Some(vid) = grouped {
            let d = &mut self.docs[i];
            if let Some(v) = d.versions.iter_mut().find(|v| v.id == vid) {
                v.title = title;
                v.body = body;
                v.at = v.at.max(at);
            }
            return true;
        }
        let id = self.new_id();
        let d = &mut self.docs[i];
        d.versions.push(DocVersion {
            id,
            by,
            at,
            title,
            body,
            restored: None,
        });
        d.trim_versions(at);
        true
    }

    /// When someone starts changing doc `doc`, whose text was `title` and
    /// `body`: when that text isn't the newest version (the doc has none
    /// yet, or it was written by an app that doesn't keep them), keep it as
    /// one, so the change can be undone. It counts as kept at `at`, when
    /// the change started (not when the text was written, which may be
    /// longer ago than versions are kept). An empty doc needs none.
    pub fn keep_doc_start(&mut self, doc: u64, title: &str, body: &str, at: i64) -> bool {
        let Some(i) = self.docs.iter().position(|d| d.id == doc) else {
            return false;
        };
        if body.trim().is_empty()
            || self.docs[i]
                .latest_version()
                .is_some_and(|v| v.title == title && v.body == body)
        {
            return false;
        }
        let id = self.new_id();
        let d = &mut self.docs[i];
        d.versions.push(DocVersion {
            id,
            by: 0,
            at,
            title: title.into(),
            body: body.into(),
            restored: None,
        });
        d.trim_versions(at);
        true
    }

    /// Restore this version: the doc's text becomes version `version`'s, and
    /// that is kept as a new version by `by` (so a restore can be undone
    /// the same way). Text not kept yet is kept first. Returns false when
    /// there is no such version or the doc already reads the same.
    pub fn restore_doc_version(&mut self, doc: u64, version: u64, by: u64, now: i64) -> bool {
        let Some(d) = self.docs.iter().find(|d| d.id == doc) else {
            return false;
        };
        let Some(v) = d.versions.iter().find(|v| v.id == version).cloned() else {
            return false;
        };
        if d.reads(&v.title, &v.body) {
            return false;
        }
        let (title, body) = (d.title.clone(), d.body.clone());
        self.keep_doc_start(doc, &title, &body, now);
        let id = self.new_id();
        let Some(d) = self.docs.iter_mut().find(|d| d.id == doc) else {
            return false;
        };
        d.title = v.title.clone();
        d.body = v.body.clone();
        d.updated = now;
        d.versions.push(DocVersion {
            id,
            by,
            at: now,
            title: v.title,
            body: v.body,
            restored: Some(v.at),
        });
        d.trim_versions(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with(body: &str) -> (Data, u64) {
        let mut d = Data::seed();
        d.docs.clear();
        let id = d.new_id();
        d.docs.push(Doc {
            id,
            space: None,
            owner: 1,
            private: false,
            archived: false,
            meeting_note: false,
            parent: None,
            title: "Plan".into(),
            body: body.into(),
            created: 0,
            updated: 100,
            tags: vec![],
            wiki: None,
            views: vec![],
            versions: vec![],
        });
        (d, id)
    }

    #[test]
    fn edits_close_together_make_one_version_and_later_ones_another() {
        let (mut d, id) = data_with("a");
        assert!(d.keep_doc_start(id, "Plan", "a", 100));
        d.docs[0].body = "ab".into();
        assert!(d.keep_doc_version(id, 1, 1000));
        d.docs[0].body = "abc".into();
        assert!(d.keep_doc_version(id, 1, 1000 + DOC_VERSION_GAP - 1));
        assert_eq!(d.docs[0].versions.len(), 2, "same person within the gap");
        assert!(!d.keep_doc_version(id, 1, 5000), "nothing new to keep");
        d.docs[0].body = "abcd".into();
        assert!(d.keep_doc_version(id, 1, 1000 + 2 * DOC_VERSION_GAP));
        d.docs[0].body = "abcde".into();
        assert!(d.keep_doc_version(id, 2, 1000 + 2 * DOC_VERSION_GAP + 1));
        let v = d.docs[0].versions_newest();
        assert_eq!(
            v.iter()
                .map(|v| (v.by, v.body.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "abcde"), (1, "abcd"), (1, "abc"), (0, "a")]
        );
    }

    #[test]
    fn an_empty_doc_or_a_kept_text_needs_no_start() {
        let (mut d, id) = data_with("");
        assert!(!d.keep_doc_start(id, "Plan", "", 100));
        d.docs[0].body = "x".into();
        assert!(d.keep_doc_version(id, 1, 200));
        assert!(!d.keep_doc_start(id, "Plan", "x", 200));
        assert!(d.keep_doc_start(id, "Plan", "changed elsewhere", 300));
    }

    #[test]
    fn restoring_brings_the_text_back_as_a_new_version_that_can_be_undone() {
        let (mut d, id) = data_with("one");
        d.keep_doc_start(id, "Plan", "one", 100);
        d.docs[0].body = "two".into();
        d.keep_doc_version(id, 1, 1000);
        let first = d.docs[0].versions_newest()[1].id;
        assert!(d.restore_doc_version(id, first, 2, 2000));
        assert_eq!(d.docs[0].body, "one");
        assert_eq!(d.docs[0].updated, 2000);
        let newest = d.docs[0].versions_newest()[0].clone();
        assert_eq!((newest.by, newest.restored), (2, Some(100)));
        assert!(
            !d.restore_doc_version(id, first, 2, 2001),
            "already reads that"
        );
        // undo the restore by restoring the version before it
        let second = d.docs[0].versions_newest()[1].id;
        assert!(d.restore_doc_version(id, second, 2, 2100));
        assert_eq!(d.docs[0].body, "two");
        assert_eq!(d.docs[0].versions.len(), 4);
    }

    #[test]
    fn restoring_keeps_text_no_version_had_first() {
        let (mut d, id) = data_with("one");
        d.keep_doc_start(id, "Plan", "one", 100);
        let first = d.docs[0].versions[0].id;
        // written by an app that doesn't keep versions
        d.docs[0].body = "from an older app".into();
        d.docs[0].updated = 500;
        assert!(d.restore_doc_version(id, first, 1, 900));
        let v = d.docs[0].versions_newest();
        assert_eq!(v[1].body, "from an older app");
        assert_eq!((v[1].by, v[1].at), (0, 900));
    }

    #[test]
    fn old_and_extra_versions_go_but_the_newest_stays() {
        let (mut d, id) = data_with("0");
        for n in 1..=60 {
            d.docs[0].body = n.to_string();
            d.keep_doc_version(id, n % 2 + 1, n as i64 * 1000);
        }
        assert_eq!(d.docs[0].versions.len(), DOC_VERSIONS_KEPT);
        assert_eq!(d.docs[0].latest_version().unwrap().body, "60");
        let later = 60_000 + DOC_VERSION_DAYS * 24 * 3600 + 1;
        d.docs[0].body = "late".into();
        d.keep_doc_version(id, 1, later);
        assert_eq!(
            d.docs[0]
                .versions_newest()
                .iter()
                .map(|v| v.body.as_str())
                .collect::<Vec<_>>(),
            vec!["late"],
            "older than the days kept"
        );
        d.docs[0].versions[0].at = 0;
        d.docs[0].trim_versions(later * 10);
        assert_eq!(d.docs[0].versions.len(), 1, "the newest always stays");
    }

    #[test]
    fn older_files_read_and_empty_versions_stay_out_of_the_file() {
        let (d, _) = data_with("x");
        let v = serde_json::to_value(&d.docs[0]).unwrap();
        assert!(v.get("versions").is_none());
        let back: Doc = serde_json::from_value(v).unwrap();
        assert!(back.versions.is_empty());
    }
}
