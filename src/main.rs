#![allow(
    dead_code,
    unused_variables,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps
)]

use std::collections::HashSet;
use std::env;
use std::ffi::CStr;
use std::os::raw::c_void;

use anyhow::{anyhow, Result};
use log::*;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use vulkanalia::loader::{LibloadingLoader, LIBRARY};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::window as vk_window;
use vulkanalia::Version;

/// The Vulkan SDK version that started requiring the portability subset extension for macOS.
const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);

/// The required instance and device layer if validation is enabled.
const VALIDATION_LAYER: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Vulkan Tutorial (Rust)")
                        .with_inner_size(LogicalSize::new(1024, 768)),
                )
                .unwrap(),
        );
        unsafe { self.create() }.unwrap();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                unsafe {
                    self.destroy();
                }
            }
            WindowEvent::RedrawRequested if !event_loop.exiting() => {
                unsafe { self.render() }.unwrap();
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    // App

    let event_loop = EventLoop::new().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);

    Ok(())
}

/// Our Vulkan app.
#[derive(Default)]
struct App {
    window: Option<Window>,
    pub entry: Option<Entry>,
    pub instance: Option<Instance>,
    pub data: AppData,
}

impl App {
    /// Creates our Vulkan app.
    unsafe fn create(&mut self) -> Result<()> {
        info!("Starting app.");

        let loader = LibloadingLoader::new(LIBRARY)?;
        let mut data = AppData::default();
        let enable = env::var("VK_ENABLE_VALIDATION").unwrap_or_else(|_| "".into());
        data.validation = enable == "1" || enable == "true";
        if data.validation {
            info!("Validation layers requested.");
        }

        self.entry = Some(Entry::new(loader).map_err(|b| anyhow!("{}", b))?);
        self.instance = Some(create_instance(
            self.window.as_ref().unwrap(),
            &self.entry.as_ref().unwrap(),
            &mut data,
        )?);
        Ok(())
    }

    /// Renders a frame for our Vulkan app.
    unsafe fn render(&mut self) -> Result<()> {
        Ok(())
    }

    /// Destroys our Vulkan app.
    unsafe fn destroy(&mut self) {
        info!("Destroying app.");

        self.instance.as_ref().unwrap().destroy_instance(None);
    }
}

/// The Vulkan handles and associated properties used by our Vulkan app.
#[derive(Clone, Debug, Default)]
struct AppData {
    pub validation: bool,
}

//================================================
// Instance
//================================================

unsafe fn create_instance(window: &Window, entry: &Entry, data: &mut AppData) -> Result<Instance> {
    // Application Info
    let application_info = vk::ApplicationInfo::builder()
        .application_name(b"Vulkan Tutorial\0")
        .application_version(vk::make_version(1, 0, 0))
        .engine_name(b"No Engine\0")
        .engine_version(vk::make_version(1, 0, 0))
        .api_version(vk::make_version(1, 0, 0));

    // Layers
    let available_layers = entry
        .enumerate_instance_layer_properties()?
        .iter()
        .map(|l| l.layer_name)
        .collect::<HashSet<_>>();

    if data.validation && !available_layers.contains(&VALIDATION_LAYER) {
        return Err(anyhow!("Validation layers requested but not supported."));
    }

    let layers = if data.validation {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        Vec::new()
    };

    // Extensions
    let mut extensions = vk_window::get_required_instance_extensions(window)
        .iter()
        .map(|e| e.as_ptr())
        .collect::<Vec<_>>();

    if data.validation {
        extensions.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
    }

    // Required by Vulkan SDK on macOS since 1.3.216.
    let flags = if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
        info!("Enabling extensions for macOS portability.");
        extensions.push(
            vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION
                .name
                .as_ptr(),
        );
        extensions.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
        vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
    } else {
        vk::InstanceCreateFlags::empty()
    };

    // Info
    let mut info = vk::InstanceCreateInfo::builder()
        .application_info(&application_info)
        .enabled_layer_names(&layers)
        .enabled_extension_names(&extensions);

    // Needs to be defined outside of the conditional since it needs to live until we are done calling create_instance
    let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        // The message_severity field allows you to specify all the types of severities you would like your callback to be called for
        .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
        // Similarly the message_type field lets you filter which types of messages your callback is notified about
        .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
        // The user_callback field specifies the callback function
        .user_callback(Some(debug_callback));

    if data.validation {
        // push it onto info's pointer chain
        info = info.push_next(&mut debug_info);
    }

    // Create instance
    Ok(entry.create_instance(&info, None)?)
}

/// Logs debug messages.
extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    type_: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    // The data parameter refers to a vk::DebugUtilsMessengerCallbackDataEXT struct containing the details of the message itself
    // message – The debug message as a null-terminated string (*const c_char)
    // objects – Array of Vulkan object handles related to the message
    // object_count – Number of objects in array
    let data = unsafe { *data };
    let message = unsafe { CStr::from_ptr(data.message) }.to_string_lossy();

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        // Message about behavior that is invalid and may cause crashes
        error!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        // Message about behavior that is not necessarily an error, but very likely a bug in your application
        warn!("({:?}) {}", type_, message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        // Informational message like the creation of a resource
        debug!("({:?}) {}", type_, message);
    } else {
        // Diagnostic message
        trace!("({:?}) {}", type_, message);
    }

    // The callback returns a (Vulkan) boolean that indicates if the Vulkan call that triggered the validation layer message should be aborted.
    vk::FALSE
}
