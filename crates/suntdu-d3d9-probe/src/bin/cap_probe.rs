use std::ffi::c_void;
use std::mem::transmute;
use std::ptr;
use std::thread;
use std::time::Duration;

type HResult = i32;
type Bool = i32;

const D3D_SDK_VERSION: u32 = 32;
const D3DDEVTYPE_HAL: u32 = 1;
const D3DCREATE_HARDWARE_VERTEXPROCESSING: u32 = 0x40;
const D3DSWAPEFFECT_DISCARD: i32 = 1;
const D3DFMT_X8R8G8B8: i32 = 22;
const D3DFMT_DXT5: i32 = 0x3554_5844;

#[repr(C)]
struct D3dPresentParameters {
    back_buffer_width: u32,
    back_buffer_height: u32,
    back_buffer_format: i32,
    back_buffer_count: u32,
    multi_sample_type: i32,
    multi_sample_quality: u32,
    swap_effect: i32,
    device_window: *mut c_void,
    windowed: Bool,
    enable_auto_depth_stencil: Bool,
    auto_depth_stencil_format: i32,
    flags: u32,
    full_screen_refresh_rate_in_hz: u32,
    presentation_interval: u32,
}

type CreateDeviceFn = unsafe extern "system" fn(
    *mut c_void,
    u32,
    u32,
    *mut c_void,
    u32,
    *mut D3dPresentParameters,
    *mut *mut c_void,
) -> HResult;

type CreateTextureFn = unsafe extern "system" fn(
    *mut c_void,
    u32,
    u32,
    u32,
    u32,
    i32,
    u32,
    *mut *mut c_void,
    *mut *mut c_void,
) -> HResult;

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;

#[link(name = "d3d9")]
unsafe extern "system" {
    fn Direct3DCreate9(sdk_version: u32) -> *mut c_void;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: *mut c_void,
        menu: *mut c_void,
        instance: *mut c_void,
        param: *mut c_void,
    ) -> *mut c_void;
    fn DestroyWindow(window: *mut c_void) -> Bool;
}

unsafe fn vtable_entry(object: *mut c_void, index: usize) -> usize {
    let vtable = unsafe { *(object as *mut *mut usize) };
    unsafe { *vtable.add(index) }
}

unsafe fn release(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    let release_fn: ReleaseFn = unsafe { transmute(vtable_entry(object, 2)) };
    let _ = unsafe { release_fn(object) };
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

unsafe fn create_probe_device() -> Result<(*mut c_void, *mut c_void, *mut c_void), i32> {
    let class_name = wide("STATIC");
    let window_name = wide("SunTDU D3D9 Cap Probe");
    let window = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            0,
            0,
            0,
            64,
            64,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if window.is_null() {
        return Err(2);
    }

    let d3d = unsafe { Direct3DCreate9(D3D_SDK_VERSION) };
    if d3d.is_null() {
        let _ = unsafe { DestroyWindow(window) };
        return Err(3);
    }

    let create_device: CreateDeviceFn = unsafe { transmute(vtable_entry(d3d, 16)) };
    let mut params = D3dPresentParameters {
        back_buffer_width: 64,
        back_buffer_height: 64,
        back_buffer_format: D3DFMT_X8R8G8B8,
        back_buffer_count: 1,
        multi_sample_type: 0,
        multi_sample_quality: 0,
        swap_effect: D3DSWAPEFFECT_DISCARD,
        device_window: window,
        windowed: 1,
        enable_auto_depth_stencil: 0,
        auto_depth_stencil_format: 0,
        flags: 0,
        full_screen_refresh_rate_in_hz: 0,
        presentation_interval: 0,
    };
    let mut device = ptr::null_mut();
    let hr = unsafe {
        create_device(
            d3d,
            0,
            D3DDEVTYPE_HAL,
            window,
            D3DCREATE_HARDWARE_VERTEXPROCESSING,
            &mut params,
            &mut device,
        )
    };

    println!("CREATE_DEVICE_HR=0x{:08X}", hr as u32);
    if hr < 0 || device.is_null() {
        unsafe { release(d3d) };
        let _ = unsafe { DestroyWindow(window) };
        return Err(4);
    }

    Ok((window, d3d, device))
}

unsafe fn run_cap_tests(device: *mut c_void) {
    let create_texture: CreateTextureFn = unsafe { transmute(vtable_entry(device, 23)) };

    for pool in [0u32, 1u32] {
        for size in [2048u32, 4096u32, 8192u32] {
            let mut texture = ptr::null_mut();
            let texture_hr = unsafe {
                create_texture(
                    device,
                    size,
                    size,
                    0,
                    0,
                    D3DFMT_DXT5,
                    pool,
                    &mut texture,
                    ptr::null_mut(),
                )
            };
            println!(
                "ALLOC size={}x{} levels=full format=DXT5 pool={} hr=0x{:08X}",
                size, size, pool, texture_hr as u32
            );
            if texture_hr >= 0 && !texture.is_null() {
                unsafe { release(texture) };
            }
        }
    }

    let mut texture = ptr::null_mut();
    let mut shared_handle = ptr::null_mut();
    let shared_hr = unsafe {
        create_texture(
            device,
            1024,
            1024,
            1,
            0,
            D3DFMT_DXT5,
            0,
            &mut texture,
            &mut shared_handle,
        )
    };
    println!(
        "SHARED_CREATE size=1024x1024 pool=0 hr=0x{:08X} handle={shared_handle:p}",
        shared_hr as u32
    );
    if shared_hr >= 0 && !texture.is_null() {
        unsafe { release(texture) };
    }
}

unsafe fn run_hold_test(device: *mut c_void, pool: u32, size: u32, count: usize, seconds: u64) {
    let create_texture: CreateTextureFn = unsafe { transmute(vtable_entry(device, 23)) };
    let mut textures = Vec::with_capacity(count);

    for index in 0..count {
        let mut texture = ptr::null_mut();
        let hr = unsafe {
            create_texture(
                device,
                size,
                size,
                0,
                0,
                D3DFMT_DXT5,
                pool,
                &mut texture,
                ptr::null_mut(),
            )
        };
        println!(
            "HOLD_ALLOC index={} size={}x{} pool={} hr=0x{:08X}",
            index, size, size, pool, hr as u32
        );
        if hr >= 0 && !texture.is_null() {
            textures.push(texture);
        } else {
            break;
        }
    }

    println!(
        "HOLD_READY pool={} size={} count={} seconds={}",
        pool,
        size,
        textures.len(),
        seconds
    );
    thread::sleep(Duration::from_secs(seconds));

    for texture in textures {
        unsafe { release(texture) };
    }
}

fn main() {
    unsafe {
        let (window, d3d, device) = match create_probe_device() {
            Ok(value) => value,
            Err(code) => std::process::exit(code),
        };

        let args: Vec<String> = std::env::args().collect();
        if args.get(1).map(String::as_str) == Some("hold") {
            let pool = args
                .get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let size = args
                .get(3)
                .and_then(|value| value.parse().ok())
                .unwrap_or(8192);
            let count = args
                .get(4)
                .and_then(|value| value.parse().ok())
                .unwrap_or(1);
            let seconds = args
                .get(5)
                .and_then(|value| value.parse().ok())
                .unwrap_or(10);
            run_hold_test(device, pool, size, count, seconds);
        } else {
            run_cap_tests(device);
        }

        release(device);
        release(d3d);
        let _ = DestroyWindow(window);
    }
}
