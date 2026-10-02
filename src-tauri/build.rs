fn main() {
    // requireAdministrator：每次启动强制 UAC（NVIDIA 颜色/ICC 安装/进程监听都需要管理员权限）。
    // 开机自启必须走计划任务（admin.rs）：注册表 Run 项在提权清单下会开机弹 UAC。
    // ⚠ manifest XML 必须保持纯 ASCII：SxS 解析器对无 BOM 清单按系统 ANSI 代码页解码，
    //   中文（含 XML 注释）会报「无效的 Xml 语法」os error 14001，exe 直接无法启动。
    let windows = tauri_build::WindowsAttributes::new().app_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df" language="*" />
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
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run build script");
}
