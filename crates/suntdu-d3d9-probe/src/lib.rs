#![allow(non_snake_case, clippy::missing_safety_doc, function_casts_as_integer)]

use std::collections::HashSet;
use std::ffi::{c_char, c_void};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::{size_of, transmute};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

type HResult = i32;
type Bool = i32;

#[repr(C)]
struct ProcessMemoryCountersEx {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
}

#[repr(C)]
struct LockedRect {
    pitch: i32,
    bits: *mut c_void,
}

#[repr(C)]
struct D3dSurfaceDesc {
    format: i32,
    resource_type: i32,
    usage: u32,
    pool: u32,
    multi_sample_type: i32,
    multi_sample_quality: u32,
    width: u32,
    height: u32,
}

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
type LockRectFn =
    unsafe extern "system" fn(*mut c_void, u32, *mut LockedRect, *const c_void, u32) -> HResult;
type UnlockRectFn = unsafe extern "system" fn(*mut c_void, u32) -> HResult;
type SetTextureFn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> HResult;
type UpdateTextureFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HResult;
type GetTextureTypeFn = unsafe extern "system" fn(*mut c_void) -> i32;
type GetTextureLevelCountFn = unsafe extern "system" fn(*mut c_void) -> u32;
type GetTextureLevelDescFn =
    unsafe extern "system" fn(*mut c_void, u32, *mut D3dSurfaceDesc) -> HResult;

const TEXTURE_RELEASE_INDEX: usize = 2;
const TEXTURE_GET_TYPE_INDEX: usize = 10;
const TEXTURE_GET_LEVEL_COUNT_INDEX: usize = 13;
const TEXTURE_GET_LEVEL_DESC_INDEX: usize = 17;
const TEXTURE_LOCK_RECT_INDEX: usize = 19;
const TEXTURE_UNLOCK_RECT_INDEX: usize = 20;
const D3DFMT_DXT5: i32 = 0x3554_5844;
const D3DUSAGE_DYNAMIC: u32 = 0x0000_0200;
const D3DLOCK_DISCARD: u32 = 0x0000_2000;
const D3DRTYPE_TEXTURE: i32 = 3;
const D3DRTYPE_CUBETEXTURE: i32 = 5;

const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const D3D9_CREATE_DEVICE_INDEX: usize = 16;
const DEVICE_CREATE_TEXTURE_INDEX: usize = 23;
const DEVICE_CREATE_VOLUME_TEXTURE_INDEX: usize = 24;
const DEVICE_CREATE_CUBE_TEXTURE_INDEX: usize = 25;
const DEVICE_UPDATE_TEXTURE_INDEX: usize = 31;
const DEVICE_SET_TEXTURE_INDEX: usize = 65;

static REAL_D3D9: OnceLock<usize> = OnceLock::new();
static REAL_DIRECT3D_CREATE9: OnceLock<usize> = OnceLock::new();
static REAL_D3DPERF_SET_OPTIONS: OnceLock<usize> = OnceLock::new();
static ORIGINAL_CREATE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CREATE_TEXTURE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CREATE_VOLUME_TEXTURE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CREATE_CUBE_TEXTURE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_SET_TEXTURE: AtomicUsize = AtomicUsize::new(0);
static SEEN_BOUND_TEXTURES: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
static LOG: OnceLock<Option<Mutex<File>>> = OnceLock::new();

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

type Direct3DCreate9Fn = unsafe extern "system" fn(u32) -> *mut c_void;
type D3dPerfSetOptionsFn = unsafe extern "system" fn(u32);

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

type CreateVolumeTextureFn = unsafe extern "system" fn(
    *mut c_void,
    u32,
    u32,
    u32,
    u32,
    u32,
    i32,
    u32,
    *mut *mut c_void,
    *mut *mut c_void,
) -> HResult;

