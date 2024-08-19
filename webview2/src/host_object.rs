use crate::WebView;
use std::mem::ManuallyDrop;
use std::sync::mpsc;
use windows::core::*;
pub use windows::Win32::System::Com::IDispatch;

use windows::Win32::System::{
    Com::{IDispatch_Impl, ITypeInfo, DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO},
    Variant::{
        VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_DISPATCH, VT_I4,
    },
};

// This is a simple usage example. add_host_object_to_script is a mapping of the native [addHostObjectToScript](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2#addhostobjecttoscript) method of webview2. It requires manual creation of hostobject and memory management. Please use it with caution.
pub struct Variant(pub VARIANT);
impl Variant {
    pub fn new(num: VARENUM, contents: VARIANT_0_0_0) -> Variant {
        Variant {
            0: VARIANT {
                Anonymous: VARIANT_0 {
                    Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                        vt: num,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: contents,
                    }),
                },
            },
        }
    }
}

impl From<String> for Variant {
    fn from(value: String) -> Variant {
        Variant::new(
            VT_BSTR,
            VARIANT_0_0_0 {
                bstrVal: ManuallyDrop::new(BSTR::from(value)),
            },
        )
    }
}

impl From<&str> for Variant {
    fn from(value: &str) -> Variant {
        Variant::from(value.to_string())
    }
}

impl From<i32> for Variant {
    fn from(value: i32) -> Variant {
        Variant::new(VT_I4, VARIANT_0_0_0 { lVal: value })
    }
}

impl From<ManuallyDrop<Option<IDispatch>>> for Variant {
    fn from(value: ManuallyDrop<Option<IDispatch>>) -> Variant {
        Variant::new(VT_DISPATCH, VARIANT_0_0_0 { pdispVal: value })
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        match VARENUM(unsafe { self.0.Anonymous.Anonymous.vt.0 }) {
            VT_BSTR => unsafe { drop(&self.0.Anonymous.Anonymous.Anonymous.bstrVal) },
            _ => {}
        }
        unsafe { drop(&self.0.Anonymous.Anonymous) }
    }
}

#[implement(IDispatch)]
pub struct FunctionWithStringArgument {
    pub sender: std::sync::mpsc::Sender<String>,
}

impl IDispatch_Impl for FunctionWithStringArgument {
    #![allow(non_snake_case)]
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> {
        Ok(0)
    }

    fn GetTypeInfo(&self, _itinfo: u32, _lcid: u32) -> windows::core::Result<ITypeInfo> {
        Err(windows::core::Error::new(
            windows::Win32::Foundation::E_FAIL,
            "GetTypeInfo Error \t\n\r".into(),
        ))
    }

    fn GetIDsOfNames(
        &self,
        _riid: *const GUID,
        _rgsznames: *const PCWSTR,
        _cnames: u32,
        _lcid: u32,
        _rgdispid: *mut i32,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn Invoke(
        &self,
        _dispidmember: i32,
        _riid: *const GUID,
        _lcid: u32,
        _wflags: DISPATCH_FLAGS,
        pdispparams: *const DISPPARAMS,
        pvarresult: *mut VARIANT,
        _pexcepinfo: *mut EXCEPINFO,
        _puargerr: *mut u32,
    ) -> windows::core::Result<()> {
        let pdispparams = unsafe { *pdispparams };
        let rgvarg = unsafe { &*(pdispparams.rgvarg) };
        let rgvarg_0_0 = unsafe { &rgvarg.Anonymous.Anonymous };
        unsafe {
            dbg!(&rgvarg_0_0.Anonymous.bstrVal);
        }
        let b_str_val = unsafe { rgvarg_0_0.Anonymous.bstrVal.to_string() };

        let pvarresult_0_0 = unsafe { &mut (*pvarresult).Anonymous.Anonymous };
        pvarresult_0_0.vt = VT_BSTR;
        pvarresult_0_0.Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(
            format!(
                r#"Successful sync call functionWithStringArgument, and the argument is "{}"."#,
                b_str_val
            )
            .to_string(),
        ));

        self.sender.send(b_str_val).unwrap();

        Ok(())
    }
}

pub fn check_loaded(name: &str) -> String {
    // format!("(function () {{ try {{ typeof window.chrome.webview.hostObjects.sync.{}; return 'loaded'; }} catch (e) {{ if(e.message.indexOf('Element out found') === 0) {{ return 'not_loaded'; }} else {{ return 'error' }} }} }})()", name)
    format!("(function () {{ try {{ typeof window.chrome.webview.hostObjects.sync.{}; return 'loaded'; }} catch (e) {{ return 'not_loaded'; }} }})()", name)
}

pub fn ensure_bind<F>(w: WebView, name: String, mut obj: Box<Variant>, cb: F)
where
    F: FnOnce(WebView) + 'static,
{
    eprintln!("bind");
    w.add_host_object_to_script(&name, &mut obj.0)
        .expect("add_host_object_to_script");

    check_bind(w.clone(), name.clone(), obj, cb);
}

pub fn check_bind<F>(w: WebView, name: String, obj: Box<Variant>, cb: F)
where
    F: FnOnce(WebView) + 'static,
{
    eprintln!("check_bind");
    let script = check_loaded(&name);

    let w0 = w.clone();
    w.execute_script(&script, move |s| {
        println!("s={:?}", s);
        if s == "\"loaded\"" {
            cb(w0);
        } else if s == "\"not_loaded\"" {
            ensure_bind(w0, name.clone(), obj, cb);
        } else {
            todo!();
        }
        Ok(())
    })
    .expect("execute_script");

    eprintln!("check_bind end");
}
