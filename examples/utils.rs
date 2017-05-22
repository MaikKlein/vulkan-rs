/*
Copyright (c) 2016, Christoph Hommelsheim
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this
  list of conditions and the following disclaimer.

* Redistributions in binary form must reproduce the above copyright notice,
  this list of conditions and the following disclaimer in the documentation
  and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

use winit;
use std::os::raw::c_char;
use std::ffi::CString;
use vulkan_rs::prelude::*;

macro_rules! vk_try {
    ( $($e:expr );+ ) => {
        $(
            trace!("start: vk_try: {} @ line {}", stringify!($e), line!());
            let res = unsafe { $e };
            trace!("end: vk_try: {} @ line {}", stringify!($e), line!());
            if res != VK_SUCCESS {
                let e : VkError = res.into();
                error!("{} @ line {}\n\t{}", e, line!(), stringify!($e));
                return Err(e);
            }
        )+
    };
}

macro_rules! vk_get_try {
    ( $f:path ; $($e:expr),* ; $t:ty ) => {
        {
          let mut handle : $t = Default::default();
          vk_try!( $f ( $($e ,)* &mut handle) );
          handle
        }
    };
}

macro_rules! vk_get {
    ( $f:path ; $($e:expr),* ; $t:ty ) => {
        {
          let mut handle : $t = Default::default();
          trace!("start: vk_get: {} @ line {}", stringify!($f ( $($e ,)* &mut handle)), line!());
          unsafe {
            $f ( $($e ,)* &mut handle);
          }
          trace!("end: vk_get: {} @ line {}", stringify!($f ( $($e ,)* &mut handle)), line!());
          handle
        }
    };
}

macro_rules! vk_drop {
    ( $f:path ; [ $($p:expr),* ] $e:expr $(, $r:expr)* ) => {
        if $e != vk_null_handle() {
            trace!("start: vk_drop: {} @ line {}", stringify!($f ( $($p,)* $e $(, $r)* )), line!());
            unsafe {
                $f ( $($p,)* $e $(, $r)* );
            }
            trace!("end: vk_drop: {} @ line {}", stringify!($f ( $($p,)* $e $(, $r)* )), line!());
            $e = vk_null_handle();
        }
    };
}

macro_rules! vk_vec_try {
    ( $f:path ; $($e:expr),* ; $t:ty ) => {
        {
          let mut count : u32 = 0;
          vk_try!( $f ( $($e ,)* &mut count, vk_null()) );
          let mut data: Vec<$t> = Vec::with_capacity(count as usize);
          if count > 0 {
              vk_try!( $f ( $($e ,)* &mut count, data.as_mut_ptr()) );
              unsafe { data.set_len(count as usize) };
          }
          data
        }
    };
}

macro_rules! vk_vec {
    ( $f:path ; $($e:expr),* ; $t:ty ) => {
        {
          let mut count : u32 = 0;
          unsafe {
              $f ( $($e ,)* &mut count, vk_null());
          }
          let mut data: Vec<$t> = Vec::with_capacity(count as usize);
          if count > 0 {
            unsafe {
              $f ( $($e ,)* &mut count, data.as_mut_ptr() );
              data.set_len(count as usize);
            }
          }
          data
        }
    };
}

const QUEUE_COUNT : usize = 2;
const GRAPHIC_QUEUE: usize = 0;
const PRESENT_QUEUE: usize = 1;
const INVALID_INDEX : u32 = (-1i32) as u32;

#[derive(Default)]
pub struct Application {
    instance: VkInstance,
    surface: VkSurfaceKHR,
    physical_device: VkPhysicalDevice,
    queues: [VkQueue; QUEUE_COUNT],
    device: VkDevice,
}

#[cfg(target_os = "windows")]
fn get_required_instance_extensions(w: &winit::Window) -> Vec<&'static str> {
    return vec![VK_KHR_SURFACE_EXTENSION_NAME,
                VK_KHR_WIN32_SURFACE_EXTENSION_NAME];
}

#[cfg(target_os = "linux")]
fn get_required_instance_extensions(w: &winit::Window) -> Vec<&'static str> {
    use winit::os::unix::WindowExt;
    if let Some(_) = (w as &WindowExt).get_wayland_display() {
        return vec![VK_KHR_SURFACE_EXTENSION_NAME,
                    VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME];
    } else if let Some(_) = (w as &WindowExt).get_xlib_display() {
        return vec![VK_KHR_SURFACE_EXTENSION_NAME,
                    VK_KHR_XLIB_SURFACE_EXTENSION_NAME];
    } /* else if let Some(_) = (w as &WindowExt).get_xcb_connection() {
        return vec![VK_KHR_SURFACE_EXTENSION_NAME,
                    VK_KHR_XCB_SURFACE_EXTENSION_NAME];
    } */ else {
        return vec![VK_KHR_SURFACE_EXTENSION_NAME];
    }
}