type CreateCubeTextureFn = unsafe extern "system" fn(
    *mut c_void,
    u32,
    u32,
    u32,
    i32,
    u32,
    *mut *mut c_void,
    *mut *mut c_void,
) -> HResult;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
    fn LoadLibraryW(file_name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
    fn VirtualProtect(
        address: *mut c_void,
        size: usize,
        new_protect: u32,
        old_protect: *mut u32,
    ) -> Bool;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentProcess() -> *mut c_void;
}

#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        process: *mut c_void,
        counters: *mut ProcessMemoryCountersEx,
        size: u32,
    ) -> Bool;
}

fn log_path() -> PathBuf {
    std::env::temp_dir().join("suntdu-d3d9-probe.log")
}

fn log_file() -> &'static Option<Mutex<File>> {
    LOG.get_or_init(|| {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path())
            .ok()
            .map(Mutex::new)
    })
}

fn log_line(line: &str) {
    let Some(lock) = log_file() else {
        return;
    };
    let Ok(mut file) = lock.lock() else {
        return;
    };
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
}

unsafe fn real_d3d9_module() -> *mut c_void {
    let value = *REAL_D3D9.get_or_init(|| unsafe {
        let mut buffer = vec![0u16; 1024];
        let len = GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) as usize;
        if len == 0 || len >= buffer.len() {
            return 0;
        }
        buffer.truncate(len);
        buffer.extend("\\d3d9.dll".encode_utf16());
        buffer.push(0);
        LoadLibraryW(buffer.as_ptr()) as usize
    });
    value as *mut c_void
}

unsafe fn real_symbol(name: &'static [u8]) -> usize {
    let module = unsafe { real_d3d9_module() };
    if module.is_null() {
        return 0;
    }
    unsafe { GetProcAddress(module, name.as_ptr().cast()) as usize }
}

