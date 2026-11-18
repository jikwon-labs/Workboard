// Click-through tests for making a new workspace and deleting one.

use super::*;

/// Points HOME at a new folder for a test that saves workspace files; the
/// guard keeps the other such tests waiting until it's done.
fn own_home(tag: &str) -> std::sync::MutexGuard<'static, ()> {
    let guard = crate::app::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home =
        std::env::temp_dir().join(format!("wb-home-{}-{:x}", tag, crate::sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    guard
}

#[test]
fn new_workspace_on_this_computer_is_a_name_and_apps() {
    let _home = own_home("wizard");
    let mut h = Ui1::new();
    let before = h.app.ws_file.clone();
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 0, .. })
    ));
    assert!(h.has("Step 1 of 2"));
    // step 1: the name
    h.type_text("Side Biz");
    h.click("Next");
    assert!(h.has("Step 2 of 2"));
    // step 2: the apps on the left bar, the default ones already chosen
    h.click("Chat");
    h.click("AI");
    // nobody to invite to a workspace only here: no invite step, one button
    assert!(!h.has("Invite people to your workspace"));
    assert!(!h.has("Skip for now"));
    assert!(!h.has("Next"));
    h.click("Create");
    h.settle();
    assert!(h.app.modal.is_none(), "the dialog closes when it's made");
    assert_ne!(h.app.ws_file, before, "the new workspace is open");
    assert_eq!(h.app.data.workspace, "Side Biz");
    assert_eq!(
        h.app.data.extra.rail_items,
        vec!["Planner", "Teams", "Chat"],
        "the chosen apps, in the bar's order"
    );
    assert_eq!(h.app.data.members.len(), 1, "only me in the new one");
    let me = h.app.data.member(h.app.data.me).unwrap();
    assert_eq!(me.role, "Owner");
    // the workspace menu lists both
    let list = Data::workspaces();
    assert!(list.iter().any(|w| w.file == before));
    assert!(list
        .iter()
        .any(|w| w.name == "Side Biz" && w.file == h.app.ws_file));
}

#[test]
fn new_workspace_needs_a_name_and_goes_back() {
    let _home = own_home("wizard-skip");
    let mut h = Ui1::new();
    let members_before = h.app.data.members.len();
    h.click("Workspace menu");
    h.click("Create Workspace");
    // no name yet: Next does nothing
    h.click("Next");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 0, .. })
    ));
    h.type_text("Acme");
    // Enter in the name moves on, like Next
    h.key(egui::Key::Enter);
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 1, .. })
    ));
    // back a step: what was typed is still there
    h.click("Back");
    match &h.app.modal {
        Some(Modal::NewWorkspace { step, name, .. }) => {
            assert_eq!(*step, 0);
            assert_eq!(name, "Acme");
        }
        _ => panic!("the dialog is still open"),
    }
    h.click("Next");
    h.click("Create");
    h.settle();
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.data.workspace, "Acme");
    assert_eq!(h.app.data.members.len(), 1, "only me in the new one");
    assert!(members_before > 1, "the seed had others");
    assert_eq!(
        h.app.data.extra.rail_items,
        Extra::default().rail_items,
        "untouched apps stay the default"
    );
}

#[test]
fn a_new_workspace_can_go_on_the_team_server_or_stay_here() {
    let _home = own_home("wizard-server");
    let mut h = Ui1::new();
    // this workspace is on a team server, signed in with an account (a
    // device of its own: a test that stops sharing as "dev1" forgets that
    // token while this one runs)
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-wizard".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.account = Some("me@team.io".into());
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    together::save_token("127.0.0.1:1", "dev-wizard", "tok");
    let before = h.app.ws_file.clone();

    // on the server is the choice it starts with
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(h.has("On the team server (127.0.0.1:1)"));
    assert!(h.has("On this computer only"));
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace {
            on_server: true,
            ..
        })
    ));
    h.type_text("Side Biz");
    h.click("Next");
    h.click("Next");
    assert!(h.has("The server emails each of them an invite code once the workspace is up."));
    h.type_text("helper@team.io");
    h.click("Create and invite");
    h.settle();
    assert!(h.app.modal.is_none());
    assert!(
        h.app.making_ws.is_some(),
        "asked the server to make it; it opens when the server answers"
    );
    assert_eq!(h.app.ws_file, before, "nothing opens before that");
    h.app.making_ws = None;

    // or only on this computer, as before
    h.click("Workspace menu");
    h.click("Create Workspace");
    h.click("On this computer only");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace {
            on_server: false,
            ..
        })
    ));
    h.type_text("Notes");
    h.click("Next");
    assert!(h.has("Step 2 of 2"), "no invite step when it stays here");
    assert!(!h.has("Invite people to your workspace"));
    h.click("Create");
    h.settle();
    assert!(h.app.making_ws.is_none());
    assert_ne!(h.app.ws_file, before);
    assert_eq!(h.app.data.workspace, "Notes");
    assert!(h.app.data.extra.shared.is_none());
}

#[test]
fn only_the_first_workspaces_owner_or_admins_get_the_server_choice() {
    let _home = own_home("wizard-member");
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.account = Some("me@team.io".into());
    // a plain member makes it on this computer, as the server would refuse
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(!h.has("On the team server (127.0.0.1:1)") && !h.has("Where"));
    h.key(egui::Key::Escape);
    // so does the owner of another workspace on that server
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1/w/abc".into());
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(!h.has("Where"));
    h.key(egui::Key::Escape);
    // an admin of the first workspace has it
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1".into());
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(h.has("On the team server (127.0.0.1:1)"));
}

