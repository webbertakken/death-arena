use super::*;
use kira::backend::mock::{MockBackend, MockBackendSettings};
use kira::sound::static_sound::StaticSoundSettings;
use kira::sound::PlaybackState;
use kira::Frame;
use std::sync::Arc;

fn mock_output() -> AudioOutput<MockBackend> {
    let manager = AudioManager::new(AudioManagerSettings::<MockBackend> {
        backend_settings: MockBackendSettings::default(),
        ..AudioManagerSettings::default()
    })
    .expect("the mock backend always opens");
    AudioOutput::with_manager(Some(manager))
}

fn one_second_of_silence() -> StaticSoundData {
    StaticSoundData {
        sample_rate: 100,
        frames: Arc::from(vec![Frame::ZERO; 100]),
        settings: StaticSoundSettings::default(),
        slice: None,
    }
}

fn loading(_: &Handle<Sound>) -> SoundReadiness<'static> {
    SoundReadiness::Loading
}

fn failed(_: &Handle<Sound>) -> SoundReadiness<'static> {
    SoundReadiness::Failed
}

#[test]
fn full_amplitude_is_zero_decibels() {
    assert!((amplitude_to_decibels(1.0).0 - 0.0).abs() < 1e-6);
}

#[test]
fn half_amplitude_is_about_minus_six_decibels() {
    assert!((amplitude_to_decibels(0.5).0 - -6.0206).abs() < 1e-3);
}

#[test]
fn zero_and_negative_amplitude_are_silence() {
    assert_eq!(amplitude_to_decibels(0.0), Decibels::SILENCE);
    assert_eq!(amplitude_to_decibels(-1.0), Decibels::SILENCE);
}

#[test]
fn amplitude_below_the_floor_clamps_to_silence() {
    assert_eq!(amplitude_to_decibels(0.000_01), Decibels::SILENCE);
}

#[test]
fn each_request_gets_its_own_instance() {
    let mut output = mock_output();

    let first = output.play(Handle::default(), Playback::default());
    let second = output.play(Handle::default(), Playback::default());

    assert_ne!(first, second);
}

#[test]
fn a_sound_waits_while_its_asset_loads() {
    let mut output = mock_output();
    let instance = output.play(Handle::default(), Playback::default());

    output.start_ready(loading);

    assert_eq!(output.queued_count(), 1);
    assert!(output.playing_handle(instance).is_none());
}

#[test]
fn a_sound_starts_once_its_asset_is_ready() {
    let mut output = mock_output();
    let instance = output.play(Handle::default(), Playback::default());

    let data = one_second_of_silence();
    output.start_ready(|_| SoundReadiness::Ready(&data));

    assert_eq!(output.queued_count(), 0);
    let handle = output.playing_handle(instance).expect("started");
    assert_eq!(handle.state(), PlaybackState::Playing);
}

#[test]
fn a_sound_whose_asset_failed_is_dropped() {
    let mut output = mock_output();
    let instance = output.play(Handle::default(), Playback::default());

    output.start_ready(failed);

    assert_eq!(output.queued_count(), 0);
    assert!(output.playing_handle(instance).is_none());
}

#[test]
fn without_a_device_ready_sounds_leave_the_queue_silently() {
    let mut output = AudioOutput::<MockBackend>::with_manager(None);
    let instance = output.play(Handle::default(), Playback::default());

    let data = one_second_of_silence();
    output.start_ready(|_| SoundReadiness::Ready(&data));

    assert_eq!(output.queued_count(), 0);
    assert!(output.playing_handle(instance).is_none());
}

#[test]
fn a_volume_set_before_start_is_kept_for_the_start() {
    let mut output = mock_output();
    let instance = output.play(Handle::default(), Playback::default());

    output.set_volume(instance, 0.25, Duration::ZERO);

    assert!((output.queued[0].playback.volume - 0.25).abs() < f32::EPSILON);
}

#[test]
fn a_volume_change_reaches_a_playing_sound() {
    let mut output = mock_output();
    let instance = output.play(
        Handle::default(),
        Playback {
            looped: true,
            ..Playback::default()
        },
    );
    let data = one_second_of_silence();
    output.start_ready(|_| SoundReadiness::Ready(&data));

    output.set_volume(instance, 0.2, Duration::from_millis(1));

    assert_eq!(
        output
            .playing_handle(instance)
            .map(StaticSoundHandle::state),
        Some(PlaybackState::Playing)
    );
}

#[test]
fn a_volume_change_for_an_unknown_instance_changes_nothing() {
    let mut output = mock_output();
    let instance = output.play(Handle::default(), Playback::default());

    output.set_volume(SoundInstance(99), 0.0, Duration::ZERO);

    assert!((output.queued[0].playback.volume - 1.0).abs() < f32::EPSILON);
    assert!(output.playing_handle(instance).is_none());
}
