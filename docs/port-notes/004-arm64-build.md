# Port note 004 — Building for ARM64 on Windows

**Date.** 2026-11-10

---

## The problem

The port has two targets:

```text
x86_64-pc-windows-msvc
aarch64-pc-windows-msvc
```

The x64 build works out of the box on a machine with the Visual Studio Build
Tools. The ARM64 build does not, and the error says nothing useful:

```text
error: linker `link.exe` not found
```

---

## Why it fails

Cross-compiling to ARM64 needs the **ARM64 variant** of the Visual Studio C++
toolchain, which is a separate component:

```text
Microsoft.VisualStudio.Component.VC.Tools.ARM64
```

It is not installed by default, even when the x64 tools are. What it adds is:

```text
VC\Tools\MSVC\<version>\bin\Hostx64\arm64\   link.exe, lib.exe, ...
VC\Tools\MSVC\<version>\lib\arm64\           msvcrt.lib, libcmt.lib, vcruntime.lib, ...
```

The **Windows SDK** already carries the ARM64 import libraries for the system
DLLs — `kernel32.lib`, `user32.lib` and the rest are in
`Windows Kits\10\Lib\<version>\um\arm64` — so the SDK is not what is
missing. Only the Visual Studio ARM64 component is.

`msvcrt.lib` is the one library that is easy to overlook, because Rust's MSVC
targets link the C runtime dynamically through `/defaultlib:msvcrt`. Without
it the link fails with:

```text
rust-lld: error: could not open 'msvcrt.lib': no such file or directory
```

and, once it is supplied, with:

```text
rust-lld: error: undefined symbol: mainCRTStartup
```

`mainCRTStartup` is not in `msvcrt.lib` at all — it comes from the Visual
Studio startup object, which is also ARM64-only.

## What does not work

**Synthesising the import library.** An ARM64 `msvcrt.lib` can be built from
the DLL's export table with `lib.exe /def:... /machine:arm64`, and the
Windows SDK's ARM64 libraries are already present. But the link still fails
on `mainCRTStartup`, and there is no honest way to synthesise the Visual
Studio startup object.

**Rust's UCRT target.** `aarch64-pc-windows-gnullvm` is a tier-3 target that
links the Universal CRT instead of `msvcrt`, and its standard library is
distributed by rustup. But it expects LLVM's gnullvm runtime libraries —
`unwind`, `mingw32`, `mingwex` — which are not shipped by rustup and which
this machine does not have:

```text
lld: error: unable to find library -lunwind
lld: error: unable to find library -lmingw32
lld: error: unable to find library -lmingwex
lld: error: unable to find library -lmsvcrt
```

**Linking with `rust-lld` and the SDK's libraries.** This works for the
system libraries but not for the C runtime, for the reason above.

## What works

Install the component:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --force `
  --override "--wait --passive --norestart --add Microsoft.VisualStudio.Component.VC.Tools.ARM64"
```

The **Visual Studio Installer** needs to be running elevated for a quiet or
passive install, and it says so:

```text
Commands with --quiet or --passive should be run elevated from the beginning.
```

From a non-elevated shell, use `Start-Process -Verb RunAs` — and **quote the
install path**, because the installer's own argument parser does not:

```powershell
$setup = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe"
Start-Process -FilePath $setup -Verb RunAs -ArgumentList @(
    'modify',
    '--installPath', '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
    '--passive', '--norestart'
)
```

An unquoted path is truncated at the first space, and the installer then
reports a confusing error:

```text
An installed product matching the following parameters cannot be found:
installPath: C:\Program
```

## What the build tool does about it

`cargo xtask dist` checks for the ARM64 cross-compiler before it starts, and
says what to install if it is missing:

```text
dist: building aarch64-pc-windows-msvc (arm64)
Error: the ARM64 cross-compiler is not installed.

The ARM64 build needs the 'MSVC v143 ARM64 build tools' component of the
Visual Studio Build Tools. Install it with:

  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --force \
    --override "--wait --passive --norestart --add Microsoft.VisualStudio.Component.VC.Tools.ARM64"
```

A failed link with "link.exe not found" tells nobody what to do; this does.

## CI

The `windows-latest` GitHub runner image includes the ARM64 cross-compiler,
so the release workflow builds both architectures without any extra setup. The
local requirement is only for developers who want the ARM64 artifacts on
their own machine.

## See also

- [`docs/DECISIONS.md`](../DECISIONS.md)
- [Port note 003 — The Windows API shapes in windows-rs 0.62](003-windows-rs-api.md)
