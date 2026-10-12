//! Layout checks for the hand-written `#[repr(C)]` structures.
//!
//! Expected values were measured by compiling the vendor header with gcc on
//! x86_64 Linux (`sizeof` / `offsetof`); 32-bit values follow from the same
//! field list with 4-byte pointers.

#![allow(non_snake_case)]

use std::mem::{align_of, offset_of, size_of};

use gcam_mvsdk_rs::ffi::*;

#[test]
fn camera_dev_info_layout() {
    assert_eq!(size_of::<tSdkCameraDevInfo>(), 260);
    assert_eq!(align_of::<tSdkCameraDevInfo>(), 4);
    assert_eq!(offset_of!(tSdkCameraDevInfo, acProductSeries), 0);
    assert_eq!(offset_of!(tSdkCameraDevInfo, acProductName), 32);
    assert_eq!(offset_of!(tSdkCameraDevInfo, acFriendlyName), 64);
    assert_eq!(offset_of!(tSdkCameraDevInfo, acPortType), 192);
    assert_eq!(offset_of!(tSdkCameraDevInfo, acSn), 224);
    assert_eq!(offset_of!(tSdkCameraDevInfo, uInstance), 256);
}

#[test]
fn frame_head_layout() {
    assert_eq!(size_of::<tSdkFrameHead>(), 64);
    assert_eq!(align_of::<tSdkFrameHead>(), 4);
    assert_eq!(offset_of!(tSdkFrameHead, uBytes), 4);
    assert_eq!(offset_of!(tSdkFrameHead, iWidth), 8);
    assert_eq!(offset_of!(tSdkFrameHead, iHeight), 12);
    assert_eq!(offset_of!(tSdkFrameHead, uiTimeStamp), 28);
    assert_eq!(offset_of!(tSdkFrameHead, uiExpTime), 32);
    assert_eq!(offset_of!(tSdkFrameHead, fBgain), 60);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn capability_layout_64() {
    assert_eq!(size_of::<tSdkCameraCapbility>(), 344);
    assert_eq!(align_of::<tSdkCameraCapbility>(), 8);
    assert_eq!(offset_of!(tSdkCameraCapbility, iTriggerDesc), 8);
    assert_eq!(offset_of!(tSdkCameraCapbility, pImageSizeDesc), 16);
    assert_eq!(offset_of!(tSdkCameraCapbility, iImageSizeDesc), 24);
    assert_eq!(offset_of!(tSdkCameraCapbility, pMediaTypeDesc), 48);
    assert_eq!(offset_of!(tSdkCameraCapbility, pFrameSpeedDesc), 64);
    assert_eq!(offset_of!(tSdkCameraCapbility, iFrameSpeedDesc), 72);
    assert_eq!(offset_of!(tSdkCameraCapbility, pPackLenDesc), 80);
    assert_eq!(offset_of!(tSdkCameraCapbility, iPackLenDesc), 88);
    assert_eq!(offset_of!(tSdkCameraCapbility, iOutputIoCounts), 92);
    assert_eq!(offset_of!(tSdkCameraCapbility, iInputIoCounts), 96);
    assert_eq!(offset_of!(tSdkCameraCapbility, pPresetLutDesc), 104);
    assert_eq!(offset_of!(tSdkCameraCapbility, iUserDataMaxLen), 116);
    assert_eq!(offset_of!(tSdkCameraCapbility, bParamInDevice), 120);
    assert_eq!(offset_of!(tSdkCameraCapbility, pAeAlmSwDesc), 128);
    assert_eq!(offset_of!(tSdkCameraCapbility, pAeAlmHdDesc), 144);
    assert_eq!(offset_of!(tSdkCameraCapbility, pBayerDecAlmSwDesc), 160);
    assert_eq!(offset_of!(tSdkCameraCapbility, pBayerDecAlmHdDesc), 176);
    assert_eq!(offset_of!(tSdkCameraCapbility, iBayerDecAlmHdDesc), 184);
    assert_eq!(offset_of!(tSdkCameraCapbility, sExposeDesc), 188);
    assert_eq!(offset_of!(tSdkCameraCapbility, sResolutionRange), 216);
    assert_eq!(offset_of!(tSdkCameraCapbility, sRgbGainRange), 248);
    assert_eq!(offset_of!(tSdkCameraCapbility, sSaturationRange), 272);
    assert_eq!(offset_of!(tSdkCameraCapbility, sGammaRange), 280);
    assert_eq!(offset_of!(tSdkCameraCapbility, sContrastRange), 288);
    assert_eq!(offset_of!(tSdkCameraCapbility, sSharpnessRange), 296);
    assert_eq!(offset_of!(tSdkCameraCapbility, sIspCapacity), 304);
}

#[test]
#[cfg(target_pointer_width = "32")]
fn capability_layout_32() {
    assert_eq!(size_of::<tSdkCameraCapbility>(), 256);
    assert_eq!(offset_of!(tSdkCameraCapbility, sExposeDesc), 104);
    assert_eq!(offset_of!(tSdkCameraCapbility, sIspCapacity), 220);
}

#[test]
fn descriptor_layouts() {
    assert_eq!(size_of::<tSdkImageResolution>(), 92);
    assert_eq!(size_of::<tSdkPackLength>(), 40);
    assert_eq!(offset_of!(tSdkPackLength, iPackSize), 36);
    assert_eq!(size_of::<tSdkFrameSpeed>(), 36);
    assert_eq!(size_of::<tSdkTrigger>(), 36);
    assert_eq!(size_of::<tSdkColorTemperatureDes>(), 36);
    assert_eq!(size_of::<tSdkPresetLut>(), 36);
    assert_eq!(size_of::<tSdkAeAlgorithm>(), 36);
    assert_eq!(size_of::<tSdkBayerDecodeAlgorithm>(), 36);
    assert_eq!(size_of::<tSdkMediaType>(), 40);
    assert_eq!(size_of::<tSdkExpose>(), 28);
    assert_eq!(size_of::<tSdkResolutionRange>(), 32);
    assert_eq!(size_of::<tRgbGainRange>(), 24);
    assert_eq!(size_of::<tSdkIspCapacity>(), 36);
    assert_eq!(size_of::<tSdkFrameStatistic>(), 12);
    assert_eq!(size_of::<tSdkRect>(), 16);
    assert_eq!(size_of::<tGammaRange>(), 8);
}

#[test]
fn camera_is_send_but_not_sync() {
    fn assert_send<T: Send>() {}
    assert_send::<gcam_mvsdk_rs::Camera>();
    assert_send::<gcam_mvsdk_rs::Sdk>();

    // Compile-time negative check: if `Camera` were `Sync`, both impls would
    // apply and the call below would be ambiguous, failing to compile.
    trait AmbiguousIfSync<A> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfSync<()> for T {}
    #[allow(dead_code)]
    struct Invalid;
    impl<T: ?Sized + Sync> AmbiguousIfSync<Invalid> for T {}
    <gcam_mvsdk_rs::Camera as AmbiguousIfSync<_>>::check();
}
