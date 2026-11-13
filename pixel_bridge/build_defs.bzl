"""Build definitions for pixel bridge."""

# Platforms where Rust decoding is unavailable.
UNSUPPORTED_PLATFORMS_RUST = (
    "//tools/cc_target_os:chromiumos",
    "//tools/cc_target_os:darwin",
    "//tools/cc_target_os:fuchsia",
    "//tools/cc_target_os:android",
    "//tools/cc_target_os:wasm",
    "//tools/cc_target_os:hercules_embedded",
    "//tools/cc_target_os:platform_ios",
    "//tools/cc_target_os:platform_tvos",
    "//tools/cc_target_os:platform_visionos",
    "//tools/cc_target_os:platform_watchos",
    "//tools/cc_target_os:mpu64",
    "//tools/cc_target_os:nestcam_ambarella",
    "//tools/cc_target_os:windows",
)
