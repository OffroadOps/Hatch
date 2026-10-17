use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{path::Path, process::Child};

#[cfg(windows)]
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}

pub fn protect(s: &str) -> Result<String> {
    if s.is_empty() || s.starts_with("DPAPI:") {
        return Ok(s.into());
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(s.len())?,
            pbData: s.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // DPAPI owns the output allocation. Every success path frees it with LocalFree.
        unsafe {
            if CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            ) == 0
            {
                return Err(std::io::Error::last_os_error()).context("DPAPI 加密失败");
            }
            let encoded = STANDARD.encode(std::slice::from_raw_parts(
                output.pbData,
                output.cbData as usize,
            ));
            LocalFree(output.pbData as _);
            Ok(format!("DPAPI:{encoded}"))
        }
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("凭据加密要求 Windows DPAPI");
    }
}

pub fn unprotect(s: &str) -> Result<String> {
    let Some(encoded) = s.strip_prefix("DPAPI:") else {
        return Ok(s.into());
    };
    let mut bytes = STANDARD.decode(encoded).context("DPAPI 数据格式错误")?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(bytes.len())?,
            pbData: bytes.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        unsafe {
            if CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            ) == 0
            {
                return Err(std::io::Error::last_os_error())
                    .context("无法解密凭据，请使用原 Windows 账户导入");
            }
            let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            String::from_utf8(plain).context("凭据编码无效")
        }
    }
    #[cfg(not(windows))]
    {
        let _ = &mut bytes;
        anyhow::bail!("DPAPI 凭据只能在 Windows 原账户解密");
    }
}

pub fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::*;
        unsafe {
            if MoveFileExW(
                wide(source.as_os_str()).as_ptr(),
                wide(destination.as_os_str()).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            ) == 0
            {
                return Err(std::io::Error::last_os_error()).context("替换配置失败");
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(source, destination).context("替换配置失败")
    }
}

pub fn hidden(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

#[cfg(windows)]
pub struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
impl Job {
    pub fn new() -> Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = Self(CreateJobObjectW(std::ptr::null(), std::ptr::null()));
            if job.0.is_null() {
                return Err(std::io::Error::last_os_error()).context("创建核心进程 Job 失败");
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error()).context("配置核心进程 Job 失败");
            }
            Ok(job)
        }
    }
    pub fn attach(&self, child: &Child) -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            if windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0,
                child.as_raw_handle() as _,
            ) == 0
            {
                return Err(std::io::Error::last_os_error()).context("绑定核心进程 Job 失败");
            }
        }
        Ok(())
    }
    pub fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
pub struct Job;
#[cfg(not(windows))]
impl Job {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
    pub fn attach(&self, _: &Child) -> Result<()> {
        Ok(())
    }
    pub fn terminate(&self) {}
}

