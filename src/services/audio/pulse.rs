use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::subscribe::InterestMaskSet;
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::operation::{self, Operation};
use libpulse_binding::proplist::properties;
use libpulse_binding::volume::{ChannelVolumes, Volume};

#[derive(Clone, Copy)]
pub enum Device {
    // speakers or headphones
    Output,

    // the microphone
    Input,
}

impl Device {
    // pulse resolves these to whatever device is the default right now
    fn name(self) -> &'static str {
        match self {
            Device::Output => "@DEFAULT_SINK@",
            Device::Input => "@DEFAULT_SOURCE@",
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Level {
    // 0 to 100
    pub volume: u8,

    pub muted: bool,
}

// one output or input the sound server knows, as pulse names it
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    // what pulse is told to make the default
    pub(crate) name: String,

    // the name people read, like "Built-in Audio Analog Stereo"
    pub(crate) description: String,

    // a freedesktop icon name like "audio-headphones", empty when pulse has none
    pub(crate) icon: String,

    pub(crate) default: bool,
}

// what pulse reports for a default device, before converting to percent
#[derive(Clone, Copy)]
struct Levels {
    volumes: ChannelVolumes,
    muted: bool,
}

// how long to wait before trying the sound server again while it is down
const RETRY: Duration = Duration::from_secs(5);

thread_local! {
    // pulse objects can't move between threads, so each thread opens its own
    static CONNECTION: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

/*
 * runs work on this thread's connection, opened again when the sound server
 * went away, like when it restarts; with no sound server work gets the default
 */
fn with_connection<T: Default>(work: impl FnOnce(&mut Connection) -> T) -> T {
    CONNECTION.with_borrow_mut(|slot| {
        let alive = slot.as_ref().is_some_and(Connection::ready);

        if !alive {
            *slot = Connection::open();
        }

        let Some(connection) = slot else {
            return T::default();
        };

        work(connection)
    })
}

pub fn read(device: Device) -> Level {
    with_connection(|connection| {
        let Some(levels) = connection.levels(device) else {
            return Level::default();
        };

        Level {
            volume: percent(levels.volumes.avg()),
            muted: levels.muted,
        }
    })
}

pub fn set_volume(device: Device, volume: u8) {
    with_connection(|connection| {
        let Some(levels) = connection.levels(device) else {
            return;
        };

        // every channel gets the same level, like a single volume slider does
        let mut volumes = levels.volumes;
        let channels = volumes.len();

        volumes.set(channels, level(volume));

        let mut introspect = connection.context.introspect();

        let operation = match device {
            Device::Output => introspect.set_sink_volume_by_name(device.name(), &volumes, None),
            Device::Input => introspect.set_source_volume_by_name(device.name(), &volumes, None),
        };

        connection.wait(operation);
    });
}

pub fn set_muted(device: Device, muted: bool) {
    with_connection(|connection| {
        let mut introspect = connection.context.introspect();

        let operation = match device {
            Device::Output => introspect.set_sink_mute_by_name(device.name(), muted, None),
            Device::Input => introspect.set_source_mute_by_name(device.name(), muted, None),
        };

        connection.wait(operation);
    });
}

// every output, or every input that isn't another output's monitor
pub fn list(device: Device) -> Vec<AudioDevice> {
    with_connection(|connection| connection.devices(device))
}

pub fn set_default(device: Device, name: &str) {
    with_connection(|connection| {
        let operation = match device {
            Device::Output => connection.context.set_default_sink(name, |_| {}),
            Device::Input => connection.context.set_default_source(name, |_| {}),
        };

        connection.wait(operation);
    });
}

/*
 * calls changed after every change to an output, an input or the
 * default choice, and never returns; it keeps its own
 * connection, so changed can still use read()
 */
pub fn watch(mut changed: impl FnMut()) {
    loop {
        if let Some(connection) = Connection::open() {
            follow(connection, &mut changed);
        }

        // the sound server is down or restarting, so it is tried again after a pause
        thread::sleep(RETRY);
    }
}

// returns once the connection breaks
fn follow(mut connection: Connection, changed: &mut impl FnMut()) {
    let dirty = Rc::new(Cell::new(false));
    let marked = Rc::clone(&dirty);

    /*
     * pulse can't answer questions from inside its own callback,
     * so the callback only marks the change and the loop reacts
     */
    connection
        .context
        .set_subscribe_callback(Some(Box::new(move |_, _, _| marked.set(true))));

    let interests = InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER;

    connection.context.subscribe(interests, |_| {});

    // a new connection may come after a restart that changed the levels
    changed();

    while connection.iterate() {
        if dirty.replace(false) {
            changed();
        }
    }
}

// fields drop in order, and the context has to go before the loop it runs on
struct Connection {
    context: Context,
    mainloop: Mainloop,
}

impl Connection {
    // none when the sound server can't be reached
    fn open() -> Option<Self> {
        let mainloop = Mainloop::new()?;

        let mut context = Context::new(&mainloop, "amane")?;

        context.connect(None, FlagSet::NOFLAGS, None).ok()?;

        let mut connection = Self { context, mainloop };

        loop {
            if !connection.iterate() {
                return None;
            }

            match connection.context.get_state() {
                State::Ready => return Some(connection),
                State::Failed | State::Terminated => return None,
                _ => {}
            }
        }
    }

    // none when there is no such device at all
    fn levels(&mut self, device: Device) -> Option<Levels> {
        let found = Rc::new(Cell::new(None));
        let slot = Rc::clone(&found);

        let introspect = self.context.introspect();

        // outputs and inputs come back as different types, so each gets its own callback
        match device {
            Device::Output => {
                let operation = introspect.get_sink_info_by_name(device.name(), move |result| {
                    let ListResult::Item(sink) = result else {
                        return;
                    };

                    slot.set(Some(Levels {
                        volumes: sink.volume,
                        muted: sink.mute,
                    }));
                });

                self.wait(operation);
            }

            Device::Input => {
                let operation = introspect.get_source_info_by_name(device.name(), move |result| {
                    let ListResult::Item(source) = result else {
                        return;
                    };

                    slot.set(Some(Levels {
                        volumes: source.volume,
                        muted: source.mute,
                    }));
                });

                self.wait(operation);
            }
        }

        found.take()
    }

    fn devices(&mut self, device: Device) -> Vec<AudioDevice> {
        let found = Rc::new(RefCell::new(Vec::new()));
        let slot = Rc::clone(&found);

        let introspect = self.context.introspect();

        match device {
            Device::Output => {
                let operation = introspect.get_sink_info_list(move |result| {
                    let ListResult::Item(sink) = result else {
                        return;
                    };

                    slot.borrow_mut().push(AudioDevice {
                        name: text(&sink.name),
                        description: text(&sink.description),
                        icon: sink
                            .proplist
                            .get_str(properties::DEVICE_ICON_NAME)
                            .unwrap_or_default(),
                        default: false,
                    });
                });

                self.wait(operation);
            }

            Device::Input => {
                let operation = introspect.get_source_info_list(move |result| {
                    let ListResult::Item(source) = result else {
                        return;
                    };

                    // a monitor only plays back what an output plays, it isn't a microphone
                    if source.monitor_of_sink.is_some() {
                        return;
                    }

                    slot.borrow_mut().push(AudioDevice {
                        name: text(&source.name),
                        description: text(&source.description),
                        icon: source
                            .proplist
                            .get_str(properties::DEVICE_ICON_NAME)
                            .unwrap_or_default(),
                        default: false,
                    });
                });

                self.wait(operation);
            }
        }

        let default = self.default_name(device);

        let mut devices = found.take();

        for found_device in &mut devices {
            found_device.default = default.as_deref() == Some(found_device.name.as_str());

            // a device without a description still needs something to show
            if found_device.description.is_empty() {
                found_device.description = found_device.name.clone();
            }
        }

        devices
    }

    // the name behind @DEFAULT_SINK@ or @DEFAULT_SOURCE@ right now
    fn default_name(&mut self, device: Device) -> Option<String> {
        let found = Rc::new(RefCell::new(None));
        let slot = Rc::clone(&found);

        let operation = self.context.introspect().get_server_info(move |server| {
            let name = match device {
                Device::Output => &server.default_sink_name,
                Device::Input => &server.default_source_name,
            };

            *slot.borrow_mut() = name.as_ref().map(|name| name.to_string());
        });

        self.wait(operation);

        found.take()
    }

    // blocks until pulse has answered
    fn wait<F: ?Sized>(&mut self, operation: Operation<F>) {
        while operation.get_state() == operation::State::Running {
            if !self.iterate() {
                return;
            }
        }
    }

    fn ready(&self) -> bool {
        self.context.get_state() == State::Ready
    }

    // false once the connection to the sound server is broken
    fn iterate(&mut self) -> bool {
        let result = self.mainloop.iterate(true);

        matches!(result, IterateResult::Success(_))
    }
}

fn text(value: &Option<std::borrow::Cow<'_, str>>) -> String {
    value.as_deref().map(String::from).unwrap_or_default()
}

// pulse counts volume with 100% at Volume::NORMAL
fn percent(volume: Volume) -> u8 {
    let level = u64::from(volume.0);
    let normal = u64::from(Volume::NORMAL.0);

    let rounded = (level * 100 + normal / 2) / normal;

    rounded.min(100) as u8
}

fn level(percent: u8) -> Volume {
    let normal = Volume::NORMAL.0;

    Volume(u32::from(percent) * normal / 100)
}