#[cfg(target_os = "android")]
fn get_required_instance_extensions(w: &winit::Window) -> Vec<&'static str> {
    return vec![VK_KHR_SURFACE_EXTENSION_NAME,
                VK_KHR_ANDROID_SURFACE_EXTENSION_NAME];
}

fn get_required_device_extensions() -> Vec<&'static str> {
    return vec![VK_KHR_SWAPCHAIN_EXTENSION_NAME];
}

#[cfg(target_os = "windows")]
fn create_surface(instance: VkInstance, w: &winit::Window) -> VkResultObj<VkSurfaceKHR> {
    use kernel32;
    let create_info = VkWin32SurfaceCreateInfoKHR {
        sType: VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR,
        pNext: vk_null(),
        flags: 0,
        hinstance: kernel32::GetModuleHandleW(ptr::null()),
        hwnd: w.get_hwnd(),
    };
    let surface = vk_get_try!(vkCreateWin32SurfaceKHR; instance, &create_info, vk_null(); VkSurfaceKHR);
    debug!("created windows surface {:?}", surface);
    return Ok(surface);
}

#[cfg(target_os = "linux")]
fn create_surface(instance: VkInstance, w: &winit::Window) -> VkResultObj<VkSurfaceKHR> {
    use winit::os::unix::WindowExt;
    if let Some(wayland_display) = w.get_wayland_display() {
        let wayland_surface = w.get_wayland_surface().unwrap();
        debug!("wayland display is {}; wayland surface is {}", wayland_display as usize, wayland_surface as usize);
        let create_info = VkWaylandSurfaceCreateInfoKHR {
            sType: VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR,
            pNext: vk_null(),
            flags: 0,
            display: wayland_display as *mut vk_platform::wayland::wl_display,
            surface: wayland_surface as *mut vk_platform::wayland::wl_surface,
        };
        let surface = vk_get_try!(vkCreateWaylandSurfaceKHR; instance, &create_info, vk_null(); VkSurfaceKHR);
        debug!("created wayland surface {:?}", surface);
        return Ok(surface);
    } else if let Some(xlib_display) = w.get_xlib_display() {
        let xlib_window = w.get_xlib_window().unwrap();
        debug!("xlib display is {}; xlib window is {}", xlib_display as usize, xlib_window as usize);
        let create_info = VkXlibSurfaceCreateInfoKHR {
            sType: VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR,
            pNext: vk_null(),
            flags: 0,
            dpy: xlib_display as *mut vk_platform::xlib::Display,
            window: xlib_window as vk_platform::xlib::Window,
        };
        let surface = vk_get_try!(vkCreateXlibSurfaceKHR; instance, &create_info, vk_null(); VkSurfaceKHR);
        debug!("created xlib surface {:?}", surface);
        return Ok(surface);
    } /* else if let Some(xcb_connection) = w.get_xcb_connection() {
        let xcb_window = w.get_xlib_window().unwrap();
        debug!("xcb connection is {}; xcb window is {}", xcb_connection as usize, xcb_window as usize);
        let create_info = VkXcbSurfaceCreateInfoKHR {
            sType: VK_STRUCTURE_TYPE_XCB_SURFACE_CREATE_INFO_KHR,
            pNext: vk_null(),
            flags: 0,
            connection: xcb_connection as *mut vk_platform::xcb::xcb_connection_t,
            window: xcb_window as vk_platform::xcb::xcb_window_t,
        };
        let surface = vk_get_try!(vkCreateXcbSurfaceKHR; instance, &create_info, vk_null(); VkSurfaceKHR);
        debug!("created xcb surface {:?}", surface);
        return Ok(surface);
    } */ else {
        return Err(VK_ERROR_EXTENSION_NOT_PRESENT.into());
    }
}