#[cfg(windows)]
pub struct SingleInstance(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl SingleInstance {
    pub fn acquire() -> Result<Self> {
        unsafe {
            use windows_sys::Win32::{Foundation::*, System::Threading::CreateMutexW};
            let handle = CreateMutexW(
                std::ptr::null(),
                0,
                wide(std::ffi::OsStr::new("Local\\NPGA-Rust-Desktop-v3")).as_ptr(),
            );
            if handle.is_null() {
                return Err(std::io::Error::last_os_error()).context("创建实例锁失败");
            }
            let lock = Self(handle);
            if GetLastError() == ERROR_ALREADY_EXISTS {
                anyhow::bail!("NPGA Rust 已在运行，请从托盘打开");
            }
            Ok(lock)
        }
    }
}
#[cfg(windows)]
impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
pub struct SystemProxy {
    previous: Vec<(String, Option<winreg::RegValue>)>,
    key: winreg::RegKey,
    journal: std::path::PathBuf,
}
#[cfg(windows)]
fn notify_proxy() {
    unsafe {
        use windows_sys::Win32::Networking::WinInet::*;
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
}
#[cfg(windows)]
impl SystemProxy {
    pub fn enable(address: &str, port: u16, journal: &Path) -> Result<Self> {
        use winreg::{RegKey, enums::*};
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_READ | KEY_WRITE,
        )?;
        let previous = [
            "ProxyEnable",
            "ProxyServer",
            "ProxyOverride",
            "AutoConfigURL",
        ]
        .into_iter()
        .map(|k| (k.into(), key.get_raw_value(k).ok()))
        .collect();
        let guard = Self {
            previous,
            key,
            journal: journal.into(),
        };
        let snapshot: ProxySnapshot = guard
            .previous
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    value
                        .as_ref()
                        .map(|v| (v.vtype.clone() as u32, v.bytes.clone())),
                )
            })
            .collect();
        if let Some(parent) = journal.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = journal.with_extension("tmp");
        std::fs::write(&temp, protect(&serde_json::to_string(&snapshot)?)?)?;
        atomic_replace(&temp, journal)?;
        guard.key.set_value(
            "ProxyServer",
            &format!("http={address}:{port};https={address}:{port}"),
        )?;
        guard
            .key
            .set_value("ProxyOverride", &"<local>;localhost;127.*;10.*;192.168.*")?;
        match guard.key.delete_value("AutoConfigURL") {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        guard.key.set_value("ProxyEnable", &1u32)?;
        notify_proxy();
        Ok(guard)
    }
    pub fn restore(&self) -> Result<()> {
        for (name, value) in &self.previous {
            if let Some(value) = value {
                self.key.set_raw_value(name, value)?;
            } else if let Err(e) = self.key.delete_value(name)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(e.into());
            }
        }
        notify_proxy();
        if self.journal.exists() {
            std::fs::remove_file(&self.journal)?;
        }
        Ok(())
    }
}
#[cfg(windows)]
impl Drop for SystemProxy {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
#[cfg(not(windows))]
pub struct SystemProxy;
#[cfg(not(windows))]
impl SystemProxy {
    pub fn enable(_: &str, _: u16, _: &Path) -> Result<Self> {
        anyhow::bail!("系统代理只支持 Windows")
    }
}

#[cfg(windows)]
type ProxySnapshot = Vec<(String, Option<(u32, Vec<u8>)>)>;

/// After an abnormal exit, recover the saved Windows proxy settings before starting another session.
pub fn recover_proxy(journal: &Path) -> Result<bool> {
    if !journal.exists() {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        use winreg::{RegKey, RegValue, enums::*};
        let snapshot: ProxySnapshot =
            serde_json::from_str(&unprotect(&std::fs::read_to_string(journal)?)?)?;
        let mut previous = vec![];
        for (name, value) in snapshot {
            if ![
                "ProxyEnable",
                "ProxyServer",
                "ProxyOverride",
                "AutoConfigURL",
            ]
            .contains(&name.as_str())
            {
                anyhow::bail!("代理恢复记录包含未知字段");
            }
            let value = value
                .map(|(kind, bytes)| -> Result<RegValue> {
                    let vtype = match kind {
                        0 => REG_NONE,
                        1 => REG_SZ,
                        2 => REG_EXPAND_SZ,
                        3 => REG_BINARY,
                        4 => REG_DWORD,
                        5 => REG_DWORD_BIG_ENDIAN,
                        6 => REG_LINK,
                        7 => REG_MULTI_SZ,
                        8 => REG_RESOURCE_LIST,
                        9 => REG_FULL_RESOURCE_DESCRIPTOR,
                        10 => REG_RESOURCE_REQUIREMENTS_LIST,
                        11 => REG_QWORD,
                        _ => anyhow::bail!("代理恢复记录类型无效"),
                    };
                    Ok(RegValue { bytes, vtype })
                })
                .transpose()?;
            previous.push((name, value));
        }
        if previous.len() != 4 {
            anyhow::bail!("代理恢复记录不完整");
        }
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_READ | KEY_WRITE,
        )?;
        let guard = SystemProxy {
            previous,
            key,
            journal: journal.into(),
        };
        guard.restore().context("恢复上次系统代理失败")?;
        Ok(true)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("代理恢复记录仅能在 Windows 使用");
    }
}

pub fn wait_for_parent(pid: u32) -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        let handle = OpenProcess(0x100000, 0, pid);
        if handle.is_null() {
            if GetLastError() == ERROR_INVALID_PARAMETER {
                return Ok(());
            }
            return Err(std::io::Error::last_os_error()).context("等待旧实例退出失败");
        }
        let status = WaitForSingleObject(handle, 15000);
        CloseHandle(handle);
        if status != WAIT_OBJECT_0 {
            anyhow::bail!("旧实例退出超时，请手动从托盘退出后重启");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(())
    }
}

pub fn startup(enable: bool, data_dir: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )?;
        if enable {
            let executable = std::env::current_exe()?;
            key.set_value(
                "NPGA-Rust",
                &format!(
                    "\"{}\" --data-dir \"{}\"",
                    executable.display(),
                    data_dir.display()
                ),
            )?;
        } else if let Err(e) = key.delete_value("NPGA-Rust")
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e.into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (enable, data_dir);
        anyhow::bail!("开机启动只支持 Windows")
    }
}

pub fn elevate(data_dir: &Path, runtime_root: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()?;
        let args = format!(
            "--data-dir \"{}\" --runtime-root \"{}\" --wait-for-parent {}",
            data_dir.display(),
            runtime_root.display(),
            std::process::id()
        );
        unsafe {
            let result = windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                wide(std::ffi::OsStr::new("runas")).as_ptr(),
                wide(executable.as_os_str()).as_ptr(),
                wide(std::ffi::OsStr::new(&args)).as_ptr(),
                std::ptr::null(),
                1,
            );
            if result as usize <= 32 {
                anyhow::bail!("管理员重启被取消或失败");
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (data_dir, runtime_root);
        anyhow::bail!("仅支持 Windows")
    }
}