unsafe fn patch_vtable_entry(
    object: *mut c_void,
    index: usize,
    replacement: usize,
    original: &AtomicUsize,
) {
    if object.is_null() {
        return;
    }

    let vtable = unsafe { *(object as *mut *mut usize) };
    if vtable.is_null() {
        return;
    }

    let slot = unsafe { vtable.add(index) };
    let current = unsafe { *slot };
    if current == replacement {
        return;
    }

    let _ = original.compare_exchange(0, current, Ordering::SeqCst, Ordering::SeqCst);

    let mut old_protect = 0u32;
    let changed = unsafe {
        VirtualProtect(
            slot.cast(),
            size_of::<usize>(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };
    if changed == 0 {
        return;
    }

    unsafe {
        ptr::write(slot, replacement);
    }

    let mut ignored = 0u32;
    let _ = unsafe { VirtualProtect(slot.cast(), size_of::<usize>(), old_protect, &mut ignored) };
}

unsafe fn hook_device(device: *mut c_void) {
    unsafe {
        patch_vtable_entry(
            device,
            DEVICE_CREATE_TEXTURE_INDEX,
            hook_create_texture as usize,
            &ORIGINAL_CREATE_TEXTURE,
        );
        patch_vtable_entry(
            device,
            DEVICE_CREATE_VOLUME_TEXTURE_INDEX,
            hook_create_volume_texture as usize,
            &ORIGINAL_CREATE_VOLUME_TEXTURE,
        );
        patch_vtable_entry(
            device,
            DEVICE_CREATE_CUBE_TEXTURE_INDEX,
            hook_create_cube_texture as usize,
            &ORIGINAL_CREATE_CUBE_TEXTURE,
        );
        if bind_probe_enabled() {
            patch_vtable_entry(
                device,
                DEVICE_SET_TEXTURE_INDEX,
                hook_set_texture as usize,
                &ORIGINAL_SET_TEXTURE,
            );
        }
    }
}

unsafe extern "system" fn hook_create_device(
    this: *mut c_void,
    adapter: u32,
    device_type: u32,
    focus_window: *mut c_void,
    behavior_flags: u32,
    params: *mut D3dPresentParameters,
    returned_device: *mut *mut c_void,
) -> HResult {
    let original = ORIGINAL_CREATE_DEVICE.load(Ordering::SeqCst);
    if original == 0 {
        return -1;
    }
    let original: CreateDeviceFn = unsafe { transmute(original) };

    if !params.is_null() {
        let pp = unsafe { &*params };
        log_line(&format!(
            "CREATE_DEVICE request adapter={} type={} behavior=0x{:08X} windowed={} backbuffer={}x{} format={} count={} refresh={} interval=0x{:08X}",
            adapter,
            device_type,
            behavior_flags,
            pp.windowed,
            pp.back_buffer_width,
            pp.back_buffer_height,
            pp.back_buffer_format,
            pp.back_buffer_count,
            pp.full_screen_refresh_rate_in_hz,
            pp.presentation_interval
        ));
    }

    let hr = unsafe {
        original(
            this,
            adapter,
            device_type,
            focus_window,
            behavior_flags,
            params,
            returned_device,
        )
    };

    let device = if hr >= 0 && !returned_device.is_null() {
        unsafe { *returned_device }
    } else {
        ptr::null_mut()
    };

    log_line(&format!(
        "CREATE_DEVICE result hr=0x{:08X} device={device:p}",
        hr as u32
    ));

    if !device.is_null() {
        unsafe { hook_device(device) };
        if allocation_probe_enabled() {
            unsafe { run_allocation_probe(device) };
        }
        if upload_probe_enabled() {
            unsafe { run_upload_probe(device) };
        }
    }

    hr
}

unsafe extern "system" fn hook_create_texture(
    this: *mut c_void,
    width: u32,
    height: u32,
    levels: u32,
    usage: u32,
    format: i32,
    pool: u32,
    texture: *mut *mut c_void,
    shared_handle: *mut *mut c_void,
) -> HResult {
    let original = ORIGINAL_CREATE_TEXTURE.load(Ordering::SeqCst);
    if original == 0 {
        return -1;
    }
    let original: CreateTextureFn = unsafe { transmute(original) };
    let hr = unsafe {
        original(
            this,
            width,
            height,
            levels,
            usage,
            format,
            pool,
            texture,
            shared_handle,
        )
    };
    log_line(&format!(
        "CREATE_TEXTURE {}x{} levels={} usage=0x{:08X} format={} pool={} shared_ptr={} hr=0x{:08X}",
        width,
        height,
        levels,
        usage,
        format,
        pool,
        !shared_handle.is_null(),
        hr as u32
    ));
    hr
}

unsafe extern "system" fn hook_create_volume_texture(
    this: *mut c_void,
    width: u32,
    height: u32,
    depth: u32,
    levels: u32,
    usage: u32,
    format: i32,
    pool: u32,
    texture: *mut *mut c_void,
    shared_handle: *mut *mut c_void,
) -> HResult {
    let original = ORIGINAL_CREATE_VOLUME_TEXTURE.load(Ordering::SeqCst);
    if original == 0 {
        return -1;
    }
    let original: CreateVolumeTextureFn = unsafe { transmute(original) };
    let hr = unsafe {
        original(
            this,
            width,
            height,
            depth,
            levels,
            usage,
            format,
            pool,
            texture,
            shared_handle,
        )
    };
    log_line(&format!(
        "CREATE_VOLUME_TEXTURE {}x{}x{} levels={} usage=0x{:08X} format={} pool={} shared_ptr={} hr=0x{:08X}",
        width,
        height,
        depth,
        levels,
        usage,
        format,
        pool,
        !shared_handle.is_null(),
        hr as u32
    ));
    hr
}

unsafe extern "system" fn hook_create_cube_texture(
    this: *mut c_void,
    edge_length: u32,
    levels: u32,
    usage: u32,
    format: i32,
    pool: u32,
    texture: *mut *mut c_void,
    shared_handle: *mut *mut c_void,
) -> HResult {
    let original = ORIGINAL_CREATE_CUBE_TEXTURE.load(Ordering::SeqCst);
    if original == 0 {
        return -1;
    }
    let original: CreateCubeTextureFn = unsafe { transmute(original) };
    let hr = unsafe {
        original(
            this,
            edge_length,
            levels,
            usage,
            format,
            pool,
            texture,
            shared_handle,
        )
    };
    log_line(&format!(
        "CREATE_CUBE_TEXTURE edge={} levels={} usage=0x{:08X} format={} pool={} shared_ptr={} hr=0x{:08X}",
        edge_length,
        levels,
        usage,
        format,
        pool,
        !shared_handle.is_null(),
        hr as u32
    ));
    hr
}

fn bind_probe_enabled() -> bool {
    std::env::var_os("SUNTDU_D3D9_BIND_PROBE").is_some()
}

fn mark_bound_texture_seen(texture: *mut c_void) -> bool {
    let seen = SEEN_BOUND_TEXTURES.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut seen) = seen.lock() else {
        return false;
    };
    seen.insert(texture as usize)
}

unsafe fn log_bound_texture(stage: u32, texture: *mut c_void) {
    if texture.is_null() || !mark_bound_texture_seen(texture) {
        return;
    }

    let get_type_address = unsafe { texture_vtable_method(texture, TEXTURE_GET_TYPE_INDEX) };
    let get_levels_address =
        unsafe { texture_vtable_method(texture, TEXTURE_GET_LEVEL_COUNT_INDEX) };
    if get_type_address == 0 || get_levels_address == 0 {
        return;
    }

    let get_type: GetTextureTypeFn = unsafe { transmute(get_type_address) };
    let get_levels: GetTextureLevelCountFn = unsafe { transmute(get_levels_address) };
    let resource_type = unsafe { get_type(texture) };
    let levels = unsafe { get_levels(texture) };

    if resource_type != D3DRTYPE_TEXTURE && resource_type != D3DRTYPE_CUBETEXTURE {
        log_line(&format!(
            "BOUND_TEXTURE stage={} ptr={texture:p} type={} levels={}",
            stage, resource_type, levels
        ));
        return;
    }

    let get_desc_address = unsafe { texture_vtable_method(texture, TEXTURE_GET_LEVEL_DESC_INDEX) };
    if get_desc_address == 0 {
        return;
    }
    let get_desc: GetTextureLevelDescFn = unsafe { transmute(get_desc_address) };
    let mut desc = D3dSurfaceDesc {
        format: 0,
        resource_type: 0,
        usage: 0,
        pool: 0,
        multi_sample_type: 0,
        multi_sample_quality: 0,
        width: 0,
        height: 0,
    };
    let hr = unsafe { get_desc(texture, 0, &mut desc) };
    log_line(&format!(
        "BOUND_TEXTURE stage={} ptr={texture:p} type={} levels={} desc_hr=0x{:08X} size={}x{} format={} usage=0x{:08X} pool={}",
        stage,
        resource_type,
        levels,
        hr as u32,
        desc.width,
        desc.height,
        desc.format,
        desc.usage,
        desc.pool
    ));
}

unsafe extern "system" fn hook_set_texture(
    this: *mut c_void,
    stage: u32,
    texture: *mut c_void,
) -> HResult {
    let original = ORIGINAL_SET_TEXTURE.load(Ordering::SeqCst);
    if original == 0 {
        return -1;
    }

    if !texture.is_null() {
        unsafe { log_bound_texture(stage, texture) };
    }

    let original: SetTextureFn = unsafe { transmute(original) };
    unsafe { original(this, stage, texture) }
}

fn allocation_probe_enabled() -> bool {
    std::env::var_os("SUNTDU_D3D9_ALLOC_PROBE").is_some()
}

fn upload_probe_enabled() -> bool {
    std::env::var_os("SUNTDU_D3D9_UPLOAD_PROBE").is_some()
}

unsafe fn run_upload_probe(device: *mut c_void) {
    let create_address = ORIGINAL_CREATE_TEXTURE.load(Ordering::SeqCst);
    let update_address = unsafe { texture_vtable_method(device, DEVICE_UPDATE_TEXTURE_INDEX) };
    if create_address == 0 || update_address == 0 {
        log_line("UPLOAD_PROBE unavailable: required device methods missing");
        return;
    }

    let create: CreateTextureFn = unsafe { transmute(create_address) };
    let update: UpdateTextureFn = unsafe { transmute(update_address) };
    let baseline = unsafe { process_memory_private_bytes() };

    let mut source = ptr::null_mut();
    let source_hr = unsafe {
        create(
            device,
            4096,
            4096,
            1,
            0,
            D3DFMT_DXT5,
            2,
            &mut source,
            ptr::null_mut(),
        )
    };
    let source_lock_hr = if source_hr >= 0 && !source.is_null() {
        unsafe { touch_dxt5_base_level(source, 4096, 4096, 0) }
    } else {
        -1
    };

    let mut destination = ptr::null_mut();
    let destination_hr = unsafe {
        create(
            device,
            4096,
            4096,
            1,
            0,
            D3DFMT_DXT5,
            0,
            &mut destination,
            ptr::null_mut(),
        )
    };
    let destination_lock_hr = if destination_hr >= 0 && !destination.is_null() {
        unsafe { touch_dxt5_base_level(destination, 4096, 4096, 0) }
    } else {
        -1
    };

    let staged = unsafe { process_memory_private_bytes() };
    let update_hr =
        if source_hr >= 0 && destination_hr >= 0 && !source.is_null() && !destination.is_null() {
            unsafe { update(device, source, destination) }
        } else {
            -1
        };

    if !source.is_null() {
        unsafe { release_texture(source) };
    }
    let destination_only = unsafe { process_memory_private_bytes() };

    log_line(&format!(
        "UPLOAD_PROBE source_hr=0x{:08X} source_lock_hr=0x{:08X} destination_hr=0x{:08X} destination_lock_hr=0x{:08X} update_hr=0x{:08X} baseline_private={} staged_private={} destination_only_private={} destination_only_delta={}",
        source_hr as u32,
        source_lock_hr as u32,
        destination_hr as u32,
        destination_lock_hr as u32,
        update_hr as u32,
        baseline.unwrap_or(0),
        staged.unwrap_or(0),
        destination_only.unwrap_or(0),
        match (baseline, destination_only) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));

    if !destination.is_null() {
        unsafe { release_texture(destination) };
    }
    let released = unsafe { process_memory_private_bytes() };
    log_line(&format!(
        "UPLOAD_PROBE_RELEASED private={} baseline_delta={}",
        released.unwrap_or(0),
        match (baseline, released) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));

    let dynamic_before = unsafe { process_memory_private_bytes() };
    let mut dynamic = ptr::null_mut();
    let dynamic_hr = unsafe {
        create(
            device,
            4096,
            4096,
            1,
            D3DUSAGE_DYNAMIC,
            D3DFMT_DXT5,
            0,
            &mut dynamic,
            ptr::null_mut(),
        )
    };
    let dynamic_lock_hr = if dynamic_hr >= 0 && !dynamic.is_null() {
        unsafe { touch_dxt5_base_level(dynamic, 4096, 4096, D3DLOCK_DISCARD) }
    } else {
        -1
    };
    let dynamic_after = unsafe { process_memory_private_bytes() };

    log_line(&format!(
        "DYNAMIC_DEFAULT_PROBE create_hr=0x{:08X} lock_discard_hr=0x{:08X} private_before={} private_after={} private_delta={}",
        dynamic_hr as u32,
        dynamic_lock_hr as u32,
        dynamic_before.unwrap_or(0),
        dynamic_after.unwrap_or(0),
        match (dynamic_before, dynamic_after) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));

    if !dynamic.is_null() {
        unsafe { release_texture(dynamic) };
    }

    let retained_baseline = unsafe { process_memory_private_bytes() };
    let mut retained_destinations = Vec::new();
    for index in 0..4 {
        let mut staging = ptr::null_mut();
        let staging_hr = unsafe {
            create(
                device,
                4096,
                4096,
                1,
                0,
                D3DFMT_DXT5,
                2,
                &mut staging,
                ptr::null_mut(),
            )
        };
        let staging_lock_hr = if staging_hr >= 0 && !staging.is_null() {
            unsafe { touch_dxt5_base_level(staging, 4096, 4096, 0) }
        } else {
            -1
        };

        let mut retained = ptr::null_mut();
        let retained_hr = unsafe {
            create(
                device,
                4096,
                4096,
                1,
                0,
                D3DFMT_DXT5,
                0,
                &mut retained,
                ptr::null_mut(),
            )
        };
        let retained_update_hr =
            if staging_hr >= 0 && retained_hr >= 0 && !staging.is_null() && !retained.is_null() {
                unsafe { update(device, staging, retained) }
            } else {
                -1
            };

        if !staging.is_null() {
            unsafe { release_texture(staging) };
        }

        if retained_hr >= 0 && !retained.is_null() {
            retained_destinations.push(retained);
        }

        let current = unsafe { process_memory_private_bytes() };
        log_line(&format!(
            "STATIC_DEFAULT_RETAIN index={} staging_hr=0x{:08X} staging_lock_hr=0x{:08X} destination_hr=0x{:08X} update_hr=0x{:08X} retained_count={} private={} baseline_delta={}",
            index,
            staging_hr as u32,
            staging_lock_hr as u32,
            retained_hr as u32,
            retained_update_hr as u32,
            retained_destinations.len(),
            current.unwrap_or(0),
            match (retained_baseline, current) {
                (Some(before), Some(after)) => after as i64 - before as i64,
                _ => 0,
            }
        ));
    }

    for texture in retained_destinations {
        unsafe { release_texture(texture) };
    }
    let retained_released = unsafe { process_memory_private_bytes() };
    log_line(&format!(
        "STATIC_DEFAULT_RETAIN_RELEASED private={} baseline_delta={}",
        retained_released.unwrap_or(0),
        match (retained_baseline, retained_released) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));
}

unsafe fn process_memory_private_bytes() -> Option<usize> {
    let mut counters = ProcessMemoryCountersEx {
        cb: size_of::<ProcessMemoryCountersEx>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
        private_usage: 0,
    };
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            size_of::<ProcessMemoryCountersEx>() as u32,
        )
    };
    (ok != 0).then_some(counters.private_usage)
}

