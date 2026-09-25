fn main() {
    // Only the Tauri app needs Tauri's build step; the macOS helper is
    // built without it.
    #[cfg(feature = "gui")]
    gui();
}

#[cfg(feature = "gui")]
fn gui() {
    // Every disk/partition write on Windows goes through PowerShell's
    // Storage module (Remove-Partition, Format-Volume, New-Partition, ...),
    // which refuses with a CIM/WMI "access denied" error unless the
    // process is elevated. Requiring administrator here means the one UAC
    // prompt happens at app launch instead of failing deep inside an
    // operation, and it only has to be answered once per run rather than
    // once per action.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#,
    );

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run build script");
}
