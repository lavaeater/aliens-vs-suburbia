use bevy_platform::time::Instant;

/// Return [`Instant::now`] in contexts where it's available.
///
/// In a Wasm-with-JS context, [`Instant::now`] will panic in
/// an audio worklet. Rather than panicking, this function returns `None`.
pub fn now() -> Option<Instant> {
    #[cfg(all(
        feature = "wasm-bindgen",
        target_family = "wasm",
        target_feature = "atomics"
    ))]
    return is_not_worklet().then(|| bevy_platform::time::Instant::now());

    #[cfg(not(all(
        feature = "wasm-bindgen",
        target_arch = "wasm32",
        target_feature = "atomics"
    )))]
    return Some(bevy_platform::time::Instant::now());
}

#[cfg(all(
    feature = "wasm-bindgen",
    target_family = "wasm",
    target_feature = "atomics"
))]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
    export function is_audio_worklet() {
        return typeof sampleRate !== 'undefined';
    }
")]
extern "C" {
    fn is_audio_worklet() -> bool;
}

/// Determines if this execution context is an audio worklet.
#[cfg(all(
    feature = "wasm-bindgen",
    target_family = "wasm",
    target_feature = "atomics"
))]
fn is_not_worklet() -> bool {
    #[cfg(feature = "std")]
    {
        // A thread local allows us to limit calls into JS to once per
        // execution context.
        thread_local! {
            static IS_NOT_WORKLET: bool = !is_audio_worklet();
        }

        return IS_NOT_WORKLET.with(|w| *w);
    }

    #[cfg(not(feature = "std"))]
    return !is_audio_worklet();
}
