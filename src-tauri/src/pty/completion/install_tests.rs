use super::*;

#[cfg(unix)]
#[test]
fn zsh_startup_preserves_profiles_and_restores_zdotdir() {
    if !Path::new("/bin/zsh").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("vlx-zsh-startup-{}", uuid::Uuid::new_v4()));
    let original = root.join("user ' 中文");
    std::fs::create_dir_all(&original).unwrap();
    for (name, marker) in [
        (".zshenv", "E"),
        (".zprofile", "P"),
        (".zshrc", "R"),
        (".zlogin", "L"),
    ] {
        std::fs::write(
            original.join(name),
            format!("VLX_TEST_ORDER+=\"{marker}\"\n"),
        )
        .unwrap();
    }
    let (state, _) = install(&root, "/bin/zsh").unwrap().unwrap();
    let mut cmd = portable_pty::CommandBuilder::new("/bin/zsh");
    cmd.env("ZDOTDIR", &original);
    configure_zsh_startup(&state, &mut cmd, &[]).unwrap();
    let output = std::process::Command::new("/bin/zsh")
        .env_clear()
        .env("HOME", &original)
        .env("PATH", "/usr/bin:/bin")
        .env("ZDOTDIR", cmd.get_env("ZDOTDIR").unwrap())
        .args([
            "-lic",
            "printf 'RESULT:%s:%s:%s|%s' \"$VLX_TEST_ORDER\" \"$ZDOTDIR\" \"$_vlxc_nonce\" \"$HISTFILE\"",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let raw = String::from_utf8_lossy(&output.stdout);
    // The integration announces itself with OSC 6973 sequences, which the PTY reader strips before display.
    let mut stdout = String::new();
    let mut rest = raw.as_ref();
    while let Some(start) = rest.find("\x1b]6973;") {
        stdout.push_str(&rest[..start]);
        rest = rest[start..].split_once('\x07').map_or("", |(_, after)| after);
    }
    stdout.push_str(rest);
    let (result, histfile) = stdout.rsplit_once('|').unwrap();
    assert_eq!(result, format!("RESULT:EPRL:{}:{}", original.display(), state.nonce));
    // macOS /etc/zshrc sets HISTFILE from ZDOTDIR; it must follow the user's directory, not the startup one.
    if !histfile.is_empty() {
        assert_eq!(histfile, original.join(".zsh_history").to_string_lossy());
    }
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scripts_are_private_and_powershell_paths_are_utf8() {
    let root = std::env::temp_dir().join(format!("vlx-completion-中文-{}", uuid::Uuid::new_v4()));
    let (state, launch) = install(&root, "powershell.exe").unwrap().unwrap();
    assert!(state.snapshot.configured);
    assert!(!state.snapshot.supported);
    let script = state
        .selection_file
        .parent()
        .unwrap()
        .join("integration.ps1");
    let bytes = std::fs::read(&script).unwrap();
    assert!(bytes.starts_with(b"\xef\xbb\xbf"));
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("@SELECTION@"));
    assert!(!text.contains("@NONCE@"));
    assert!(launch.contains("中文"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(script.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    drop(state);
    assert!(!script.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn bash_startup_replays_login_profiles_and_loads_integration() {
    if !Path::new("/bin/bash").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("vlx-bash-startup-' 中文-{}", uuid::Uuid::new_v4()));
    let home = root.join("user ' 中文");
    std::fs::create_dir_all(&home).unwrap();
    // Bash reads the first readable file of this list only, so a login shell records just one marker.
    for (name, marker) in [
        (".bash_profile", "P"),
        (".bash_login", "L"),
        (".profile", "R"),
    ] {
        std::fs::write(
            home.join(name),
            format!("VLX_TEST_ORDER+=\"{marker}\"\n"),
        )
        .unwrap();
    }
    let (state, _) = install(&root, "/bin/bash").unwrap().unwrap();
    let rcfile = configure_bash_startup(&state, &[]).unwrap();
    let output = std::process::Command::new("/bin/bash")
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .args([
            "--rcfile",
            &rcfile.to_string_lossy(),
            "-ic",
            "printf 'RESULT:%s:%s' \"$VLX_TEST_ORDER\" \"$_vlxc_nonce\"",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    // The integration announces its state first; Bash 3 reports itself unsupported right when it loads.
    assert!(String::from_utf8_lossy(&output.stdout)
        .ends_with(&format!("RESULT:P:{}", state.nonce)));
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn bash_startup_finds_opencode_installed_after_the_app_started() {
    use std::os::unix::fs::PermissionsExt;
    if !Path::new("/bin/bash").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("vlx-bash-opencode-' 中文-{}", uuid::Uuid::new_v4()));
    let home = root.join("user ' 中文");
    let bin_dir = home.join(".opencode/bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".bash_profile"), "export PATH=/usr/bin:/bin\nVLX_TEST_PROFILE=P\n").unwrap();
    std::fs::write(home.join(".bash_login"), "VLX_TEST_PROFILE=L\n").unwrap();
    let (state, _) = install(&root, "/bin/bash").unwrap().unwrap();
    let run = |rcfile: &Path| {
        let output = std::process::Command::new("/bin/bash")
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .args([
                "--rcfile",
                &rcfile.to_string_lossy(),
                "-ic",
                "if command -v opencode >/dev/null; then printf '\\nRESULT:%s:%s:%s\\n' \"$(opencode)\" \"$VLX_TEST_PROFILE\" \"${VLX_TEST_RC_LOADS:-0}\"; else printf '\\nRESULT:missing\\n'; fi",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let rcfile = configure_bash_startup(&state, &[]).unwrap();
    assert!(run(&rcfile).ends_with("RESULT:missing\n"));

    // Reproduce the installer adding a binary and editing .bashrc while the app's PATH stays unchanged.
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin = bin_dir.join("opencode");
    std::fs::write(&bin, "#!/bin/sh\nprintf opencode-fixture-ok\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(home.join(".bashrc"), "export PATH=\"$HOME/.opencode/bin:$PATH\"\nVLX_TEST_RC_LOADS=$(( ${VLX_TEST_RC_LOADS:-0} + 1 ))\n").unwrap();
    let rcfile = configure_bash_startup(&state, &[bin.to_string_lossy().into_owned()]).unwrap();
    assert!(run(&rcfile).ends_with("RESULT:opencode-fixture-ok:P:0\n"));

    // Profiles that already source .bashrc must keep doing so exactly once.
    std::fs::write(home.join(".bash_profile"), "export PATH=/usr/bin:/bin\nVLX_TEST_PROFILE=P\nsource \"$HOME/.bashrc\"\n").unwrap();
    assert!(run(&rcfile).ends_with("RESULT:opencode-fixture-ok:P:1\n"));
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn bash_startup_agent_paths_preserve_order_and_require_executables() {
    use std::os::unix::fs::PermissionsExt;
    if !Path::new("/bin/bash").exists() {
        return;
    }
    let root = std::env::temp_dir().join(format!("vlx-bash-path-{}", uuid::Uuid::new_v4()));
    let home = root.join("user");
    let fallback_dir = home.join(".opencode/bin");
    let preferred_dir = home.join("preferred");
    std::fs::create_dir_all(&fallback_dir).unwrap();
    std::fs::create_dir_all(&preferred_dir).unwrap();
    let fallback = fallback_dir.join("opencode");
    for (path, label) in [(&fallback, "fallback"), (&preferred_dir.join("opencode"), "preferred")] {
        std::fs::write(path, format!("#!/bin/sh\nprintf {label}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (state, _) = install(&root, "/bin/bash").unwrap().unwrap();
    let run = |rcfile: &Path| {
        let output = std::process::Command::new("/bin/bash")
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .args([
                "--rcfile",
                &rcfile.to_string_lossy(),
                "-ic",
                "printf '\\nRESULT:%s\\nPATH:%s\\n' \"$(opencode 2>/dev/null)\" \"$PATH\"",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    // A profile can replace PATH; its preferred installation must keep precedence over our fallback.
    let preferred_path = format!("{}:/usr/bin:/bin", preferred_dir.display());
    std::fs::write(home.join(".bash_profile"), format!("export PATH='{preferred_path}'\n")).unwrap();
    let rcfile = configure_bash_startup(&state, &[fallback.to_string_lossy().into_owned()]).unwrap();
    assert!(run(&rcfile).ends_with(&format!("RESULT:preferred\nPATH:{preferred_path}:{}\n", fallback_dir.display())));

    let present_path = format!("{preferred_path}:{}", fallback_dir.display());
    std::fs::write(home.join(".bash_profile"), format!("export PATH='{present_path}'\n")).unwrap();
    assert!(run(&rcfile).ends_with(&format!("RESULT:preferred\nPATH:{present_path}\n")));

    // A partial download without its executable permission, or a missing binary, adds no PATH entry.
    std::fs::write(home.join(".bash_profile"), format!("export PATH='{preferred_path}'\n")).unwrap();
    std::fs::set_permissions(&fallback, std::fs::Permissions::from_mode(0o644)).unwrap();
    let rcfile = configure_bash_startup(&state, &[fallback.to_string_lossy().into_owned()]).unwrap();
    assert!(run(&rcfile).ends_with(&format!("RESULT:preferred\nPATH:{preferred_path}\n")));
    std::fs::remove_file(&fallback).unwrap();
    let rcfile = configure_bash_startup(&state, &[fallback.to_string_lossy().into_owned()]).unwrap();
    assert!(run(&rcfile).ends_with(&format!("RESULT:preferred\nPATH:{preferred_path}\n")));
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}
