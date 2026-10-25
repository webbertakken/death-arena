//! Game audio: decodes sound assets and plays them through kira's mixer.
//!
//! Sounds load asynchronously through Bevy's asset server, so a sound asked for
//! before its asset is decoded waits in a queue and starts the frame it becomes
//! ready. Callers hold a [`SoundInstance`] to steer a sound (its volume) whether
//! it has started yet or not.

use bevy::asset::{AssetLoader, LoadContext, LoadState, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::{BoxedFuture, HashMap};
use kira::backend::Backend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend};
use std::io::Cursor;
use std::time::Duration;

pub use kira::{Easing, Tween};

#[cfg(test)]
mod tests;

/// Opens the audio output and plays [`Sound`] assets (`.ogg`, `.mp3`) on it.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<Sound>()
            .init_asset_loader::<SoundLoader>()
            .insert_non_send_resource(AudioOutput::open())
            .add_system(start_ready_sounds);
    }
}

/// A fully decoded sound, ready to play.
#[derive(TypeUuid)]
#[uuid = "8f0c2b7e-5d3a-4c1e-9b6f-2a4d7e1c9f30"]
pub struct Sound(StaticSoundData);

#[derive(Default)]
struct SoundLoader;

impl AssetLoader for SoundLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let data = StaticSoundData::from_cursor(Cursor::new(bytes.to_vec()))?;
            load_context.set_default_asset(LoadedAsset::new(Sound(data)));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ogg", "mp3"]
    }
}

/// How a sound plays: its loudness, whether it loops and how it fades in.
#[derive(Clone, Copy, Debug)]
pub struct Playback {
    /// Linear amplitude, `0.0` (silent) to `1.0` (as recorded).
    pub volume: f32,
    pub looped: bool,
    pub fade_in: Option<Tween>,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looped: false,
            fade_in: None,
        }
    }
}

/// Identifies one requested playback of a sound, started or still queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundInstance(u64);

/// Whether a queued sound's asset can be played yet.
pub enum SoundReadiness<'a> {
    Loading,
    Ready(&'a StaticSoundData),
    Failed,
}

struct QueuedSound {
    instance: SoundInstance,
    sound: Handle<Sound>,
    playback: Playback,
}

/// The audio device and every sound requested on it.
///
/// Without a device (none present, or the platform refused it) requests are
/// still accepted and simply never sound; [`AudioOutput::open`] logs why.
pub struct AudioOutput<B: Backend = DefaultBackend> {
    manager: Option<AudioManager<B>>,
    next_instance: u64,
    queued: Vec<QueuedSound>,
    playing: HashMap<SoundInstance, StaticSoundHandle>,
}

impl AudioOutput<DefaultBackend> {
    fn open() -> Self {
        match AudioManager::new(AudioManagerSettings::default()) {
            Ok(manager) => {
                info!("Audio output opened");
                Self::with_manager(Some(manager))
            }
            Err(error) => {
                error!("Audio output unavailable, the game runs without sound: {error}");
                Self::with_manager(None)
            }
        }
    }
}

impl<B: Backend> AudioOutput<B> {
    fn with_manager(manager: Option<AudioManager<B>>) -> Self {
        Self {
            manager,
            next_instance: 0,
            queued: Vec::new(),
            playing: HashMap::default(),
        }
    }

    /// Plays `sound` once its asset is decoded.
    pub fn play(&mut self, sound: Handle<Sound>, playback: Playback) -> SoundInstance {
        let instance = SoundInstance(self.next_instance);
        self.next_instance += 1;
        self.queued.push(QueuedSound {
            instance,
            sound,
            playback,
        });
        instance
    }

    /// Moves `instance` to `volume` (linear amplitude) over `fade`.
    pub fn set_volume(&mut self, instance: SoundInstance, volume: f32, fade: Duration) {
        if let Some(handle) = self.playing.get_mut(&instance) {
            handle.set_volume(
                amplitude_to_decibels(volume),
                Tween {
                    duration: fade,
                    ..Tween::default()
                },
            );
        } else if let Some(queued) = self.queued.iter_mut().find(|q| q.instance == instance) {
            queued.playback.volume = volume;
        }
    }

    /// Starts every queued sound whose asset is ready; drops those that failed to load.
    pub fn start_ready<'a>(&mut self, readiness: impl Fn(&Handle<Sound>) -> SoundReadiness<'a>) {
        let mut still_loading = Vec::new();
        for queued in std::mem::take(&mut self.queued) {
            match readiness(&queued.sound) {
                SoundReadiness::Loading => still_loading.push(queued),
                SoundReadiness::Failed => {
                    error!(
                        "Sound {:?} failed to load; it will not play",
                        queued.instance
                    );
                }
                SoundReadiness::Ready(data) => self.start(queued, data.clone()),
            }
        }
        self.queued = still_loading;
    }

    fn start(&mut self, queued: QueuedSound, data: StaticSoundData) {
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        let playback = queued.playback;
        let data = data
            .volume(amplitude_to_decibels(playback.volume))
            .fade_in_tween(playback.fade_in);
        let data = if playback.looped {
            data.loop_region(..)
        } else {
            data
        };
        match manager.play(data) {
            Ok(handle) => {
                self.playing.insert(queued.instance, handle);
            }
            Err(error) => error!("Sound {:?} could not start: {error}", queued.instance),
        }
    }

    /// Requests that have not started yet.
    #[cfg(test)]
    const fn queued_count(&self) -> usize {
        self.queued.len()
    }

    #[cfg(test)]
    fn playing_handle(&self, instance: SoundInstance) -> Option<&StaticSoundHandle> {
        self.playing.get(&instance)
    }
}

/// kira mixes in decibels; the game speaks in linear amplitude.
/// Anything at or below kira's silence floor is silence.
#[must_use]
pub fn amplitude_to_decibels(amplitude: f32) -> Decibels {
    if amplitude <= 0.0 {
        return Decibels::SILENCE;
    }
    Decibels((20.0 * amplitude.log10()).max(Decibels::SILENCE.0))
}

fn start_ready_sounds(
    mut output: NonSendMut<AudioOutput>,
    sounds: Res<Assets<Sound>>,
    asset_server: Res<AssetServer>,
) {
    output.start_ready(|handle| match sounds.get(handle) {
        Some(sound) => SoundReadiness::Ready(&sound.0),
        None if asset_server.get_load_state(handle) == LoadState::Failed => SoundReadiness::Failed,
        None => SoundReadiness::Loading,
    });
}