unsafe fn texture_vtable_method(texture: *mut c_void, index: usize) -> usize {
    if texture.is_null() {
        return 0;
    }
    let vtable = unsafe { *(texture as *mut *mut usize) };
    if vtable.is_null() {
        return 0;
    }
    unsafe { *vtable.add(index) }
}

unsafe fn touch_dxt5_base_level(
    texture: *mut c_void,
    width: u32,
    height: u32,
    flags: u32,
) -> HResult {
    let lock_address = unsafe { texture_vtable_method(texture, TEXTURE_LOCK_RECT_INDEX) };
    let unlock_address = unsafe { texture_vtable_method(texture, TEXTURE_UNLOCK_RECT_INDEX) };
    if lock_address == 0 || unlock_address == 0 {
        return -1;
    }

    let lock: LockRectFn = unsafe { transmute(lock_address) };
    let unlock: UnlockRectFn = unsafe { transmute(unlock_address) };
    let mut locked = LockedRect {
        pitch: 0,
        bits: ptr::null_mut(),
    };
    let hr = unsafe { lock(texture, 0, &mut locked, ptr::null(), flags) };
    if hr < 0 || locked.bits.is_null() || locked.pitch <= 0 {
        return hr;
    }

    let row_bytes = width.div_ceil(4) as usize * 16;
    let block_rows = height.div_ceil(4) as usize;
    for row in 0..block_rows {
        let destination = unsafe { (locked.bits as *mut u8).add(row * locked.pitch as usize) };
        unsafe { ptr::write_bytes(destination, 0x5A, row_bytes) };
    }

    unsafe { unlock(texture, 0) }
}

