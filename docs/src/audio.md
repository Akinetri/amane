# Audio

`Audio` reads and controls the volume of your default speaker and microphone. It talks to PulseAudio, which also covers PipeWire through `pipewire-pulse`, the default on most distributions.

It doesn't poll. The sound server announces every change, so the bar updates as soon as you press a volume key.

## A volume widget

Scroll to change the volume, click to mute:

```rust
use amane::{App, Audio, Button, LayerWindow, Parent, Rectangle, Scroll, Service, Text};

fn main() {
    App::new().window(view).run();
}

fn view() -> LayerWindow {
    let audio = Audio::read();

    let label = if audio.muted() {
        String::from("muted")
    } else {
        format!("volume {}%", audio.volume())
    };

    LayerWindow::new().width(200.0).height(30.0).child(
        Rectangle::new()
            .width(Parent)
            .height(Parent)
            .on_click(clicked)
            .on_scroll(scrolled)
            .child(Text::new(label)),
    )
}

fn clicked(_: Button) {
    Audio::toggle_mute();
}

// each wheel step moves the volume by 5
fn scrolled(scroll: Scroll) {
    let volume = i32::from(Audio::read().volume());

    let step = if scroll.y < 0.0 { 5 } else { -5 };

    let changed = (volume + step).clamp(0, 100);

    Audio::set_volume(changed as u8);
}
```

## Reference

| Function | Gives or does |
|---|---|
| `volume()` | speaker volume, 0 to 100 |
| `muted()` | `true` when the speaker is muted |
| `microphone_volume()` | microphone volume, 0 to 100 |
| `microphone_muted()` | `true` when the microphone is muted |
| `Audio::set_volume(volume)` | sets the speaker volume, 0 to 100 |
| `Audio::toggle_mute()` | mutes or unmutes the speaker |
| `Audio::set_microphone_volume(volume)` | sets the microphone volume, 0 to 100 |
| `Audio::toggle_microphone_mute()` | mutes or unmutes the microphone |
| `outputs()` | every speaker or headphone, as `AudioDevice`s |
| `inputs()` | every microphone, as `AudioDevice`s, without the monitors that only replay an output |
| `Audio::set_output(name)` | makes the output with that `name()` the default |
| `Audio::set_input(name)` | makes the input with that `name()` the default |

An `AudioDevice` gives:

| Function | Gives |
|---|---|
| `name()` | the sound server's name for it, what `set_output` and `set_input` take |
| `description()` | the name people read, like "Built-in Audio Analog Stereo" |
| `icon()` | a freedesktop icon name like `audio-headphones`, empty when there is none |
| `default()` | `true` for the device the volume and mute belong to |

"Speaker" and "microphone" mean your default output and input devices. When you switch the default, for example by plugging in headphones or with `set_output`, `Audio` follows, and so do the apps that play without a device of their own.

The control functions run on a background thread, so a slow sound server never freezes the shell. The new value shows up when the sound server announces it, usually right away.
