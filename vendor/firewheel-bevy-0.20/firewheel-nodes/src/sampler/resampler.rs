use core::ops::Range;
use firewheel_core::node::ProcExtra;

#[cfg(not(feature = "std"))]
use num_traits::Float;

use super::{MAX_OUT_CHANNELS, PlaybackSpeedQuality, SamplerProcessor};

pub(super) struct Resampler {
    fract_in_frame: f64,
    is_first_process: bool,
    prev_speed: f64,
    _quality: PlaybackSpeedQuality,
    wraparound_buffer: [[f32; 2]; MAX_OUT_CHANNELS],
}

impl Resampler {
    pub fn new(quality: PlaybackSpeedQuality) -> Self {
        Self {
            fract_in_frame: 0.0,
            is_first_process: true,
            prev_speed: 1.0,
            _quality: quality,
            wraparound_buffer: [[0.0; 2]; MAX_OUT_CHANNELS],
        }
    }

    pub fn resample_linear(
        &mut self,
        out_buffers: &mut [&mut [f32]],
        out_buffer_range: Range<usize>,
        extra: &mut ProcExtra,
        processor: &mut SamplerProcessor,
        looping: bool,
    ) -> (bool, usize) {
        let total_out_frames = out_buffer_range.end - out_buffer_range.start;

        assert_ne!(total_out_frames, 0);

        let in_frame_start = if self.is_first_process {
            self.prev_speed = processor.speed;
            self.fract_in_frame = 0.0;

            0.0
        } else {
            self.fract_in_frame + processor.speed
        };

        let out_frame_to_in_frame = |out_frame: f64, in_frame_start: f64, speed: f64| -> f64 {
            in_frame_start + (out_frame * speed)
        };

        // The function which maps the output frame to the input frame is given by
        // the kinematic equation:
        //
        // in_frame = in_frame_start + (out_frame * start_speed) + (0.5 * accel * out_frame^2)
        //      where: accel = (end_speed - start_speed)
        let out_frame_to_in_frame_with_accel =
            |out_frame: f64, in_frame_start: f64, start_speed: f64, half_accel: f64| -> f64 {
                in_frame_start + (out_frame * start_speed) + (out_frame * out_frame * half_accel)
            };

        let num_channels = processor.num_channels_filled();
        let copy_start = if self.is_first_process { 0 } else { 2 };
        let mut finished_playing = false;

        if self.prev_speed == processor.speed {
            self.resample_linear_inner(
                out_frame_to_in_frame,
                in_frame_start,
                self.prev_speed,
                out_buffer_range.clone(),
                processor,
                extra,
                looping,
                copy_start,
                num_channels,
                out_buffers,
                out_buffer_range.start,
                &mut finished_playing,
            );
        } else {
            let half_accel = 0.5 * (processor.speed - self.prev_speed) / total_out_frames as f64;

            self.resample_linear_inner(
                |out_frame: f64, in_frame_start: f64, speed: f64| {
                    out_frame_to_in_frame_with_accel(out_frame, in_frame_start, speed, half_accel)
                },
                in_frame_start,
                self.prev_speed,
                out_buffer_range.clone(),
                processor,
                extra,
                looping,
                copy_start,
                num_channels,
                out_buffers,
                out_buffer_range.start,
                &mut finished_playing,
            );
        }

        self.prev_speed = processor.speed;
        self.is_first_process = false;

        (finished_playing, num_channels)
    }