unsafe fn release_texture(texture: *mut c_void) {
    let release_address = unsafe { texture_vtable_method(texture, TEXTURE_RELEASE_INDEX) };
    if release_address == 0 {
        return;
    }
    let release: ReleaseFn = unsafe { transmute(release_address) };
    let _ = unsafe { release(texture) };
}

unsafe fn run_allocation_probe(device: *mut c_void) {
    let create_address = ORIGINAL_CREATE_TEXTURE.load(Ordering::SeqCst);
    if create_address == 0 {
        log_line("ALLOC_PROBE unavailable: CreateTexture original missing");
        return;
    }

    let create: CreateTextureFn = unsafe { transmute(create_address) };
    let baseline = unsafe { process_memory_private_bytes() };
    log_line(&format!(
        "ALLOC_PROBE_START private_bytes={}",
        baseline.unwrap_or(0)
    ));

    let mut textures: Vec<*mut c_void> = Vec::new();
    for size in [1024u32, 2048, 4096, 8192] {
        let mut texture = ptr::null_mut();
        let before = unsafe { process_memory_private_bytes() };
        let hr = unsafe {
            create(
                device,
                size,
                size,
                0,
                0,
                D3DFMT_DXT5,
                1,
                &mut texture,
                ptr::null_mut(),
            )
        };

        let touch_hr = if hr >= 0 && !texture.is_null() {
            unsafe { touch_dxt5_base_level(texture, size, size, 0) }
        } else {
            -1
        };
        let after = unsafe { process_memory_private_bytes() };
        let delta = match (before, after) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        };

        log_line(&format!(
            "ALLOC_PROBE_MANAGED size={} levels=full format=DXT5 create_hr=0x{:08X} touch_hr=0x{:08X} private_delta={} private_after={}",
            size,
            hr as u32,
            touch_hr as u32,
            delta,
            after.unwrap_or(0)
        ));

        if hr >= 0 && !texture.is_null() {
            textures.push(texture);
        }
    }

    let retained = unsafe { process_memory_private_bytes() };
    log_line(&format!(
        "ALLOC_PROBE_RETAINED count={} private_bytes={} baseline_delta={}",
        textures.len(),
        retained.unwrap_or(0),
        match (baseline, retained) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));

    for texture in textures {
        unsafe { release_texture(texture) };
    }

    let released = unsafe { process_memory_private_bytes() };
    log_line(&format!(
        "ALLOC_PROBE_RELEASED private_bytes={} baseline_delta={}",
        released.unwrap_or(0),
        match (baseline, released) {
            (Some(before), Some(after)) => after as i64 - before as i64,
            _ => 0,
        }
    ));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Direct3DCreate9(sdk_version: u32) -> *mut c_void {
    let address =
        *REAL_DIRECT3D_CREATE9.get_or_init(|| unsafe { real_symbol(b"Direct3DCreate9\0") });
    if address == 0 {
        return ptr::null_mut();
    }

    let real: Direct3DCreate9Fn = unsafe { transmute(address) };
    let object = unsafe { real(sdk_version) };

    log_line(&format!(
        "PROBE_START pid={} sdk={} object={object:p}",
        unsafe { GetCurrentProcessId() },
        sdk_version
    ));

    unsafe {
        patch_vtable_entry(
            object,
            D3D9_CREATE_DEVICE_INDEX,
            hook_create_device as usize,
            &ORIGINAL_CREATE_DEVICE,
        );
    }

    object
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3DPERF_SetOptions(options: u32) {
    let address =
        *REAL_D3DPERF_SET_OPTIONS.get_or_init(|| unsafe { real_symbol(b"D3DPERF_SetOptions\0") });
    if address == 0 {
        return;
    }

    let real: D3dPerfSetOptionsFn = unsafe { transmute(address) };
    unsafe { real(options) };
}
