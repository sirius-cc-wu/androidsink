#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use gst::gst_element_error;
use gst::prelude::*;

use byte_slice_cast::*;

use std::i16;
use std::i32;

use anyhow::Error;
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
#[display(fmt = "Missing element {}", _0)]
struct MissingElement(#[error(not(source))] &'static str);

#[derive(Debug, Display, Error)]
#[display(fmt = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    src: String,
    error: String,
    debug: Option<String>,
    source: glib::Error,
}

fn create_pipeline() -> Result<gst::Pipeline, Error> {
    let pipeline = gst::Pipeline::new(None);
    let src = gst::ElementFactory::make("audiotestsrc", None)
        .map_err(|_| MissingElement("audiotestsrc"))?;
    let sink = gst::ElementFactory::make("appsink", None).map_err(|_| MissingElement("appsink"))?;

    pipeline.add_many(&[&src, &sink])?;
    src.link(&sink)?;

    let appsink = sink
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");

    // Tell the appsink what format we want. It will then be the audiotestsrc's job to
    // provide the format we request.
    // This can be set after linking the two objects, because format negotiation between
    // both elements will happen during pre-rolling of the pipeline.
    appsink.set_caps(Some(&gst::Caps::new_simple(
        "audio/x-raw",
        &[
            ("format", &gst_audio::AUDIO_FORMAT_S16.to_str()),
            ("layout", &"interleaved"),
            ("channels", &(1i32)),
            ("rate", &gst::IntRange::<i32>::new(1, i32::MAX)),
        ],
    )));

    // Getting data out of the appsink is done by setting callbacks on it.
    // The appsink will then call those handlers, as soon as data is available.
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(|appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.get_buffer().ok_or_else(|| {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("Failed to get buffer from appsink")
                    );

                    gst::FlowError::Error
                })?;

                // At this point, buffer is only a reference to an existing memory region somewhere.
                // When we want to access its content, we have to map it while requesting the required
                // mode of access (read, read/write).
                // This type of abstraction is necessary, because the buffer in question might not be
                // on the machine's main memory itself, but rather in the GPU's memory.
                // So mapping the buffer makes the underlying memory region accessible to us.
                // See: https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/allocation.html
                let map = buffer.map_readable().map_err(|_| {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("Failed to map buffer readable")
                    );

                    gst::FlowError::Error
                })?;

                // We know what format the data in the memory region has, since we requested
                // it by setting the appsink's caps. So what we do here is interpret the
                // memory region we mapped as an array of signed 16 bit integers.
                let samples = map.as_slice_of::<i16>().map_err(|_| {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("Failed to interprete buffer as S16 PCM")
                    );

                    gst::FlowError::Error
                })?;

                // For buffer (= chunk of samples), we calculate the root mean square:
                // (https://en.wikipedia.org/wiki/Root_mean_square)
                let sum: f64 = samples
                    .iter()
                    .map(|sample| {
                        let f = f64::from(*sample) / f64::from(i16::MAX);
                        f * f
                    })
                    .sum();
                let rms = (sum / (samples.len() as f64)).sqrt();
                println!("rms: {}", rms);

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    Ok(pipeline)
}

fn main_loop(pipeline: gst::Pipeline) -> Result<(), Error> {
    trace!("set pipeline state to playing");
    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline
        .get_bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    trace!("entering main loop");
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null)?;
                return Err(ErrorMessage {
                    src: msg
                        .get_src()
                        .map(|s| String::from(s.get_path_string()))
                        .unwrap_or_else(|| String::from("None")),
                    error: err.get_error().to_string(),
                    debug: err.get_debug(),
                    source: err.get_error(),
                }
                .into());
            }
            _ => (),
        }
    }
    trace!("leaving main loop");

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}

