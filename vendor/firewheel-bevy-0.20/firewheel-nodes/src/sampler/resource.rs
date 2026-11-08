use core::{
    num::{NonZeroU32, NonZeroUsize},
    ops::Range,
};
use firewheel_core::{
    collector::{ArcGc, OwnedGcUnsized},
    sample_resource::{SampleResource, SampleResourceInfo},
};

#[cfg(not(feature = "std"))]
use bevy_platform::prelude::Box;

/// A source of audio samples for a [`SamplerNode`](super::SamplerNode).
pub enum SamplerNodeResource {
    /// A resource of audio samples where the entire contents of the sample are
    /// already loaded into memory.
    ///
    /// Prefer this for resources which are less than 20 or so seconds long
    /// (i.e. sound effects).
    InMemory(ArcGc<dyn SampleResource + Send + Sync + 'static>),

    /// NOT IMPLEMENTED YET! Will lead to a panic if used.
    ///
    /// A resource of audio samples that are streamed from disk or over a network.
    ///
    /// Prefer this for resources which are greater than 20 or so seconds long
    /// (i.e. music tracks and ambience).
    ///
    /// This uses considerably less memory, but requires a more complicated setup.
    /// It also has the potential to run into cache misses if the playhead is moved
    /// to a region that hasn't been loaded yet, or if the stream fails to send
    /// enough samples in time.
    Streamed(OwnedGcUnsized<dyn StreamedSample>),
}

impl SamplerNodeResource {
    pub fn from_sample<T: SampleResource + Send + Sync + 'static>(sample: T) -> Self {
        Self::InMemory(sample.into())
    }

    pub fn from_streamed<T: StreamedSample>(sample: T) -> Self {
        Self::Streamed(OwnedGcUnsized::new_unsized(Box::new(sample)))
    }

    /// The number of channels in this resource.
    pub fn num_channels(&self) -> NonZeroUsize {
        match self {
            Self::InMemory(s) => s.num_channels(),
            Self::Streamed(s) => s.num_channels(),
        }
    }

    /// The length of this resource in samples (of a single channel of audio).
    ///
    /// Not to be confused with video frames.
    pub fn len_frames(&self) -> u64 {
        match self {
            Self::InMemory(s) => s.len_frames(),
            Self::Streamed(s) => s.len_frames(),
        }
    }

    /// The sample rate of this resource.
    ///
    /// Returns `None` if the sample rate is unknown.
    pub fn sample_rate(&self) -> Option<NonZeroU32> {
        match self {
            Self::InMemory(s) => s.sample_rate(),
            Self::Streamed(s) => s.sample_rate(),
        }
    }

    /// Fill the given buffers with audio data starting from the given
    /// starting frame in the resource.
    ///
    /// * `out_buffer` - The buffers to fill with data. If the length of `buffers`
    ///   is greater than the number of channels in this resource, then ignore
    ///   the extra buffers.
    /// * `out_buffer_range` - The range inside each buffer slice in which to
    ///   fill with data. Do not fill any data outside of this range.
    /// * `start_frame` - The sample (of a single channel of audio) in the
    ///   resource at which to start copying from. Not to be confused with video
    ///   frames.
    /// * `speed` - The speed at which playback is occurring, where `1.0` is
    ///   playing at the sample rate of this resource, `0.5` is playing at half
    ///   the sample rate, and `2.0` is playing at twice the sample rate.
    ///
    /// Returns the number of frames that were successfully filled. This may
    /// be less than the length of `out_buffer_range` if the range is all or
    /// partly out of bounds of the resource, or if a cache miss occurred.
    /// Any frames that were not successfully filled will be left untouched.
    pub fn fill_buffers(
        &mut self,
        out_buffer: &mut [&mut [f32]],
        out_buffer_range: Range<usize>,
        start_frame: u64,
        speed: f64,
        is_playing_backwards: bool,
    ) -> usize {
        match self {
            SamplerNodeResource::InMemory(s) => {
                s.fill_buffers(out_buffer, out_buffer_range.clone(), start_frame)
            }
            SamplerNodeResource::Streamed(s) => s.fill_buffers(
                out_buffer,
                out_buffer_range,
                start_frame,
                speed,
                is_playing_backwards,
            ),
        }
    }

    /// Returns `true` if the given range of frames is loaded
    /// into memory and ready to be read.
    pub fn range_is_ready(&mut self, range: Range<u64>) -> bool {
        if let SamplerNodeResource::Streamed(s) = self {
            s.range_is_ready(range)
        } else {
            true
        }
    }

    /// Request to cache a new region at the given starting frame.
    pub fn cache_new_starting_frame(&mut self, frame: u64, speed: f64, will_play_backwards: bool) {
        if let SamplerNodeResource::Streamed(s) = self {
            s.cache_new_starting_frame(frame, speed, will_play_backwards);
        }
    }
}

impl From<ArcGc<dyn SampleResource + Send + Sync + 'static>> for SamplerNodeResource {
    fn from(value: ArcGc<dyn SampleResource + Send + Sync + 'static>) -> Self {
        Self::InMemory(value)
    }
}

impl From<OwnedGcUnsized<dyn StreamedSample>> for SamplerNodeResource {
    fn from(value: OwnedGcUnsized<dyn StreamedSample>) -> Self {
        Self::Streamed(value)
    }
}

/// A resource of audio samples that are streamed from disk or over a network.
///
/// This uses considerably less memory, but requires a more complicated setup. It
/// also has the potential to run into cache misses if the playhead is moved to a
/// region that hasn't been loaded yet, or if the stream fails to send enough samples
/// in time.
pub trait StreamedSample: SampleResourceInfo + Send + Sync + 'static {
    /// Fill the given buffers with audio data starting from the given
    /// starting frame in the resource.
    ///
    /// * `out_buffer` - The buffers to fill with data. If the length of `buffers`
    ///   is greater than the number of channels in this resource, then ignore
    ///   the extra buffers.
    /// * `out_buffer_range` - The range inside each buffer slice in which to
    ///   fill with data. Do not fill any data outside of this range.
    /// * `start_frame` - The sample (of a single channel of audio) in the
    ///   resource at which to start copying from. Not to be confused with video
    ///   frames.
    /// * `speed` - The speed at which playback is occurring, where `1.0` is
    ///   playing at the sample rate of this resource, `0.5` is playing at half
    ///   the sample rate, and `2.0` is playing at twice the sample rate.
    ///
    /// Returns the number of frames that were successfully filled. This may
    /// be less than the length of `out_buffer_range` if the range is all or
    /// partly out of bounds of the resource, or if a cache miss occurred.
    /// Any frames that were not successfully filled will be left untouched.
    fn fill_buffers(
        &mut self,
        out_buffer: &mut [&mut [f32]],
        out_buffer_range: Range<usize>,
        start_frame: u64,
        speed: f64,
        is_playing_backwards: bool,
    ) -> usize;

    /// Returns `true` if the given range of frames is loaded
    /// into memory and ready to be read.
    fn range_is_ready(&mut self, range: Range<u64>) -> bool;

    /// Request to cache a new region at the given starting frame.
    fn cache_new_starting_frame(&mut self, frame: u64, speed: f64, will_play_backwards: bool);
}
