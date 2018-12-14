use std::process::Command;
use std::str;

fn command_output(cmd: &mut Command) -> String {
    str::from_utf8(&cmd.output().unwrap().stdout)
        .unwrap()
        .trim_right()
        .to_string()
}

fn stack_dylib_output_dir() -> String {
    let mut s = command_output(Command::new("stack").args(&["path", "--local-install-root"]));
    s.push_str("/lib");
    s
}

fn main() {
    println!(
        "cargo:rustc-link-search=native={}",
        &stack_dylib_output_dir()
    );
    println!("cargo:rustc-link-lib=dylib=pandoc-ffi-bridge");
}