#[cfg(target_os = "android")]
fn create_surface(instance: VkInstance, w: &winit::Window) -> VkResultObj<VkSurfaceKHR> {
    use kernel32;
    let create_info = VkAndroidSurfaceCreateInfoKHR {
        sType: VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR,
        pNext: vk_null(),
        flags: 0,
        window: w.get_native_window() as *mut vk_platform::android::ANativeWindow,
    };
    let surface = vk_get_try!(vkCreateAndroidSurfaceKHR; instance, &create_info, vk_null(); VkSurfaceKHR);
    debug!("created android surface {:?}", surface);
    return Ok(surface);
}

fn choose_queue_family_indices(physical_device: VkPhysicalDevice, surface: VkSurfaceKHR) -> [u32;2] {
    let mut result : [u32;QUEUE_COUNT] = [INVALID_INDEX, INVALID_INDEX];
    let queue_family_props = vk_vec!(vkGetPhysicalDeviceQueueFamilyProperties; physical_device; VkQueueFamilyProperties);
    debug!("got {} queue family properties", queue_family_props.len());

    for (i, props) in queue_family_props.iter().enumerate() {
        debug!("querying queue family {}: queues={}, flags={}", i, props.queueCount, props.queueFlags);
        if props.queueCount > 0 {
            let has_surface_support = if surface != vk_null_handle() {
                vk_get!(vkGetPhysicalDeviceSurfaceSupportKHR; physical_device, i as u32, surface  ; VkBool32)
            } else {
                VK_FALSE
            };
            debug!("has_surface_support={}", has_surface_support);

            if (props.queueFlags&VK_QUEUE_GRAPHICS_BIT)!=0 {
                if result[GRAPHIC_QUEUE] == INVALID_INDEX {
                    result[GRAPHIC_QUEUE] = i as u32;
                }
                if has_surface_support != VK_FALSE {
                    result[GRAPHIC_QUEUE] = i as u32;
                    result[PRESENT_QUEUE] = i as u32;
                    break;
                }
            }

            if has_surface_support != VK_FALSE && result[PRESENT_QUEUE] == INVALID_INDEX {
                result[PRESENT_QUEUE] = i as u32;
            }
        }
    }
    return result;
}

fn get_device_queues(device: VkDevice, queue_family_indices: &[u32; QUEUE_COUNT]) -> [VkQueue; QUEUE_COUNT] {
    let mut result : [VkQueue; QUEUE_COUNT] = [vk_null_handle(), vk_null_handle()];
    for i in 0..QUEUE_COUNT {
        if queue_family_indices[i] != INVALID_INDEX {
            result[i] = vk_get!(vkGetDeviceQueue; device, queue_family_indices[i], 0; VkQueue);
            debug!("created device queue {:?} for index {}", result[i], i);
        }
    }
    return result;
}

fn create_instance(app_aame: &str, exts: &[&str]) -> VkResultObj<VkInstance> {
    let app_aame = CString::new(app_aame).unwrap();
    let exts: Vec<CString> = exts.iter().map(|s| CString::new(*s).unwrap()).collect();
    let exts_p: Vec<*const c_char> = exts.iter().map(|s| s.as_ptr()).collect();
    let app_info = VkApplicationInfo {
        sType: VK_STRUCTURE_TYPE_APPLICATION_INFO,
        pNext: vk_null(),
        pApplicationName: app_aame.as_ptr(),
        applicationVersion: 1,
        pEngineName: app_aame.as_ptr(),
        engineVersion: 1,
        apiVersion: VK_API_VERSION_1_0,
    };
    let create_info = VkInstanceCreateInfo {
        sType: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        pNext: vk_null(),
        flags: 0,
        pApplicationInfo: &app_info,
        enabledLayerCount: 0,
        ppEnabledLayerNames: vk_null(),
        enabledExtensionCount: exts_p.len() as u32,
        ppEnabledExtensionNames: exts_p.as_ptr(),
    };
    let instance = vk_get_try!(vkCreateInstance; &create_info, vk_null(); VkInstance);
    debug!("created instalce {:?}", instance);
    return Ok(instance);
}

