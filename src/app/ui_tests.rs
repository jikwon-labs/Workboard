// Click-through tests: the whole interface runs in a 1440x900 window and
// receives real pointer moves, presses, releases and typed text at the
// on-screen position of each control (found by its name), then the result
// is checked. This is the same input path a mouse click takes.

use super::hubs::{DocFilter, OwnFilter, SheetsTab, TeamsPage};
use super::*;
use egui::Pos2;
use workboard_server::testing::temp_home;

mod ai;
mod archived;
mod audit_sweep;
mod chat;
mod comments;
mod docs;
mod empty_states;
mod forms;
mod goals;
mod harness;
mod hub_sidebars;
mod huddle;
mod i18n_tests;
mod inbox;
mod layout_tests;
mod look;
mod media;
mod meetings;
mod mini;
mod narrow;
mod navigation;
mod notify_settings;
mod people;
mod posts;
mod profile;
mod quit;
mod sign_in;
mod slack;
mod tasks;
mod template_tests;
mod views;
mod whiteboard;
mod wording_tests;
mod workspaces;

use harness::*;
// for the tests outside the click-through ones that look at painted shapes
pub(super) use harness::each_shape;
