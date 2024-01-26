use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn format_write(builder: bindgen::Builder) -> String {
    builder
        .generate()
        .unwrap()
        .to_string()
        .replace("/**", "/*")
        .replace("/*!", "/*")
}

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo:warning={}", format!($($tokens)*))
    }
}

fn main() {
    let libs = system_deps::Config::new().probe().unwrap();
    // TODO pass include paths to bindgen
    let x264 = libs.get_by_name("x264").unwrap();
    let headers = x264.include_paths.clone();
    let buildver = x264.version.split('.').nth(1).unwrap();
   
    if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        println!("cargo:rustc-link-search=native=/usr/lib/aarch64-linux-gnu/");
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu/");
    } 
    
    println!("cargo:rustc-link-lib=static=x264");
    
    let mut builder = bindgen::builder()
        .raw_line(format!(
            "pub unsafe fn x264_encoder_open(params: *mut x264_param_t) -> *mut x264_t {{
                               x264_encoder_open_{}(params)
                          }}",
            buildver
        ))
        .header("data/x264.h");

    for header in headers {
        builder = builder.clang_arg("-I").clang_arg(header.to_str().unwrap());
    }

    // Manually fix the comment so rustdoc won't try to pick them
    let s = format_write(builder);

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut file = File::create(out_path.join("x264.rs")).unwrap();

    let _ = file.write(s.as_bytes());
}
