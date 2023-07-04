use std::error::Error;
use pulser::api::PAIdent;
use pulser::simple::PulseAudio;

pub struct PulseAudioHandler {
    device_index: PAIdent,
    pulseaudio: PulseAudio
}

// Abstract the pulser crate for future testing?
impl PulseAudioHandler {
    // Initialize PulseAudio.
    // I know doing logic in a "constructor" is a bad idea, but I'm lazy.
    pub fn new(device_idx: u32) -> Result<Self, Box<dyn Error>> {
        let device_index = {
            let tmp = &mut PAIdent::Index(device_idx);
            tmp.clone()
        };

        let pulseaudio = PulseAudio::connect(Some("Mewture Button"));
        pulseaudio.set_default_source(device_index.clone())?;

        Ok(Self { device_index, pulseaudio })
    }

    pub fn get_mute_state(&mut self) -> Result<bool, Box<dyn Error>> {
        let mute_state = self.pulseaudio.get_source_mute(self.device_index.clone())?;
        Ok(mute_state)
    }

    pub fn set_mute_state(&mut self, mute_state: bool) -> Result<(), Box<dyn Error>> {
        self.pulseaudio.set_source_mute(self.device_index.clone(), mute_state)?;
        Ok(())
    }
}
