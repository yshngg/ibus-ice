use std::ffi::CString;
use std::os::raw::c_char;

#[repr(C)]
pub struct IceDebugResult {
    pub json: *mut c_char,
}

impl IceDebugResult {
    pub fn from_json(json: String) -> Box<Self> {
        Box::new(IceDebugResult {
            json: CString::new(json).unwrap().into_raw(),
        })
    }
}

impl Drop for IceDebugResult {
    fn drop(&mut self) {
        unsafe {
            if !self.json.is_null() {
                drop(CString::from_raw(self.json));
            }
        }
    }
}
