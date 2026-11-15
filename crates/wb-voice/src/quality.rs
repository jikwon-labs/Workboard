// How good the video is, and how it adapts.
//
// Sending: the camera goes out as three simulcast layers (320, 640 and 1280
// pixels wide) and a shared screen as two (960 at 5 frames a second and full
// size, up to 1920, at 10). The server forwards each receiver the layer that
// fits (dynacast stops layers nobody watches). When this computer's connection
// gets poor, what is fed into the encoder shrinks too (`limits`), on top of
// WebRTC's own bandwidth adaptation (`degradation_preference`).
//
// Receiving: the app says how wide each picture is drawn, and `layer_for`
// turns that into the layer to ask the server for.

use livekit::options::{DegradationPreference, TrackPublishOptions, VideoEncoding, VideoPreset};
use livekit::prelude::TrackSource;
use livekit::track::VideoQuality;

/// What the camera is captured at (the top simulcast layer).
pub const CAMERA_WIDTH: u32 = 1280;
pub const CAMERA_FPS: u32 = 15;
/// A shared screen: sharper, slower.
pub const SCREEN_WIDTH: u32 = 1920;
pub const SCREEN_FPS: u32 = 10;

/// The lower camera layers (the top one is the captured size).
pub const CAMERA_LAYERS: [VideoPreset; 2] = [
    VideoPreset::new(320, 180, 150_000, 15.0),
    VideoPreset::new(640, 360, 500_000, 15.0),
];
/// The lower screen layer: half size, few frames (text stays readable large).
pub const SCREEN_LAYERS: [VideoPreset; 1] = [VideoPreset::new(960, 540, 400_000, 5.0)];

pub fn camera_options() -> TrackPublishOptions {
    TrackPublishOptions {
        source: TrackSource::Camera,
        simulcast: true,
        video_encoding: Some(VideoEncoding {
            max_bitrate: 1_500_000,
            max_framerate: CAMERA_FPS as f64,
        }),
        simulcast_layers: Some(CAMERA_LAYERS.to_vec()),
        // a face: give up some sharpness and some smoothness alike
        degradation_preference: Some(DegradationPreference::Balanced),
        ..Default::default()
    }
}

pub fn screen_options() -> TrackPublishOptions {
    TrackPublishOptions {
        source: TrackSource::Screenshare,
        simulcast: true,
        video_encoding: Some(VideoEncoding {
            max_bitrate: 2_500_000,
            max_framerate: SCREEN_FPS as f64,
        }),
        simulcast_layers: Some(SCREEN_LAYERS.to_vec()),
        // text on a screen: keep it sharp, drop frames instead
        degradation_preference: Some(DegradationPreference::MaintainResolution),
        ..Default::default()
    }
}

/// The layer to ask for when a picture is drawn `px` physical pixels wide.
pub fn layer_for(px: f32) -> VideoQuality {
    if px <= 360.0 {
        VideoQuality::Low
    } else if px <= 800.0 {
        VideoQuality::Medium
    } else {
        VideoQuality::High
    }
}

/// How this computer's connection is doing (from the server's connection quality).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Good,
    Poor,
    /// Hardly anything gets through.
    Bad,
}

impl From<livekit::prelude::ConnectionQuality> for Network {
    fn from(q: livekit::prelude::ConnectionQuality) -> Self {
        use livekit::prelude::ConnectionQuality::*;
        match q {
            Excellent | Good => Network::Good,
            Poor => Network::Poor,
            Lost => Network::Bad,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Camera,
    Screen,
}

/// The widest picture and the most frames a second to feed the encoder.
/// The camera gets smaller and slower; a screen keeps its size and slows down.
pub fn limits(kind: Kind, net: Network) -> (u32, u32) {
    match (kind, net) {
        (Kind::Camera, Network::Good) => (CAMERA_WIDTH, CAMERA_FPS),
        (Kind::Camera, Network::Poor) => (640, 10),
        (Kind::Camera, Network::Bad) => (320, 5),
        (Kind::Screen, Network::Good) => (SCREEN_WIDTH, SCREEN_FPS),
        (Kind::Screen, Network::Poor) => (SCREEN_WIDTH, 5),
        (Kind::Screen, Network::Bad) => (1280, 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use livekit::options::compute_video_encodings;

    /// (width of the layer, frames a second) for each layer sent, smallest first.
    fn layers(w: u32, h: u32, o: &TrackPublishOptions) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = compute_video_encodings(w, h, o)
            .iter()
            .map(|e| {
                (
                    (w as f64 / e.scale_resolution_down_by.unwrap_or(1.0)).round() as u32,
                    e.max_framerate.unwrap_or(0.0) as u32,
                )
            })
            .collect();
        v.sort();
        v
    }

    #[test]
    fn camera_goes_out_in_three_layers() {
        assert_eq!(
            layers(1280, 720, &camera_options()),
            vec![(320, 15), (640, 15), (1280, 15)]
        );
        assert_eq!(camera_options().source, TrackSource::Camera);
    }

    #[test]
    fn screen_goes_out_larger_and_slower() {
        assert_eq!(
            layers(1920, 1080, &screen_options()),
            vec![(960, 5), (1920, 10)]
        );
        let o = screen_options();
        assert_eq!(o.source, TrackSource::Screenshare);
        assert_eq!(
            o.degradation_preference,
            Some(DegradationPreference::MaintainResolution)
        );
        // a smaller screen still gets its own size as the top layer
        assert_eq!(layers(1440, 900, &o).last(), Some(&(1440, 10)));
    }

    #[test]
    fn receivers_ask_for_the_layer_that_fits_the_tile() {
        assert_eq!(layer_for(140.0), VideoQuality::Low);
        assert_eq!(layer_for(320.0), VideoQuality::Low);
        // a 320-point tile on a Retina screen is 640 pixels
        assert_eq!(layer_for(640.0), VideoQuality::Medium);
        assert_eq!(layer_for(1120.0), VideoQuality::High);
    }

    #[test]
    fn a_poor_connection_sends_less() {
        use livekit::prelude::ConnectionQuality as Q;
        assert_eq!(Network::from(Q::Excellent), Network::Good);
        assert_eq!(Network::from(Q::Poor), Network::Poor);
        assert_eq!(Network::from(Q::Lost), Network::Bad);
        let (w, fps) = limits(Kind::Camera, Network::Good);
        let (pw, pfps) = limits(Kind::Camera, Network::Poor);
        assert!(pw < w && pfps < fps);
        // a screen keeps its sharpness and slows down instead
        let (sw, sfps) = limits(Kind::Screen, Network::Good);
        let (spw, spfps) = limits(Kind::Screen, Network::Poor);
        assert!(spw == sw && spfps < sfps);
        assert!(sw > w && sfps < fps, "screens: larger, fewer frames");
    }
}
