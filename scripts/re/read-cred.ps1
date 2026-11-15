# Reads one credential from Windows Credential Manager and prints it.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File read-cred.ps1 <target> <user>
#
# The value is printed to the console only. Nothing is written to disk.
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [string]$User = ''
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class Cred {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct CREDENTIAL {
        public uint Flags;
        public uint Type;
        public IntPtr TargetName;
        public IntPtr Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        public IntPtr TargetAlias;
        public IntPtr UserName;
    }

    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", EntryPoint = "CredFree", SetLastError = true)]
    public static extern void CredFree(IntPtr buffer);
}
"@

$type = 1  # CRED_TYPE_GENERIC
$pointer = [IntPtr]::Zero

if (-not [Cred]::CredRead($Target, $type, 0, [ref]$pointer)) {
    Write-Output "not found (error $((Get-ItemProperty -Path Env:UNKNOWN -ErrorAction SilentlyContinue)))"
    exit 1
}

try {
    $credential = [System.Runtime.InteropServices.Marshal]::PtrToStructure(
        $pointer, [type][Cred+CREDENTIAL])
    $size = $credential.CredentialBlobSize
    $slice = New-Object byte[] $size
    [System.Runtime.InteropServices.Marshal]::Copy(
        $credential.CredentialBlob, $slice, 0, $size)
    $text = [System.Text.Encoding]::Unicode.GetString($slice)
    Write-Output "target: $Target"
    Write-Output "user:   $([System.Runtime.InteropServices.Marshal]::PtrToStringUni($credential.UserName))"
    Write-Output "bytes:  $size"
    Write-Output "value:  $text"
}
finally {
    [Cred]::CredFree($pointer)
}
