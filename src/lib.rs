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
    gst::init()?;

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
        Err(e) => trace!("Error! {}", e),
    }
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    use jni::objects::JClass;
    use jni::sys::jint;
    use jni::{JNIEnv, JNIVersion, JavaVM};
    use libc::c_void;
    use std::sync::Mutex;

    use android_logger::Config;
    use log::Level;

    lazy_static! {
        static ref RUNNING: Mutex<bool> = Mutex::new(false);
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
    unsafe fn JNI_OnLoad(jvm: JavaVM, _reserved: *mut c_void) -> jint {
        android_logger::init_once(
            Config::default()
                .with_min_level(Level::Trace)
                .with_tag("androidsink"),
        );
        trace!("onload");

        let env;
        match jvm.get_env() {
            Ok(v) => {
                env = v;
            }
            Err(e) => {
                trace!("Error: {}", e);
                return 0;
            }
        }

        let version: jint;
        match env.get_version() {
            Ok(v) => {
                version = v.into();
                trace!("JNI Version: {:#x?}", version);
            }
            Err(e) => {
                trace!("Error: {}", e);
                return 0;
            }
        }

        JNIVersion::V4.into()
    }
}
