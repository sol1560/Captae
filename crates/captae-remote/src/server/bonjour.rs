//! Bonjour 广播 `_captae._tcp`：交给系统的 mDNSResponder（`DNSServiceRegister`，macOS 的 libSystem 里就有，不用加依赖）。
//!
//! 不自己收发组播（原来用 mdns-sd）：macOS 15 起，应用自己发局域网组播会弹「本地网络」权限的询问；
//! 交给 mDNSResponder 广播，Captae 自己不碰组播。Info.plist 里照样写了 `NSLocalNetworkUsageDescription` 和 `NSBonjourServices`，
//! 万一系统还是问，说明也是清楚的（apps/desktop/src-tauri/Info.plist）。
//!
//! TXT 记录：`v=<协议版本>`、`fp=<证书指纹>`、`name=<电脑名>`。手机按 fp 认出是配对过的哪一台。
//! 其他系统（Captae 现在只发 macOS 版）不广播，扫码照样能连。

use anyhow::Result;

/// 一次注册。丢掉它就撤回广播（mDNSResponder 会发告别包，手机那边马上知道电脑走了）
pub struct Advert {
    /// 不读它，只为了丢掉时撤回广播（Drop）
    #[cfg(target_os = "macos")]
    _sd: mac::Ref,
}

/// TXT 记录：每项一个长度字节加 `键=值`，每项最长 255 字节（太长的截掉，按字符边界）
pub(crate) fn txt_record(items: &[(&str, &str)]) -> Vec<u8> {
    let mut out = vec![];
    for (k, v) in items {
        let mut s = format!("{k}={v}");
        while s.len() > 255 {
            s.pop();
        }
        out.push(s.len() as u8);
        out.extend_from_slice(s.as_bytes());
    }
    out
}

/// 实例名：电脑名，最长 63 字节（按字符边界截）；空的用「Captae」
pub(crate) fn instance_name(name: &str) -> String {
    let mut s: String = name.chars().filter(|c| !c.is_control()).collect();
    while s.len() > 63 {
        s.pop();
    }
    if s.trim().is_empty() {
        "Captae".into()
    } else {
        s
    }
}

impl Advert {
    pub fn register(name: &str, port: u16, fp: &str) -> Result<Advert> {
        let v = crate::proto::VERSION.to_string();
        let txt = txt_record(&[("v", &v), ("fp", fp), ("name", name)]);
        let inst = instance_name(name);
        #[cfg(target_os = "macos")]
        {
            Ok(Advert { _sd: mac::register(&inst, port, &txt)? })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (inst, port, txt);
            tracing::info!("这个系统上不广播 Bonjour（只有 macOS 交给系统广播），手机扫码照样能连");
            Ok(Advert {})
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use anyhow::{anyhow, Result};
    use std::ffi::{c_char, c_void, CString};
    use std::ptr;

    #[repr(C)]
    struct Opaque {
        _p: [u8; 0],
    }
    type DNSServiceRef = *mut Opaque;
    type Callback = Option<unsafe extern "C" fn(DNSServiceRef, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void)>;

    // dns_sd.h（libSystem 自带）
    unsafe extern "C" {
        fn DNSServiceRegister(
            sd_ref: *mut DNSServiceRef,
            flags: u32,
            interface_index: u32,
            name: *const c_char,
            regtype: *const c_char,
            domain: *const c_char,
            host: *const c_char,
            port: u16,
            txt_len: u16,
            txt_record: *const c_void,
            callback: Callback,
            context: *mut c_void,
        ) -> i32;
        fn DNSServiceRefDeallocate(sd_ref: DNSServiceRef);
    }

    /// 注册拿到的句柄。只在注册和撤回时用到，中间不读写，所以可以跨线程交出去
    pub struct Ref(DNSServiceRef);
    unsafe impl Send for Ref {}
    unsafe impl Sync for Ref {}

    impl Drop for Ref {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // 撤回广播，断开和 mDNSResponder 的连接
                unsafe { DNSServiceRefDeallocate(self.0) };
            }
        }
    }

    pub fn register(name: &str, port: u16, txt: &[u8]) -> Result<Ref> {
        let name = CString::new(name).map_err(|_| anyhow!("电脑名里有空字符"))?;
        let regtype = CString::new("_captae._tcp").unwrap();
        let mut sd: DNSServiceRef = ptr::null_mut();
        // 回调传空：不要注册结果（名字冲突时系统自己改名，不用我们处理），也就不用跑 DNSServiceProcessResult 的循环。
        // 端口要网络字节序
        let err = unsafe { DNSServiceRegister(&mut sd, 0, 0, name.as_ptr(), regtype.as_ptr(), ptr::null(), ptr::null(), port.to_be(), txt.len() as u16, txt.as_ptr() as *const c_void, None, ptr::null_mut()) };
        if err != 0 || sd.is_null() {
            return Err(anyhow!("DNSServiceRegister 出错：{err}"));
        }
        Ok(Ref(sd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_and_name() {
        assert_eq!(txt_record(&[("v", "1"), ("fp", "ab")]), b"\x03v=1\x05fp=ab".to_vec());
        let long = "长".repeat(100);
        let t = txt_record(&[("name", &long)]);
        assert!(t[0] as usize == t.len() - 1 && t.len() - 1 <= 255);
        assert!(std::str::from_utf8(&t[1..]).is_ok(), "按字符边界截");
        assert_eq!(instance_name("  "), "Captae");
        assert!(instance_name(&"字".repeat(40)).len() <= 63);
        assert_eq!(instance_name("Sol 的 MacBook Pro"), "Sol 的 MacBook Pro");
    }

    /// 真的交给 mDNSResponder 注册几秒（会在局域网上广播，所以默认不跑）：
    /// `cargo test -p captae-remote bonjour_registers -- --ignored`，同时在另一个终端 `dns-sd -L "Captae 自测" _captae._tcp` 看 TXT
    #[test]
    #[ignore]
    fn bonjour_registers() {
        let a = Advert::register("Captae 自测", 47999, "dGVzdA").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(4));
        drop(a);
    }
}
