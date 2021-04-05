use bindings::Windows::Win32::SystemServices::*;
use widestring::U16CString;
use windows::Result;

pub fn pwstr_from_str(string: &str) -> PWSTR {
    PWSTR(U16CString::from_str(string).unwrap().into_raw())
}

pub fn unwrap_hresult<T>(result: Result<T>) -> T {
    result.unwrap_or_else(|err| panic!("Program crashed due to winapi error: {}", err.message()))
}