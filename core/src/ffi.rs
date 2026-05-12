use crate::debug_result::IceDebugResult;
use crate::engine::IceEngine;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub struct IceEngineHandle {
    engine: IceEngine,
}

#[repr(C)]
pub struct IceCandidate {
    pub text: *mut c_char,
    pub freq: i32,
    pub word_len: i32,
}

#[repr(C)]
pub struct IceCandidateList {
    pub candidates: *mut IceCandidate,
    pub count: i32,
}

#[no_mangle]
pub extern "C" fn ice_engine_new(
    dict_path: *const c_char,
    user_dict_path: *const c_char,
) -> *mut IceEngineHandle {
    if dict_path.is_null() || user_dict_path.is_null() {
        return std::ptr::null_mut();
    }

    let dict_path = unsafe { CStr::from_ptr(dict_path) }.to_string_lossy().into_owned();
    let user_dict_path = unsafe { CStr::from_ptr(user_dict_path) }.to_string_lossy().into_owned();

    match IceEngine::new(&dict_path, &user_dict_path) {
        Ok(engine) => Box::into_raw(Box::new(IceEngineHandle { engine })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn ice_engine_free(handle: *mut IceEngineHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

#[no_mangle]
pub extern "C" fn ice_process(
    handle: *mut IceEngineHandle,
    pinyin: *const c_char,
) -> *mut IceCandidateList {
    if handle.is_null() || pinyin.is_null() {
        return std::ptr::null_mut();
    }

    let engine = unsafe { &mut (*handle).engine };
    let pinyin = unsafe { CStr::from_ptr(pinyin) }.to_string_lossy();

    let candidates = engine.process(&pinyin);

    if candidates.is_empty() {
        return std::ptr::null_mut();
    }

    let count = candidates.len() as i32;
    let mut ffi_candidates: Vec<IceCandidate> = candidates
        .into_iter()
        .map(|c| {
            let text = CString::new(c.text).unwrap().into_raw();
            IceCandidate {
                text,
                freq: c.freq as i32,
                word_len: c.word_len as i32,
            }
        })
        .collect();

    let ptr = ffi_candidates.as_mut_ptr();
    std::mem::forget(ffi_candidates);

    Box::into_raw(Box::new(IceCandidateList {
        candidates: ptr,
        count,
    }))
}

#[no_mangle]
pub extern "C" fn ice_select(handle: *mut IceEngineHandle, text: *const c_char) {
    if handle.is_null() || text.is_null() {
        return;
    }
    let engine = unsafe { &mut (*handle).engine };
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    engine.select(&text);
}

#[no_mangle]
pub extern "C" fn ice_candidates_free(list: *mut IceCandidateList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let slice = unsafe { std::slice::from_raw_parts_mut(list.candidates, list.count as usize) };
    for c in slice.iter_mut() {
        if !c.text.is_null() {
            unsafe { drop(CString::from_raw(c.text)) };
        }
    }
    unsafe {
        drop(Vec::from_raw_parts(
            list.candidates,
            list.count as usize,
            list.count as usize,
        ))
    };
}

#[no_mangle]
pub extern "C" fn ice_reset(handle: *mut IceEngineHandle) {
    if handle.is_null() {
        return;
    }
    let engine = unsafe { &mut (*handle).engine };
    engine.reset();
}

#[no_mangle]
pub extern "C" fn ice_debug_process(
    handle: *mut IceEngineHandle,
    pinyin: *const c_char,
) -> *mut IceDebugResult {
    if handle.is_null() || pinyin.is_null() {
        return std::ptr::null_mut();
    }
    let engine = unsafe { &(*handle).engine };
    let pinyin = unsafe { CStr::from_ptr(pinyin) }.to_string_lossy();
    let json = engine.debug_process(&pinyin);
    Box::into_raw(IceDebugResult::from_json(json))
}

#[no_mangle]
pub extern "C" fn ice_debug_result_free(result: *mut IceDebugResult) {
    if !result.is_null() {
        unsafe { drop(Box::from_raw(result)) };
    }
}
