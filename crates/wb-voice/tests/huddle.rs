// A real huddle: a local livekit-server, a workboard-server that hands out
// call tokens, and two calls that send each other a tone and measure what
// arrives. Skipped (with a note) when livekit-server isn't installed.

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wb_net::{Client, Event, Msg, Role, SignIn, Status};
use wb_voice::{
    screen_key, Audio, Call, Event as VoiceEvent, Join, Person, Picture, Share, Video, ME,
};
use workboard_server::testing::{livekit_bin, start_livekit, wait_until, TestServer};

const SECRET: &str = "test-secret-test-secret-test-secret-0123";

/// A signed-in app connection, just enough to ask for codes and call tokens.
struct App {
    client: Client,
    data: Option<Value>,
    role: Option<Role>,
    code: Option<String>,
    token: Option<(String, String)>,
    denied: Vec<String>,
    connected: bool,
}

impl App {
    fn start(port: u16, sign_in: SignIn) -> App {
        let mut s = sign_in;
        s.device = format!("d{:x}", wb_sync::random_u64() as u32);
        // an app of this version (the server takes no changes from older ones)
        s.sync = Some(wb_sync::META_V);
        App {
            client: Client::start(&format!("127.0.0.1:{}", port), s, Arc::new(|| {})),
            data: None,
            role: None,
            code: None,
            token: None,
            denied: vec![],
            connected: false,
        }
    }

    fn pump(&mut self) {
        for e in self.client.events() {
            match e {
                Event::Status(s) => self.connected = s == Status::Connected,
                Event::Welcome { role, snapshot, .. } => {
                    self.role = Some(role);
                    if let Some(s) = snapshot {
                        self.data = Some(s);
                    }
                    if let Some(d) = &self.data {
                        self.client.push(d.clone());
                    }
                }
                Event::Update { data, .. } => self.data = Some(data),
                Event::Invite { code, .. } => self.code = Some(code),
                Event::CallToken { url, token, .. } => self.token = Some((url, token)),
                Event::Denied(why) | Event::CallDenied { why, .. } => self.denied.push(why),
                _ => {}
            }
        }
    }

    fn until(&mut self, what: &str, cond: impl Fn(&App) -> bool) {
        let held = wait_until(Duration::from_secs(20), Duration::from_millis(20), || {
            self.pump();
            cond(self)
        });
        if held {
            return;
        }
        panic!("timed out waiting for: {} (denied {:?})", what, self.denied);
    }

    fn invite(&mut self, member: u64) -> String {
        self.code = None;
        self.client.send(Msg::CreateInvite {
            member,
            role: Role::Member,
        });
        self.until("invite code", |a| a.code.is_some());
        self.code.clone().unwrap()
    }

    fn call_token(&mut self, channel: u64) -> Join {
        self.token = None;
        self.client.send(Msg::CallJoin { channel });
        self.until("call token", |a| a.token.is_some());
        let (url, token) = self.token.clone().unwrap();
        Join { url, token }
    }
}

struct Talker {
    call: Call,
    people: Vec<Person>,
    joined: bool,
    left: Option<Option<String>>,
    pictures: std::collections::HashMap<String, Picture>,
}

impl Talker {
    fn new(join: Join, hz: f32) -> Talker {
        Self::with_video(join, hz, [128, 128, 128])
    }

    fn with_video(join: Join, hz: f32, color: [u8; 3]) -> Talker {
        Talker {
            call: Call::start(
                join,
                Audio::Tone(hz),
                Video::Pattern(color),
                Arc::new(|| {}),
            ),
            people: vec![],
            joined: false,
            left: None,
            pictures: Default::default(),
        }
    }

    /// The other person's latest picture.
    fn their_picture(&self) -> Option<&Picture> {
        let id = &self.other()?.identity;
        self.pictures.get(id)
    }

