use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat, TextureUsages};

use bevy::time::common_conditions::on_timer;
use ffmpeg_next as ffmpeg;

use ffmpeg::format::{Pixel, input};
use ffmpeg::frame::Video;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};

pub struct FfmpegPlugin;

impl Plugin for FfmpegPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<VideoResource>()
            .add_systems(Startup, (initialize_ffmpeg))
            .add_systems(
                Update,
                play_video.run_if(on_timer(Duration::from_micros(33333))),
            );
    }
}

pub fn make_video(
    path: &str,
    image_handle: Handle<Image>,
    mut video_resource: NonSendMut<VideoResource>,
    entity: Entity,
) -> VideoPlayer {
    println!("{:?}", path);
    let (video_player, video_player_non_send) = VideoPlayer::new(path, image_handle).unwrap();

    // let entity = commands
    //     .spawn(Sprite::from_image(video_player.image_handle.clone()))
    //     .insert(video_player.clone())
    //     .id();
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
    pub video_players: HashMap<Entity, VideoPlayerNonSendData>,
}

struct VideoPlayerNonSendData {
    decoder: ffmpeg::decoder::Video,
    input_context: ffmpeg::format::context::Input,
    scaler_context: Context,
}

#[derive(Component, Clone)]
pub struct VideoPlayer {
    pub image_handle: Handle<Image>,
    pub video_stream_index: usize,
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
            },
            VideoPlayerNonSendData {
                decoder,
                input_context,
                scaler_context,
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
        let video_player_non_send = video_resource.video_players.get_mut(&entity).unwrap();
        // read packets from stream until complete frame received
        while let Some((stream, packet)) = video_player_non_send.input_context.packets().next() {
            // check if packets is for the selected video stream
            if stream.index() == video_player.video_stream_index {
                // pass packet to decoder
                video_player_non_send.decoder.send_packet(&packet).unwrap();
                let mut decoded = Video::empty();
                // check if complete frame was received
                if let Ok(()) = video_player_non_send.decoder.receive_frame(&mut decoded) {
                    let mut rgb_frame = Video::empty();
                    // run frame through scaler for color space conversion
                    video_player_non_send
                        .scaler_context
                        .run(&decoded, &mut rgb_frame)
                        .unwrap();
                    // update data of image texture
                    let image = images.get_mut(&video_player.image_handle).unwrap();

                    if let Some(id) = image.data.as_mut() {
                        id.copy_from_slice(rgb_frame.data(0));
                    }
                    return;
                }
            }
        }
        // no frame received
        // signal end of playback to decoder
        match video_player_non_send.decoder.send_eof() {
            Err(ffmpeg::Error::Eof) => {}
            other => other.unwrap(),
        }
    }
}