fn choose_physical_device(instance: VkInstance) -> VkResultObj<VkPhysicalDevice> {
    let devices = vk_vec_try!(vkEnumeratePhysicalDevices; instance; VkPhysicalDevice);
    if devices.len() <= 0 {
        return Err(VK_ERROR_INITIALIZATION_FAILED.into());
    }
    let device = devices[0]; // return first:
    // TODO: choose via scoring
    // https://vulkan-tutorial.com/Drawing_a_triangle/Setup/Physical_devices_and_queue_families
    debug!("choosed physical device {:?}", device);
    return Ok(device);
}

fn create_logical_device(physical_device: VkPhysicalDevice, queue_family_indices: &[u32], exts: &[&str]) -> VkResultObj<VkDevice> {
    let queue_family_indices : ::std::collections::HashSet<u32> = queue_family_indices.iter().cloned().collect(); // make unique
    let exts: Vec<CString> = exts.iter().map(|s| CString::new(*s).unwrap()).collect();
    let exts_p: Vec<*const c_char> = exts.iter().map(|s| s.as_ptr()).collect();

    let queue_priorities = [0.0f32];
    let queue_create_info : Vec<VkDeviceQueueCreateInfo> = queue_family_indices.iter()
        .map(|family_index| VkDeviceQueueCreateInfo {
            sType: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
            pNext: vk_null(),
            flags: 0,
            queueCount: queue_priorities.len() as u32,
            pQueuePriorities: queue_priorities.as_ptr(),
            queueFamilyIndex: *family_index,
        }).collect();
    let device_create_info = VkDeviceCreateInfo{
        sType: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        pNext: vk_null(),
        flags: 0,
        queueCreateInfoCount: queue_create_info.len() as u32,
        pQueueCreateInfos: queue_create_info.as_ptr(),
        enabledLayerCount: 0,
        ppEnabledLayerNames: vk_null(),
        enabledExtensionCount: exts_p.len() as u32,
        ppEnabledExtensionNames: exts_p.as_ptr(),
        pEnabledFeatures: vk_null(),
    };
    let device = vk_get_try!(vkCreateDevice; physical_device, &device_create_info, vk_null() ; VkDevice);
    debug!("created device {:?}", device);
    return Ok(device);
}

impl Application {
    pub fn new(app_name: &str, window: &winit::Window) -> VkResultObj<Application> {
        let mut app : Application = Default::default();
        let instance_exts = get_required_instance_extensions(window);
        let device_exts = get_required_device_extensions();
        app.instance = try!(create_instance(app_name, instance_exts.as_slice()));
        app.surface = try!(create_surface(app.instance, window));
        app.physical_device = try!(choose_physical_device(app.instance));
        let mem_props = vk_get!(vkGetPhysicalDeviceMemoryProperties; app.physical_device; VkPhysicalDeviceMemoryProperties);
        let dev_props = vk_get!(vkGetPhysicalDeviceProperties; app.physical_device; VkPhysicalDeviceProperties);
        debug!("props: memoryTypeCount: {}, memoryHeapCount: {}, apiVersion: {}, driverVersion: {}, vendorID: {}, deviceID: {}", mem_props.memoryTypeCount, mem_props.memoryHeapCount, dev_props.apiVersion, dev_props.driverVersion, dev_props.vendorID, dev_props.deviceID);
        let queue_family_indices = choose_queue_family_indices(app.physical_device, app.surface);
        app.device = try!(create_logical_device(app.physical_device, &queue_family_indices, device_exts.as_slice()));
        app.queues = get_device_queues(app.device, &queue_family_indices);
        return Ok(app);
    }

    pub fn dispose(&mut self) {
        vk_drop!(vkDestroyDevice; [] self.device, vk_null());
        vk_drop!(vkDestroySurfaceKHR; [self.instance] self.surface, vk_null());
        vk_drop!(vkDestroyInstance; [] self.instance, vk_null());
        self.queues = [vk_null_handle(), vk_null_handle()];
        self.physical_device = vk_null_handle();
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.dispose()
    }
}