    fn until(&mut self, what: &str, cond: impl Fn(&Talker) -> bool) {
        let until = Instant::now() + Duration::from_secs(25);
        while Instant::now() < until {
            for e in self.call.events() {
                match e {
                    VoiceEvent::Joined => self.joined = true,
                    VoiceEvent::People(p) => self.people = p,
                    VoiceEvent::Left(why) => self.left = Some(why),
                    VoiceEvent::Trouble(why) => panic!("trouble: {}", why),
                    VoiceEvent::ScreenEnded(why) => panic!("screen sharing ended: {:?}", why),
                }
            }
            for (id, p) in self.call.new_pictures() {
                match p {
                    Some(p) => {
                        self.pictures.insert(id, p);
                    }
                    None => {
                        self.pictures.remove(&id);
                    }
                }
            }
            if let Some(Some(why)) = &self.left {
                panic!("left the call while waiting for {}: {}", what, why);
            }
            if cond(self) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for: {} (people {:?})", what, self.people);
    }

    fn other(&self) -> Option<&Person> {
        self.people.iter().find(|p| !p.me)
    }
}

#[test]
fn two_people_hear_each_other_in_a_huddle() {
    let Some(bin) = livekit_bin() else {
        eprintln!("skipped: livekit-server isn't installed (brew install livekit)");
        return;
    };
    let lk = start_livekit(&bin, "testkey", SECRET).expect("start livekit-server");
    let lk_port = lk.port;
    let home = workboard_server::testing::temp_home("voice");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join(workboard_server::LIVEKIT_FILE),
        json!({"url": format!("ws://127.0.0.1:{}", lk_port), "key": "testkey", "secret": SECRET})
            .to_string(),
    )
    .unwrap();
    let server = TestServer::start_in(&home);