    #[expect(clippy::too_many_arguments, reason = "Function needs many arguments")]
    fn resample_linear_inner<OutToInFrame>(
        &mut self,
        out_to_in_frame: OutToInFrame,
        in_frame_start: f64,
        speed: f64,
        out_buffer_range: Range<usize>,
        processor: &mut SamplerProcessor,
        extra: &mut ProcExtra,
        looping: bool,
        mut copy_start: usize,
        num_channels: usize,
        out_buffers: &mut [&mut [f32]],
        out_buffer_start: usize,
        finished_playing: &mut bool,
    ) where
        OutToInFrame: Fn(f64, f64, f64) -> f64,
    {
        let mut scratch_buffers = extra.scratch_buffers.all_mut();

        let total_out_frames = out_buffer_range.end - out_buffer_range.start;
        let output_frame_end = (total_out_frames - 1) as f64;

        let input_frame_end = out_to_in_frame(output_frame_end, in_frame_start, speed);
        let input_frames_needed = input_frame_end.trunc() as usize + 2;

        let mut input_frames_processed = 0;
        let mut output_frames_processed = 0;
        while output_frames_processed < total_out_frames {
            let input_frames =
                (input_frames_needed - input_frames_processed).min(processor.max_block_frames);

            if input_frames > copy_start {
                let (finished, _) = processor.copy_from_sample(
                    &mut scratch_buffers[..num_channels],
                    copy_start..input_frames,
                    looping,
                );
                if finished {
                    *finished_playing = true;
                }
            }

            let max_block_frames_minus_1 = processor.max_block_frames - 1;
            let out_ch_start = out_buffer_start + output_frames_processed;

            let mut out_frames_count = 0;

            // Have an optimized loop for stereo audio.
            if num_channels == 2 {
                let mut last_in_frame = 0;
                let mut last_fract_frame = 0.0;

                let (out_ch_0, out_ch_1) = out_buffers.split_first_mut().unwrap();
                let (r_ch_0, r_ch_1) = scratch_buffers.split_first_mut().unwrap();

                let out_ch_0 = &mut out_ch_0[out_ch_start..out_buffer_range.end];
                let out_ch_1 = &mut out_ch_1[0][out_ch_start..out_buffer_range.end];

                let r_ch_0 = &mut r_ch_0[..processor.max_block_frames];
                let r_ch_1 = &mut r_ch_1[0][..processor.max_block_frames];

                if copy_start > 0 {
                    r_ch_0[0] = self.wraparound_buffer[0][0];
                    r_ch_1[0] = self.wraparound_buffer[1][0];

                    r_ch_0[1] = self.wraparound_buffer[0][1];
                    r_ch_1[1] = self.wraparound_buffer[1][1];
                }

                for (i, (out_s_0, out_s_1)) in
                    out_ch_0.iter_mut().zip(out_ch_1.iter_mut()).enumerate()
                {
                    let out_frame = (i + output_frames_processed) as f64;

                    let in_frame_f64 = out_to_in_frame(out_frame, in_frame_start, speed);

                    let in_frame_usize = in_frame_f64.trunc() as usize - input_frames_processed;
                    let fract_frame = in_frame_f64.fract();

                    if in_frame_usize >= max_block_frames_minus_1 {
                        break;
                    }

                    let s0_0 = r_ch_0[in_frame_usize];
                    let s0_1 = r_ch_1[in_frame_usize];

                    let s1_0 = r_ch_0[in_frame_usize + 1];
                    let s1_1 = r_ch_1[in_frame_usize + 1];

                    *out_s_0 = s0_0 + ((s1_0 - s0_0) * fract_frame as f32);
                    *out_s_1 = s0_1 + ((s1_1 - s0_1) * fract_frame as f32);

                    last_in_frame = in_frame_usize;
                    last_fract_frame = fract_frame;

                    out_frames_count += 1;
                }

                self.wraparound_buffer[0][0] = r_ch_0[last_in_frame];
                self.wraparound_buffer[1][0] = r_ch_1[last_in_frame];

                self.wraparound_buffer[0][1] = r_ch_0[last_in_frame + 1];
                self.wraparound_buffer[1][1] = r_ch_1[last_in_frame + 1];

                self.fract_in_frame = last_fract_frame;
            } else {
                for ((out_ch, r_ch), w_ch) in out_buffers[..num_channels]
                    .iter_mut()
                    .zip(scratch_buffers[..num_channels].iter_mut())
                    .zip(self.wraparound_buffer[..num_channels].iter_mut())
                {
                    // Hint to compiler to optimize loop.
                    assert_eq!(r_ch.len(), processor.max_block_frames);

                    if copy_start > 0 {
                        r_ch[0] = w_ch[0];
                        r_ch[1] = w_ch[1];
                    }

                    let mut last_in_frame = 0;
                    let mut last_fract_frame = 0.0;
                    let mut out_frames_ch_count = 0;
                    for (i, out_s) in out_ch[out_ch_start..out_buffer_range.end]
                        .iter_mut()
                        .enumerate()
                    {
                        let out_frame = (i + output_frames_processed) as f64;

                        let in_frame_f64 = out_to_in_frame(out_frame, in_frame_start, speed);

                        let in_frame_usize = in_frame_f64.trunc() as usize - input_frames_processed;
                        last_fract_frame = in_frame_f64.fract();

                        if in_frame_usize >= max_block_frames_minus_1 {
                            break;
                        }

                        let s0 = r_ch[in_frame_usize];
                        let s1 = r_ch[in_frame_usize + 1];

                        *out_s = s0 + ((s1 - s0) * last_fract_frame as f32);

                        last_in_frame = in_frame_usize;
                        out_frames_ch_count += 1;
                    }

                    w_ch[0] = r_ch[last_in_frame];
                    w_ch[1] = r_ch[last_in_frame + 1];

                    self.fract_in_frame = last_fract_frame;
                    out_frames_count = out_frames_ch_count;
                }
            }

            output_frames_processed += out_frames_count;
            input_frames_processed += input_frames - 2;

            copy_start = 2;
        }
    }

    pub fn reset(&mut self) {
        self.fract_in_frame = 0.0;
        self.is_first_process = true;
    }
}