pub fn run() {
    match create_pipeline().and_then(main_loop) {
        Ok(r) => r,
        Err(e) => trace!("{}:{}:{}", file!(), line!(), e),
    }
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    use dlopen::symbor::Library;
    use jni::objects::{JClass, JObject};
    use jni::sys::jint;
    use jni::{JNIEnv, JavaVM};
    use libc::{c_void, pthread_self};
    use std::fmt::Write;
    use std::sync::Mutex;

    use android_logger::Config;
    use log::Level;

    use glib::{Cast, ObjectExt};
    use gst::util_get_timestamp;
    use gst::{ClockTime, DebugCategory, DebugLevel, DebugMessage, GstObjectExt, Pad};

    lazy_static! {
        static ref RUNNING: Mutex<bool> = Mutex::new(false);
        static ref JAVA_VM: Mutex<Option<JavaVM>> = Mutex::new(None);
        static ref PLUGINS: Mutex<Vec<Library>> = Mutex::new(Vec::new());
        static ref PRIV_GST_INFO_START_TIME: Mutex<ClockTime> = Mutex::new(ClockTime::none());
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_tw_mapacode_androidsink_AndroidSink_nativeRun(
        _env: JNIEnv,
        _: JClass,
    ) {
        std::thread::spawn(move || {
            let mut running = RUNNING.lock().unwrap();
            *running = true;
            trace!("running");
            super::run();
            trace!("stopped running");
            *running = false;
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_org_freedesktop_gstreamer_GStreamer_nativeInit(
        _env: JNIEnv,
        _: JClass,
        _context: JObject,
    ) {
        trace!("GStreamer.init()");
    }

    fn print_info(msg: &str) {
        log!(Level::Info, "{}", msg);
    }

    fn print_error(msg: &str) {
        log!(Level::Error, "{}", msg);
    }

    fn log_target(target: &str, level: glib::LogLevel, msg: &str) {
        let l: Level = match level {
            glib::LogLevel::Error => Level::Error,
            glib::LogLevel::Critical => Level::Error,
            glib::LogLevel::Warning => Level::Warn,
            glib::LogLevel::Message => Level::Info,
            glib::LogLevel::Info => Level::Info,
            glib::LogLevel::Debug => Level::Debug,
        };
        log!(target: target, l, "{}", msg);
    }

    fn debug_logcat(
        category: DebugCategory,
        level: DebugLevel,
        file: &str,
        function: &str,
        line: u32,
        object: Option<&glib::object::Object>,
        message: &DebugMessage,
    ) {
        if level > category.get_threshold() {
            return;
        }

        let elapsed;
        match PRIV_GST_INFO_START_TIME.lock() {
            Ok(t) => {
                let now = util_get_timestamp();
                elapsed = now - *t;
                trace!("now: {}, start: {}, elapsed: {}", now, *t, elapsed);
            }
            Err(e) => {
                trace!("Cannot get PRIV_GST_INFO_START_TIME, {}", e);
                elapsed = ClockTime::none();
            }
        }

        let lvl = match level {
            DebugLevel::Error => Level::Error,
            DebugLevel::Warning => Level::Warn,
            DebugLevel::Info => Level::Info,
            DebugLevel::Debug => Level::Debug,
            _ => Level::Trace,
        };

        let tag = String::from("GStreamer+") + category.get_name();
        let mut label = String::new();
        match object {
            Some(obj) => {
                if obj.is::<Pad>() {
                    let pad = obj.downcast_ref::<Pad>().unwrap();
                    let name = pad.get_name();
                    let parent_name;
                    match pad.get_parent() {
                        Some(parent) => {
                            parent_name = parent.get_name().to_string();
                        }
                        None => {
                            parent_name = "".to_string();
                        }
                    }
                    write!(&mut label, "<{}:{}>", parent_name, name).unwrap();
                } else if obj.is::<gst::Object>() {
                    let ob = obj.downcast_ref::<gst::Object>().unwrap();
                    let name = ob.get_name().to_string();
                    write!(&mut label, "<{}>", name).unwrap();
                } else {
                    write!(&mut label, "<{}@{:#x?}>", obj.get_type(), obj).unwrap();
                }
                log!(
                    target: &tag,
                    lvl,
                    "{} {:#x?} {}:{}:{}:{} {}",
                    elapsed,
                    unsafe { pthread_self() },
                    file,
                    line,
                    function,
                    label,
                    message.get().unwrap()
                )
            }
            None => log!(
                target: &tag,
                lvl,
                "{} {:#x?} {}:{}:{} {}",
                elapsed,
                unsafe { pthread_self() },
                file,
                line,
                function,
                message.get().unwrap()
            ),
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn gst_android_get_java_vm() -> *const jni::sys::JavaVM {
        match JAVA_VM.lock() {
            Ok(opt_vm) => match &*opt_vm {
                Some(vm) => vm.get_java_vm_pointer(),
                None => {
                    trace!("Could not get jvm");
                    return std::ptr::null();
                }
            },
            Err(e) => {
                trace!("Could not get jvm, error: {}", e);
                return std::ptr::null();
            }
        }
    }

    #[no_mangle]
    unsafe fn JNI_OnLoad(jvm: JavaVM, _reserved: *mut c_void) -> jint {
        android_logger::init_once(
            Config::default()
                .with_min_level(Level::Trace)
                .with_tag("androidsink"),
        );

        trace!("get JNIEnv");

        let env: JNIEnv;
        match jvm.get_env() {
            Ok(v) => {
                env = v;
            }
            Err(e) => {
                trace!("Could not retrieve JNIEnv, error: {}", e);
                return 0;
            }
        }

        // TODO: check the version > JNI_VERSION_1_4

        trace!("get JNI version");

        let version: jint;
        match env.get_version() {
            Ok(v) => {
                version = v.into();
                trace!("JNI Version: {:#x?}", version);
            }
            Err(e) => {
                trace!("Could not retrieve JNI version, error: {}", e);
                return 0;
            }
        }

        trace!("find class GStreamer");

        match env.find_class("org/freedesktop/gstreamer/GStreamer") {
            Ok(_c) => {}
            Err(e) => {
                trace!(
                    "Could not retreive class org.freedesktop.gstreamer.GStreamer, error: {}",
                    e
                );
                return 0;
            }
        }

        trace!("save java vm");

        /* Remember Java VM */
        match JAVA_VM.lock() {
            Ok(mut vm) => *vm = Some(jvm),
            Err(e) => {
                trace!("Could not store jvm, error: {}", e);
                return 0;
            }
        }

        // Set GLIB print handlers
        trace!("set glib handlers");
        glib::set_print_handler(print_info);
        glib::set_printerr_handler(print_error);
        glib::log_set_default_handler(log_target);

        // Disable this for releases if performance is important
        // or increase the threshold to get more information
        gst::debug_set_active(true);
        gst::debug_set_default_threshold(gst::DebugLevel::Warning);
        gst::debug_remove_default_log_function();
        gst::debug_add_log_function(debug_logcat);

        match PRIV_GST_INFO_START_TIME.lock() {
            Ok(mut t) => {
                *t = util_get_timestamp();
            }
            Err(e) => {
                trace!("{}", e);
                return 0;
            }
        }

        trace!("gst init");
        match gst::init() {
            Ok(_) => { /* Do nothing. */ }
            Err(e) => {
                trace!("{}", e);
                return 0;
            }
        }

        {
            trace!("load plugins");
            let plugins = ["coreelements", "androidmedia", "audiotestsrc"];
            for name in &plugins {
                let mut so_name = String::from("libgst");
                so_name.push_str(name);
                so_name.push_str(".so");
                trace!("loading {}", so_name);
                match Library::open(&so_name) {
                    Ok(lib) => {
                        // Register plugin
                        let mut plugin_register = String::from("gst_plugin_");
                        plugin_register.push_str(name);
                        plugin_register.push_str("_register");
                        trace!("registering {}", so_name);
                        match lib.symbol::<unsafe extern "C" fn()>(&plugin_register) {
                            Ok(f) => f(),
                            Err(e) => {
                                trace!("{}", e);
                                return 0;
                            }
                        }
                        // Keep plugin
                        match PLUGINS.lock() {
                            Ok(mut p) => p.push(lib),
                            Err(e) => {
                                trace!("{}", e);
                                return 0;
                            }
                        }
                    }
                    Err(e) => {
                        trace!("{}", e);
                    }
                };
            }
        }

        version
    }
}