    // Ann sets up the workspace: #general has Ann and Bo, Cy isn't in it
    let workspace = json!({
        "workspace": "Team", "spaces": [], "tasks": [], "docs": [], "me": 1, "next_id": 1, "favorites": [],
        "members": [{"id": 1, "name": "Ann"}, {"id": 2, "name": "Bo"}, {"id": 3, "name": "Cy"}],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "members": [1, 2], "messages": []}],
        "extra": {"sync": {"stamps": {"1": 1, "2": 1, "3": 1, "10": 1}}}
    });
    let mut ann = App::start(
        server.port,
        SignIn {
            claim: Some(1),
            ..Default::default()
        },
    );
    ann.data = Some(workspace);
    ann.until("Ann is the owner", |a| a.role == Some(Role::Owner));
    ann.until("workspace on the server", |a| {
        a.data
            .as_ref()
            .is_some_and(|d| d["members"].as_array().is_some_and(|m| m.len() == 3))
    });
    let mut bo = App::start(
        server.port,
        SignIn {
            invite: Some(ann.invite(2)),
            ..Default::default()
        },
    );
    bo.until("Bo signed in", |b| b.connected);
    let mut cy = App::start(
        server.port,
        SignIn {
            invite: Some(ann.invite(3)),
            ..Default::default()
        },
    );
    cy.until("Cy signed in", |c| c.connected);

    // only people in the conversation get into its huddle
    cy.client.send(Msg::CallJoin { channel: 10 });
    cy.until("Cy is turned away", |c| {
        c.denied
            .iter()
            .any(|d| d.contains("not in that conversation"))
    });

    let mut a = Talker::with_video(ann.call_token(10), 440.0, [220, 30, 30]);
    let mut b = Talker::with_video(bo.call_token(10), 660.0, [30, 40, 220]);
    a.until("Ann joined", |t| t.joined);
    b.until("Bo joined", |t| t.joined);
    a.until("Ann hears Bo", |t| {
        t.other()
            .is_some_and(|p| p.name == "Bo" && p.member() == Some(2) && p.heard > 0.02)
    });
    b.until("Bo hears Ann", |t| {
        t.other().is_some_and(|p| p.name == "Ann" && p.heard > 0.02)
    });
    assert_eq!(a.people.len(), 2);

    // Bo mutes: Ann sees it and the sound stops
    b.call.set_muted(true);
    a.until("Ann sees Bo muted and silent", |t| {
        t.other().is_some_and(|p| p.muted && p.heard < 0.005)
    });
    b.call.set_muted(false);
    a.until("Bo unmuted and heard again", |t| {
        t.other().is_some_and(|p| !p.muted && p.heard > 0.02)
    });

    // video: each turns the camera on (a colored test picture) and sees the other's
    a.call.set_camera(true);
    b.call.set_camera(true);
    let reddish = |p: &Picture| {
        let c = p.average();
        c[0] > 150 && c[1] < 90 && c[2] < 90
    };
    let bluish = |p: &Picture| {
        let c = p.average();
        c[2] > 150 && c[0] < 90 && c[1] < 100
    };
    a.until("Ann sees herself", |t| {
        t.pictures.get(ME).is_some_and(|p| reddish(p))
    });
    // nothing drawn yet: the smallest layer (320 of the 1280 sent)
    b.until("Bo sees Ann's red picture, small", |t| {
        t.other().is_some_and(|p| p.camera)
            && t.their_picture()
                .is_some_and(|p| reddish(p) && p.width == 320 && p.height == 180)
    });
    // Bo draws Ann large: the full layer comes; small again: the small one
    let ann_id = b.other().unwrap().identity.clone();
    b.call
        .set_view_sizes([(ann_id.clone(), 1400.0)].into_iter().collect());
    b.until("Ann's picture at full size", |t| {
        t.their_picture()
            .is_some_and(|p| reddish(p) && p.width == 1280 && p.height == 720)
    });
    b.call
        .set_view_sizes([(ann_id.clone(), 600.0)].into_iter().collect());
    b.until("Ann's picture at the middle layer", |t| {
        t.their_picture().is_some_and(|p| p.width == 640)
    });
    a.until("Ann sees Bo's blue picture", |t| {
        t.their_picture().is_some_and(|p| bluish(p))
    });
    assert!(b.people.iter().find(|p| p.me).is_some_and(|p| p.camera));
    // pictures keep coming (the bar moves)
    let first = b.their_picture().unwrap().clone();
    b.until("Ann's picture changes", |t| {
        t.their_picture().is_some_and(|p| *p != first)
    });

    // Ann shares her screen (a green test picture): Bo sees it as a screen,
    // larger than video, next to her camera
    a.call.share_screen(Some(Share::Pattern([30, 200, 60])));
    let greenish = |p: &Picture| {
        let c = p.average();
        c[1] > 150 && c[0] < 100 && c[2] < 110
    };
    let ann_screen = screen_key(&ann_id);
    b.call.set_view_sizes(
        [(ann_id.clone(), 300.0), (ann_screen.clone(), 1800.0)]
            .into_iter()
            .collect(),
    );
    b.until("Bo sees Ann's screen", |t| {
        t.other().is_some_and(|p| p.screen && p.camera)
            && t.pictures
                .get(&ann_screen)
                .is_some_and(|p| greenish(p) && p.width == 1920 && p.height == 1080)
            && t.their_picture().is_some_and(|p| reddish(p))
    });
    a.until("Ann sees she is sharing", |t| {
        t.people.iter().any(|p| p.me && p.screen)
    });
    assert!(
        !a.pictures.keys().any(|k| k.ends_with("#screen")),
        "no picture of your own screen"
    );
    a.call.share_screen(None);
    b.until("Ann's screen is gone", |t| {
        t.other().is_some_and(|p| !p.screen) && !t.pictures.contains_key(&ann_screen)
    });
    assert!(b.their_picture().is_some(), "her camera stays");

    // Ann turns her camera off: her picture goes away for Bo
    a.call.set_camera(false);
    b.until("Ann's video is gone", |t| {
        t.other().is_some_and(|p| !p.camera) && t.their_picture().is_none()
    });
    a.until("and her own preview too", |t| !t.pictures.contains_key(ME));

    // Bo leaves with his camera still on: his picture goes away and stays away
    let bo_id = a.other().unwrap().identity.clone();
    assert!(a.pictures.contains_key(&bo_id));
    b.call.leave();
    a.until("only Ann is left", |t| t.people.len() == 1);
    a.until("Bo's picture is gone", |t| !t.pictures.contains_key(&bo_id));
    let settle = Instant::now() + Duration::from_secs(2);
    while Instant::now() < settle {
        a.until("still in the call", |_| true);
        assert!(
            !a.pictures.contains_key(&bo_id),
            "Bo's picture came back after he left"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(a);
    drop(server);
    let _ = std::fs::remove_dir_all(&home);
}
