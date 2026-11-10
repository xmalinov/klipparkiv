fn main() {
    tauri_build::build();

    println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
    println!("cargo:rerun-if-changed=cpp/wrapper.h");
    println!("cargo:rerun-if-changed=cpp/signalsmith-stretch.h");
    println!("cargo:rerun-if-changed=cpp/signalsmith-linear/stft.h");
    println!("cargo:rerun-if-changed=cpp/signalsmith-linear/fft.h");

    cc::Build::new()
        .cpp(true)
        .std("c++14")
        .file("cpp/wrapper.cpp")
        .include("cpp")
        .compile("signalsmith-stretch");
}
