use std::error::Error;
use pulser::api::PAIdent;
use pulser::simple::PulseAudio;

pub struct PulseAudioHandler {
    device_name: PAIdent,
    pulseaudio: PulseAudio
}

// Abstract the pulser crate for future testing?
impl PulseAudioHandler {
    /// Creates a new `PulseAudioHandler` instance.
    /// I know doing logic in a "constructor" is a bad idea, but I'm lazy.
    /// # Arguments
    ///
    /// * `pulseaudio` - A `PulseAudio` instance.
    /// * `device_name` - The name of the audio device to manage.
    ///
    /// # Errors
    ///
    /// Returns an error if setting the default source fails.
    pub fn new(pulseaudio: PulseAudio, device_name: String) -> Result<Self, Box<dyn Error>> {
        let device_name = PAIdent::Name(device_name);
        pulseaudio.set_default_source(device_name.clone())?;
        Ok(Self { device_name, pulseaudio })
    }

    /// Gets the mute state of the managed audio source.
    ///
    /// # Errors
    ///
    /// Returns an error if getting the mute state fails.
    pub fn get_mute_state(&mut self) -> Result<bool, Box<dyn Error>> {
        let mute_state = self.pulseaudio.get_source_mute(self.device_name.clone())?;
        Ok(mute_state)
    }

    /// Sets the mute state of the managed audio source.
    ///
    /// # Arguments
    ///
    /// * `mute_state` - The desired mute state (true for muted, false for unmuted).
    ///
    /// # Errors
    ///
    /// Returns an error if setting the mute state fails.
    pub fn set_mute_state(&mut self, mute_state: bool) -> Result<(), Box<dyn Error>> {
        self.pulseaudio.set_source_mute(self.device_name.clone(), mute_state)?;
        Ok(())
    }
}
