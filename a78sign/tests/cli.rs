//! End-to-end tests of the `a78sign` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

fn temp_rom(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("a78sign-test-{}-{name}.bin", std::process::id()));
    let mut rom = vec![0x5Au8; 0x8000];
    for (i, b) in rom.iter_mut().enumerate() {
        *b = (i * 31 + (i >> 8)) as u8;
    }
    rom[0x8000 - 128..0x8000 - 8].fill(0);
    rom[0x8000 - 8] = 0xFF;
    rom[0x8000 - 7] = 0x87;
    rom[0x8000 - 3] = 0xF0;
    std::fs::write(&path, rom).unwrap();
    path
}

fn a78sign(args: &[&str], path: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_a78sign"))
        .args(args)
        .arg(path)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn sign_write_and_verify() {
    let path = temp_rom("sign");
    let original = std::fs::read(&path).unwrap();

    // Unsigned: -t reports it and exits nonzero, without touching the file.
    let out = a78sign(&["-t"], &path);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("appears to be empty"));
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // Signing without -w prints the signature but leaves the file alone.
    let out = a78sign(&[], &path);
    assert!(out.status.success());
    assert!(stdout(&out).contains("success!"));
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // -w writes exactly the 120 signature bytes.
    let out = a78sign(&["-w"], &path);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("Wrote back 120 bytes"));
    let signed = std::fs::read(&path).unwrap();
    let changed: Vec<usize> = (0..signed.len())
        .filter(|&i| signed[i] != original[i])
        .collect();
    assert!(!changed.is_empty());
    assert!(changed
        .iter()
        .all(|&i| (0x8000 - 128..0x8000 - 8).contains(&i)));

    // Now it verifies, and re-signing without -f does not rewrite it.
    let out = a78sign(&["-t"], &path);
    assert!(out.status.success());
    assert!(stdout(&out).contains("is valid!"));
    let out = a78sign(&["-w"], &path);
    assert!(stdout(&out).contains("not re-writing file"));
    assert_eq!(std::fs::read(&path).unwrap(), signed);

    // Re-signing with -w -f forces rewrite.
    let out = a78sign(&["-w", "-f"], &path);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Wrote back 120 bytes"));
    assert_eq!(std::fs::read(&path).unwrap(), signed);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn rejects_conflicting_and_bad_input() {
    let path = temp_rom("bad");
    let out = a78sign(&["-t", "-w"], &path);
    assert_eq!(out.status.code(), Some(2));

    let missing = std::env::temp_dir().join("a78sign-does-not-exist.bin");
    let out = a78sign(&[], &missing);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Can't open"));

    // Headered .a78 file is rejected with actionable error message
    let mut headered = vec![0x41; 128];
    headered.extend_from_slice(&std::fs::read(&path).unwrap());
    let headered_path =
        std::env::temp_dir().join(format!("a78sign-test-{}-headered.a78", std::process::id()));
    std::fs::write(&headered_path, &headered).unwrap();
    let out = a78sign(&[], &headered_path);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("128-byte .a78 header"));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&headered_path);
}
