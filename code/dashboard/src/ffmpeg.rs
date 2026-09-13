use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;

use bevy::time::common_conditions::on_timer;
use ffmpeg_next as ffmpeg;

use ffmpeg::format::{Pixel, input};
use ffmpeg::frame::Video;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};

pub struct FfmpegPlugin;

impl Plugin for FfmpegPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send::<VideoResource>()
            .add_systems(Startup, initialize_ffmpeg)
            .add_systems(
                Update,
                play_video.run_if(on_timer(Duration::from_micros(33333))),
            );
    }
}

pub fn make_video(
    path: &str,
    image_handle: Handle<Image>,
    video_resource: &mut VideoResource,
    entity: Entity,
) -> VideoPlayer {
    println!("{:?}", path);
    let (video_player, video_player_non_send) = VideoPlayer::new(path, image_handle).unwrap();

    video_resource
        .video_players
        .insert(entity, video_player_non_send);

    return video_player;
}

fn initialize_ffmpeg() {
    ffmpeg::init().unwrap();
}

// workaround non-send data not being allowed in components by using non-send resource instead
#[derive(Default)]
pub struct VideoResource {
    video_players: HashMap<Entity, VideoPlayerNonSendData>,
}

impl VideoResource {
    fn get_videoplayers(self) -> HashMap<Entity, VideoPlayerNonSendData> {
        return self.video_players;
    }

    /// Seek the video player for `entity` to the given timestamp in milliseconds.
    /// After seeking, the decoder is flushed so the next decoded frame will be near that position.
    pub fn seek(&mut self, entity: Entity, timestamp_ms: i64) {
        if let Some(data) = self.video_players.get_mut(&entity) {
            let clamped_ms = timestamp_ms.max(0);
            // avformat_seek_file with stream_index == -1 expects AV_TIME_BASE units (microseconds)
            let ts_us = clamped_ms * 1000;
            let _ = data.input_context.seek(ts_us, ..);
            data.decoder.flush();
            data.position_ms = clamped_ms;
        }
    }

    /// Last decoded presentation timestamp in milliseconds, if any frames
    /// have been decoded (or a seek has set the position explicitly).
    pub fn position_ms(&self, entity: Entity) -> Option<i64> {
        self.video_players.get(&entity).map(|d| d.position_ms)
    }

    /// Video duration in milliseconds, if known from the container.
    pub fn duration_ms(&self, entity: Entity) -> Option<i64> {
        self.video_players.get(&entity).and_then(|d| d.duration_ms)
    }
}

struct VideoPlayerNonSendData {
    decoder: ffmpeg::decoder::Video,
    input_context: ffmpeg::format::context::Input,
    scaler_context: Context,
    time_base: ffmpeg::Rational,
    /// Best-known playback position in ms (updated from decoded pts, or set by seek).
    position_ms: i64,
    /// Container duration in ms, if reported.
    duration_ms: Option<i64>,
}

#[derive(Component, Clone)]
pub struct VideoPlayer {
    pub image_handle: Handle<Image>,
    pub video_stream_index: usize,
    /// When `true`, the `play_video` system skips decoding for this player.
    pub paused: bool,
}

impl VideoPlayer {
    fn new<'a, P>(
        path: P,
        image_handle: Handle<Image>,
    ) -> Result<(VideoPlayer, VideoPlayerNonSendData), ffmpeg::Error>
    where
        P: AsRef<Path>,
    {
        let input_context = input(&path)?;

        // initialize decoder
        let input_stream = input_context
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input_stream.index();
        let time_base = input_stream.time_base();
        // Container duration is in AV_TIME_BASE units (microseconds), may be unavailable.
        let duration_ms = {
            let dur = input_context.duration();
            if dur > 0 { Some(dur / 1000) } else { None }
        };

        let context_decoder =
            ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
        let decoder = context_decoder.decoder().video()?;

        // initialize scaler
        let scaler_context = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGBA,
            decoder.width(),
            decoder.height(),
            Flags::BILINEAR,
        )?;

        println!("{:?} x {:?}", decoder.width(), decoder.height());

        Ok((
            VideoPlayer {
                image_handle,
                video_stream_index,
                paused: false,
            },
            VideoPlayerNonSendData {
                decoder,
                input_context,
                scaler_context,
                time_base,
                position_ms: 0,
                duration_ms,
            },
        ))
    }
}

fn play_video(
    mut video_player_query: Query<(&mut VideoPlayer, Entity)>,
    mut video_resource: NonSendMut<VideoResource>,
    mut images: ResMut<Assets<Image>>,
) {
    for (video_player, entity) in video_player_query.iter_mut() {
        // Skip decoding when the player is paused
        if video_player.paused {
            continue;
        }

        let Some(video_player_non_send) = video_resource.video_players.get_mut(&entity) else {
            // Component present but decoder not (yet) registered — e.g. before "Load".
            continue;
        };
        // read packets from stream until complete frame received
        while let Some((stream, packet)) = video_player_non_send.input_context.packets().next() {
            // check if packets is for the selected video stream
            if stream.index() == video_player.video_stream_index {
                // pass packet to decoder
                video_player_non_send.decoder.send_packet(&packet).unwrap();
                let mut decoded = Video::empty();
                // check if complete frame was received
                if let Ok(()) = video_player_non_send.decoder.receive_frame(&mut decoded) {
                    // Track the real video clock from the frame pts.
                    if let Some(pts) = decoded.pts() {
                        let seconds = pts as f64 * f64::from(video_player_non_send.time_base);
                        video_player_non_send.position_ms = (seconds * 1000.0) as i64;
                    }
                    let mut rgb_frame = Video::empty();
                    // run frame through scaler for color space conversion
                    video_player_non_send
                        .scaler_context
                        .run(&decoded, &mut rgb_frame)
                        .unwrap();
                    // update data of image texture
                    let Some(mut image) = images.get_mut(&video_player.image_handle) else {
                        return;
                    };

                    if let Some(id) = image.data.as_mut() {
                        let src = rgb_frame.data(0);
                        if id.len() == src.len() {
                            id.copy_from_slice(src);
                        }
                    }
                    return;
                }
            }
        }
        // no frame received: end of stream — loop back to the start so the
        // video keeps pace with the (looping) log playback instead of
        // freezing on the last frame.
        let _ = video_player_non_send.input_context.seek(0, ..);
        video_player_non_send.decoder.flush();
        video_player_non_send.position_ms = 0;
    }
}
