// Talking to workboard-server: the messages both sides send (JSON text over a
// WebSocket) and the app's side of the connection, which runs on its own
// thread so the window never waits for the network.
//
// Order of a connection: the app sends `hello` (with its device token, an
// invite code, an email and password, a password-reset code, or a claim on
// an empty server), the server answers `welcome` with the team's copy (and a
// new token when it issued one), then the app
// sends `push` with its shared copy whenever it changes and the server
// sends `update` to everyone when the team's copy changed. Both sides answer
// `ping` with `pong`. Owners and admins also send the access requests
// (`create_invite`, `set_role`, `revoke`, `access_list`). Anyone signed in
// may send `set_password` to set up or change their account.
//
// Deltas: when both sides say `deltas` (in hello and welcome), only what
// changed travels after the first whole copy. The app pushes the part of
// its copy that differs from what the server has (`wb_sync::delta`; merging
// it is the same as merging the whole copy, since a missing item never
// counts as deleted), and the server sends `delta` messages with the part
// of the team's copy that changed since the revision `base`. An app that
// missed a revision asks for the whole copy again (`resync`). Apps and
// servers from before deltas keep sending and getting whole copies.
//
// Accounts: an email and a password that sign a person in on any computer
// or phone. Someone joining with an invite code chooses the password then;
// the owner chooses one when setting up an empty server. A forgotten
// password is reset with a code the server emails (POST /auth/forgot).

mod addr;
mod bell;
mod client;
mod conn;
mod http;
mod protocol;
mod watch;
pub use addr::*;
pub use client::*;
pub use conn::{trust_certificate, Conn};
pub use http::{
    create_workspace, delete_workspace, download, download_update, forgot_password,
    list_workspaces, name_ok, remote_size, sha256_file, update_text, upload, Auth, FileError,
};
pub use protocol::*;
pub use watch::{Watch, Watcher};

#[cfg(test)]
mod tests;
