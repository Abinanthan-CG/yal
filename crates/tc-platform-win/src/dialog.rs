//! Native Windows 11 file picker (`IFileOpenDialog`) for importing backups and
//! Raycast snippet JSON exports.

use std::path::PathBuf;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
};

/// Show a native Windows Open File dialog filtered to `filter_spec` (e.g. `"*.json"`).
///
/// Returns `None` if the user cancels or the dialog cannot be shown.
pub fn pick_file(title: &str, filter_name: &str, filter_spec: &str) -> Option<PathBuf> {
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = filter_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let spec_w: Vec<u16> = filter_spec
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let chosen = unsafe {
        (|| -> Option<PathBuf> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            let _ = dialog.SetTitle(PCWSTR(title_w.as_ptr()));
            let filters = [COMDLG_FILTERSPEC {
                pszName: PCWSTR(name_w.as_ptr()),
                pszSpec: PCWSTR(spec_w.as_ptr()),
            }];
            let _ = dialog.SetFileTypes(&filters);
            if let Ok(opts) = dialog.GetOptions() {
                let _ = dialog.SetOptions(opts | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST);
            }
            dialog.Show(None).ok()?;
            let item = dialog.GetResult().ok()?;
            let path_pwstr: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path = path_pwstr.to_string().ok().map(PathBuf::from);
            CoTaskMemFree(Some(path_pwstr.0 as *const std::ffi::c_void));
            path
        })()
    };
    if com_ok {
        unsafe { CoUninitialize() };
    }
    chosen
}
