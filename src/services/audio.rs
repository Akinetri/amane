mod pulse;

use crate::Service;
use crate::services::worker;

use pulse::Device;

pub use pulse::AudioDevice;

#[derive(Default)]
pub struct Audio {
    // 0 to 100, for the default output
    volume: u8,

    muted: bool,

    // 0 to 100, for the default microphone
    microphone_volume: u8,

    microphone_muted: bool,

    // every speaker or headphone the sound server knows, the default one marked
    outputs: Vec<AudioDevice>,

    // every microphone, without the monitors that only replay an output
    inputs: Vec<AudioDevice>,
}

// event-driven, the sound server announces every volume change
impl Service for Audio {
    fn new() -> Self {
        let mut audio = Self::default();

        audio.update();

        audio
    }

    fn update(&mut self) -> bool {
        let before = (
            self.volume,
            self.muted,
            self.microphone_volume,
            self.microphone_muted,
        );

        let outputs = pulse::list(Device::Output);
        let inputs = pulse::list(Device::Input);

        let devices_changed = outputs != self.outputs || inputs != self.inputs;

        self.outputs = outputs;
        self.inputs = inputs;

        let output = pulse::read(Device::Output);
        let input = pulse::read(Device::Input);

        self.volume = output.volume;
        self.muted = output.muted;

        self.microphone_volume = input.volume;
        self.microphone_muted = input.muted;

        (
            self.volume,
            self.muted,
            self.microphone_volume,
            self.microphone_muted,
        ) != before
            || devices_changed
    }

    // the sound server also announces changes nothing shows, like a port switch
    fn listen() {
        pulse::watch(|| {
            let mut audio = Self::write();

            if !audio.update() {
                audio.quiet();
            }
        });
    }
}

impl Audio {
    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn microphone_volume(&self) -> u8 {
        self.microphone_volume
    }

    pub fn microphone_muted(&self) -> bool {
        self.microphone_muted
    }

    pub fn outputs(&self) -> &[AudioDevice] {
        &self.outputs
    }

    pub fn inputs(&self) -> &[AudioDevice] {
        &self.inputs
    }

    // anything above 100 is treated as 100
    pub fn set_volume(volume: u8) {
        worker::run(move || pulse::set_volume(Device::Output, volume.min(100)));
    }

    pub fn toggle_mute() {
        worker::run(|| toggle(Device::Output));
    }

    // anything above 100 is treated as 100
    pub fn set_microphone_volume(volume: u8) {
        worker::run(move || pulse::set_volume(Device::Input, volume.min(100)));
    }

    pub fn toggle_microphone_mute() {
        worker::run(|| toggle(Device::Input));
    }

    // name is an output's name(), the volume and mute then follow it
    pub fn set_output(name: &str) {
        let name = String::from(name);

        worker::run(move || pulse::set_default(Device::Output, &name));
    }

    // name is an input's name(), the microphone volume and mute then follow it
    pub fn set_input(name: &str) {
        let name = String::from(name);

        worker::run(move || pulse::set_default(Device::Input, &name));
    }
}

impl AudioDevice {
    // what set_output() or set_input() takes, like "alsa_output.pci-0000_00_1f.3.analog-stereo"
    pub fn name(&self) -> &str {
        &self.name
    }

    // the name people read, like "Built-in Audio Analog Stereo"
    pub fn description(&self) -> &str {
        &self.description
    }

    // a freedesktop icon name like "audio-headphones", empty when the sound server has none
    pub fn icon(&self) -> &str {
        &self.icon
    }

    // true for the device the volume and mute belong to
    pub fn default(&self) -> bool {
        self.default
    }
}

fn toggle(device: Device) {
    let level = pulse::read(device);

    pulse::set_muted(device, !level.muted);
}