#[test]
fn a_workspace_on_this_computer_only_has_no_server_choice() {
    let _home = own_home("wizard-local");
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(!h.has("On this computer only"));
    assert!(!h.has("Where"));
}

#[test]
fn a_workspace_on_this_computer_is_deleted_after_typing_its_name() {
    let mut h = Ui1::new();
    let name = h.app.data.workspace.clone();
    let first_task = h.app.data.tasks[0].name.clone();
    h.app.open_settings("General");
    h.frame(vec![]);
    assert!(h.has("Everything in it is deleted from this computer."));
    h.click("Delete workspace...");
    assert!(matches!(h.app.modal, Some(Modal::DeleteWorkspace { .. })));
    assert!(h.has(&format!("Delete {}?", name)));
    // not before the name is typed
    h.click("Delete");
    assert!(matches!(h.app.modal, Some(Modal::DeleteWorkspace { .. })));
    assert!(h.app.data.tasks.iter().any(|t| t.name == first_task));
    h.click("Cancel");
    assert!(h.app.modal.is_none());
    assert!(h.app.data.tasks.iter().any(|t| t.name == first_task));

    h.app.open_settings("General");
    h.frame(vec![]);
    h.click("Delete workspace...");
    h.click("Workspace name to delete");
    h.type_text("not it");
    h.click("Delete");
    assert!(matches!(h.app.modal, Some(Modal::DeleteWorkspace { .. })));
    h.app.modal = Some(Modal::DeleteWorkspace {
        typed: String::new(),
    });
    h.frame(vec![]);
    h.click("Workspace name to delete");
    h.type_text(&name);
    h.click("Delete");
    assert!(
        !h.app.data.tasks.iter().any(|t| t.name == first_task),
        "its tasks are gone"
    );
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.clone()),
        Some(format!("Deleted {}", name))
    );
    // nothing left on this computer: the start screen, as on a first start
    assert!(h.app.login.is_some() && h.app.data.extra.welcome);
    assert!(h.app.modal.is_none());
}

#[test]
fn only_an_owner_deletes_a_team_server_workspace_and_never_the_first() {
    let mut h = Ui1::new();
    let shared = |server: &str| {
        Some(crate::sync::SharedCfg {
            folder: String::new(),
            device: "dev-del".into(),
            tag: 5,
            server: Some(server.into()),
        })
    };
    // a plain member: no way to delete it, and nothing said about it
    h.app.data.extra.shared = shared("127.0.0.1:1/w/abc");
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.app.open_settings("General");
    h.frame(vec![]);
    assert!(h.has("Workspace name"));
    assert!(!h.has("Delete workspace...") && !h.has("Delete workspace"));

    // the owner, signed in
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.frame(vec![]);
    assert!(h.has("It is deleted from the team server for everyone."));
    h.click("Delete workspace...");
    assert!(h.has("It is deleted from the team server for everyone: their apps lose it too. The server keeps a copy for 30 days, which whoever runs the server can bring back."));
    h.click("Cancel");

    // not signed in yet: who they are isn't known
    h.app.sync_rt.role = None;
    h.app.open_settings("General");
    h.frame(vec![]);
    assert!(h.has("Connect to the team server to delete this workspace."));
    assert!(!h.has("Delete workspace..."));

    // the server's first workspace stays
    h.app.data.extra.shared = shared("127.0.0.1:1");
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.frame(vec![]);
    assert!(h.has("This is the team server's first workspace. It can't be deleted."));
    assert!(!h.has("Delete workspace..."));

    // in a shared folder: this computer's copy
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: "/tmp/nowhere".into(),
        device: "dev-del".into(),
        tag: 5,
        server: None,
    });
    h.frame(vec![]);
    assert!(h.has(
        "This computer's copy is deleted. The shared folder and your teammates' copies stay."
    ));
}

#[test]
fn a_workspace_deleted_on_the_server_says_so_and_comes_off_this_computer() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-gone".into(),
        tag: 5,
        server: Some("127.0.0.1:1/w/abc".into()),
    });
    together::save_token("127.0.0.1:1/w/abc", "dev-gone", "token-1");
    let team_task = h.app.data.tasks[0].name.clone();
    h.app.sync_rt.net_status = Some(wb_net::Status::Gone(wb_net::WORKSPACE_GONE.into()));
    h.app.modal = Some(h.app.together_modal());
    h.frame(vec![]);
    h.frame(vec![]);
    let gone = wb_net::Status::Gone(wb_net::WORKSPACE_GONE.into());
    assert_eq!(
        together::net_status_text(true, Some(&gone), 0),
        "Deleted from the server"
    );
    assert!(h.has(wb_net::WORKSPACE_GONE));
    assert!(!h.has("Sign in"), "signing in again wouldn't help");
    h.click("Remove from this computer");
    assert!(h.app.data.extra.shared.is_none());
    assert!(!h.app.data.tasks.iter().any(|t| t.name == team_task));
    assert!(together::load_token("127.0.0.1:1/w/abc", "dev-gone").is_none());
    assert!(h.app.modal.is_none());
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|t| t.0.starts_with("Removed ")));
}
