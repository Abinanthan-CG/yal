//! Master volume and mute.
//!
//! The endpoint volume is read and written through the audio session COM
//! interfaces. The launcher runs these calls on demand — there is nothing
//! to keep running between uses.

/// A failure while reading or changing the volume.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("the audio endpoint could not be opened: {0}")]
    Endpoint(String),

    #[error("the volume operation failed: {0}")]
    Failed(String),
}

/// The volume level on a 0 to 100 scale, and whether output is muted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    /// The level from 0 to 100.
    pub level: f64,
    /// Whether output is muted.
    pub muted: bool,
}

/// The default output endpoint, initialised for COM on this thread.
#[cfg(windows)]
fn endpoint() -> Result<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume, AudioError> {
    use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::core::GUID;

    // The system device enumerator. Its class identifier is part of the
    // published COM contract for audio endpoints.
    const CLSID_MM_DEVICE_ENUMERATOR: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&CLSID_MM_DEVICE_ENUMERATOR, None, CLSCTX_ALL)
                .map_err(|err| AudioError::Endpoint(err.to_string()))?;

        let device: IMMDevice = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|err| AudioError::Endpoint(err.to_string()))?;

        device
            .Activate::<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume>(
                CLSCTX_ALL,
                None,
            )
            .map_err(|err| AudioError::Endpoint(err.to_string()))
    }
}

/// The current volume and mute state of the default output device.
#[cfg(windows)]
pub fn volume() -> Result<VolumeState, AudioError> {
    let endpoint = endpoint()?;
    unsafe {
        let scalar = endpoint
            .GetMasterVolumeLevelScalar()
            .map_err(|err| AudioError::Failed(err.to_string()))?;
        let muted = endpoint
            .GetMute()
            .map_err(|err| AudioError::Failed(err.to_string()))?;
        Ok(VolumeState {
            level: f64::from(scalar) * 100.0,
            muted: muted.as_bool(),
        })
    }
}

/// The volume state on a host without audio control.
#[cfg(not(windows))]
pub fn volume() -> Result<VolumeState, AudioError> {
    Ok(VolumeState {
        level: 0.0,
        muted: false,
    })
}

/// Set the volume on the 0 to 100 scale.
#[cfg(windows)]
pub fn set_volume(level: f64) -> Result<(), AudioError> {
    if !(0.0..=100.0).contains(&level) {
        return Err(AudioError::Failed(format!(
            "{level} is not a volume between 0 and 100"
        )));
    }
    let endpoint = endpoint()?;
    unsafe {
        endpoint
            .SetMasterVolumeLevelScalar((level / 100.0) as f32, std::ptr::null())
            .map_err(|err| AudioError::Failed(err.to_string()))
    }
}

/// Set the volume on a host without audio control.
#[cfg(not(windows))]
pub fn set_volume(_level: f64) -> Result<(), AudioError> {
    Err(AudioError::Failed(
        "volume control is not implemented on this platform".into(),
    ))
}

/// Change the volume by a number of points, clamped to the range.
#[cfg(windows)]
pub fn adjust_volume(delta: f64) -> Result<VolumeState, AudioError> {
    let current = volume()?;
    let next = (current.level + delta).clamp(0.0, 100.0);
    set_volume(next)?;
    volume()
}

/// Change the volume by a number of points on a host without audio control.
#[cfg(not(windows))]
pub fn adjust_volume(_delta: f64) -> Result<VolumeState, AudioError> {
    volume()
}

/// Mute or unmute the default output device.
#[cfg(windows)]
pub fn set_muted(muted: bool) -> Result<(), AudioError> {
    let endpoint = endpoint()?;
    unsafe {
        endpoint
            .SetMute(muted, std::ptr::null())
            .map_err(|err| AudioError::Failed(err.to_string()))
    }
}

/// Mute or unmute on a host without audio control.
#[cfg(not(windows))]
pub fn set_muted(_muted: bool) -> Result<(), AudioError> {
    Err(AudioError::Failed(
        "volume control is not implemented on this platform".into(),
    ))
}

/// Toggle mute, returning the new state.
#[cfg(windows)]
pub fn toggle_mute() -> Result<VolumeState, AudioError> {
    let current = volume()?;
    set_muted(!current.muted)?;
    volume()
}

/// Toggle mute on a host without audio control.
#[cfg(not(windows))]
pub fn toggle_mute() -> Result<VolumeState, AudioError> {
    volume()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_out_of_range_volume() {
        assert!(set_volume(150.0).is_err());
        assert!(set_volume(-1.0).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn reads_the_volume_back_within_the_range() {
        if let Ok(state) = volume() {
            assert!((0.0..=100.0).contains(&state.level));
        }
    }

    #[cfg(windows)]
    #[test]
    fn setting_the_volume_round_trips() {
        // Remember where it was so the test leaves the machine as it was.
        let Ok(before) = volume() else {
            return;
        };
        let Ok(_) = set_volume(37.0) else {
            return;
        };
        let after = volume().unwrap();
        assert!((after.level - 37.0).abs() < 0.5);
        let _ = set_volume(before.level);
    }
}
