extern crate x264_sys as ffi;

use ffi::x264::*;
use std::ffi::CString;
use std::mem;
use std::os::raw::c_int;
use std::ptr::null;

pub struct Picture {
    pic: x264_picture_t,
    plane_size: [usize; 3],
    native: bool,
}

struct ColorspaceScale {
    w: [usize; 3],
    h: [usize; 3],
}

impl ColorspaceScale {
    fn iter(&self) -> impl Iterator<Item = (&usize, &usize)> {
        self.w.iter().zip(self.h.iter())
    }
}

fn scale_from_csp(csp: u32) -> ColorspaceScale {
    match csp {
        X264_CSP_I420 => ColorspaceScale {
            w: [256, 256 / 2, 256 / 2],
            h: [256, 256 / 2, 256 / 2],
        },
        X264_CSP_YV12 => ColorspaceScale {
            w: [256, 256 / 2, 256 / 2],
            h: [256, 256 / 2, 256 / 2],
        },
        X264_CSP_NV12 => ColorspaceScale {
            w: [256, 256, 0],
            h: [256, 256 / 2, 0],
        },
        X264_CSP_NV21 => ColorspaceScale {
            w: [256, 256, 0],
            h: [256, 256 / 2, 0],
        },
        X264_CSP_I422 => ColorspaceScale {
            w: [256, 256 / 2, 256 / 2],
            h: [256, 256, 256],
        },
        X264_CSP_YV16 => ColorspaceScale {
            w: [256, 256 / 2, 256 / 2],
            h: [256, 256, 256],
        },
        X264_CSP_NV16 => ColorspaceScale {
            w: [256, 256, 0],
            h: [256, 256, 0],
        },
        // X264_CSP_YUYV => {
        //     ColorspaceScale {
        //         w: [256 * 2, 0, 0],
        //         h: [256, 0, 0],
        //     }
        // }
        // X264_CSP_UYVY => {
        //     ColorspaceScale {
        //         w: [256 * 2, 0, 0],
        //         h: [256, 0, 0],
        //     }
        // }
        X264_CSP_I444 => ColorspaceScale {
            w: [256, 256, 256],
            h: [256, 256, 256],
        },
        X264_CSP_YV24 => ColorspaceScale {
            w: [256, 256, 256],
            h: [256, 256, 256],
        },
        X264_CSP_BGR => ColorspaceScale {
            w: [256 * 3, 0, 0],
            h: [256, 0, 0],
        },
        X264_CSP_BGRA => ColorspaceScale {
            w: [256 * 4, 0, 0],
            h: [256, 0, 0],
        },
        X264_CSP_RGB => ColorspaceScale {
            w: [256 * 3, 0, 0],
            h: [256, 0, 0],
        },
        _ => unreachable!(),
    }
}

impl Picture {
    /*
        pub fn new() -> Picture {
            let mut pic = unsafe { mem::uninitialized() };

            unsafe { x264_picture_init(&mut pic as *mut x264_picture_t) };

            Picture { pic: pic }
        }
    */

    pub fn from_param(param: &Param) -> Result<Picture, &'static str> {
        let mut pic: mem::MaybeUninit<x264_picture_t> = mem::MaybeUninit::uninit();

        let ret = unsafe {
            x264_picture_alloc(
                pic.as_mut_ptr(),
                param.par.i_csp,
                param.par.i_width,
                param.par.i_height,
            )
        };
        if ret < 0 {
            Err("Allocation Failure")
        } else {
            let pic = unsafe { pic.assume_init() };
            let i_plane = pic.img.i_plane as usize;

            let scale = scale_from_csp(param.par.i_csp as u32 & X264_CSP_MASK as u32);
            let bytes = 1 + (param.par.i_csp as u32 & X264_CSP_HIGH_DEPTH as u32);

            let mut plane_size = [0; 3];
            plane_size
                .iter_mut()
                .zip(scale.iter())
                .take(i_plane)
                .for_each(|(size, (w, h))| {
                    *size = param.par.i_width as usize * w / 256
                        * bytes as usize
                        * param.par.i_height as usize
                        * h
                        / 256;
                });

            Ok(Picture {
                pic,
                plane_size,
                native: true,
            })
        }
    }

    pub fn as_slice<'a>(&'a self, plane: usize) -> Result<&'a [u8], &'static str> {
        if plane > self.pic.img.i_plane as usize {
            Err("Invalid Argument")
        } else {
            let size = self.plane_size[plane];
            Ok(unsafe { std::slice::from_raw_parts(self.pic.img.plane[plane], size) })
        }
    }

    pub fn as_mut_slice<'a>(&'a mut self, plane: usize) -> Result<&'a mut [u8], &'static str> {
        if plane > self.pic.img.i_plane as usize {
            Err("Invalid Argument")
        } else {
            let size = self.plane_size[plane];
            Ok(unsafe { std::slice::from_raw_parts_mut(self.pic.img.plane[plane], size) })
        }
    }

    pub fn set_timestamp(mut self, pts: i64) -> Picture {
        self.pic.i_pts = pts;
        self
    }
}

impl Drop for Picture {
    fn drop(&mut self) {
        if self.native {
            unsafe { x264_picture_clean(&mut self.pic as *mut x264_picture_t) };
        }
    }
}

// TODO: Provide a builder API instead?
pub struct Param {
    par: x264_param_t,
}

impl Default for Param {
    fn default() -> Self {
        Self::new()
    }
}

impl Param {
    pub fn new() -> Param {
        let mut par: mem::MaybeUninit<x264_param_t> = mem::MaybeUninit::uninit();

        let par = unsafe {
            x264_param_default(par.as_mut_ptr());
            par.assume_init()
        };

        Param { par }
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub fn default_preset<'a, 'b, Oa, Ob>(preset: Oa, tune: Ob) -> Result<Param, &'static str>
    where
        Oa: Into<Option<&'a str>>,
        Ob: Into<Option<&'b str>>,
    {
        let mut par: mem::MaybeUninit<x264_param_t> = mem::MaybeUninit::uninit();
        let t = tune
            .into()
            .map_or_else(|| None, |v| Some(CString::new(v).unwrap()));
        let p = preset
            .into()
            .map_or_else(|| None, |v| Some(CString::new(v).unwrap()));

        let c_tune = t.as_ref().map_or_else(null, |v| v.as_ptr() as *const u8);
        let c_preset = p.as_ref().map_or_else(null, |v| v.as_ptr() as *const u8);

        match unsafe { x264_param_default_preset(par.as_mut_ptr(), c_preset, c_tune) } {
            -1 => Err("Invalid Argument"),
            0 => Ok(Param {
                par: unsafe { par.assume_init() },
            }),
            _ => Err("Unexpected"),
        }
    }

    #[cfg(any(not(target_os = "linux"), target_arch = "x86_64"))]
    pub fn default_preset<'a, 'b, Oa, Ob>(preset: Oa, tune: Ob) -> Result<Param, &'static str>
    where
        Oa: Into<Option<&'a str>>,
        Ob: Into<Option<&'b str>>,
    {
        let mut par: mem::MaybeUninit<x264_param_t> = mem::MaybeUninit::uninit();
        let t = tune
            .into()
            .map_or_else(|| None, |v| Some(CString::new(v).unwrap()));
        let p = preset
            .into()
            .map_or_else(|| None, |v| Some(CString::new(v).unwrap()));

        let c_tune = t.as_ref().map_or_else(null, |v| v.as_ptr() as *const i8);
        let c_preset = p.as_ref().map_or_else(null, |v| v.as_ptr() as *const i8);

        match unsafe { x264_param_default_preset(par.as_mut_ptr(), c_preset, c_tune) } {
            -1 => Err("Invalid Argument"),
            0 => Ok(Param {
                par: unsafe { par.assume_init() },
            }),
            _ => Err("Unexpected"),
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub fn apply_profile(mut self, profile: &str) -> Result<Param, &'static str> {
        let p = CString::new(profile).unwrap();
        match unsafe { x264_param_apply_profile(&mut self.par, p.as_ptr() as *const u8) } {
            -1 => Err("Invalid Argument"),
            0 => Ok(self),
            _ => Err("Unexpected"),
        }
    }

    #[cfg(any(not(target_os = "linux"), target_arch = "x86_64"))]
    pub fn apply_profile(mut self, profile: &str) -> Result<Param, &'static str> {
        let p = CString::new(profile).unwrap();
        match unsafe { x264_param_apply_profile(&mut self.par, p.as_ptr() as *const i8) } {
            -1 => Err("Invalid Argument"),
            0 => Ok(self),
            _ => Err("Unexpected"),
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub fn param_parse(mut self, name: &str, value: &str) -> Result<Param, &'static str> {
        let n = CString::new(name).unwrap();
        let v = CString::new(value).unwrap();
        match unsafe {
            x264_param_parse(
                &mut self.par,
                n.as_ptr() as *const u8,
                v.as_ptr() as *const u8,
            )
        } {
            -1 => Err("Invalid Argument"),
            0 => Ok(self),
            _ => Err("Unexpected"),
        }
    }

    #[cfg(any(not(target_os = "linux"), target_arch = "x86_64"))]
    pub fn param_parse(mut self, name: &str, value: &str) -> Result<Param, &'static str> {
        let n = CString::new(name).unwrap();
        let v = CString::new(value).unwrap();
        match unsafe {
            x264_param_parse(
                &mut self.par,
                n.as_ptr() as *const i8,
                v.as_ptr() as *const i8,
            )
        } {
            -1 => Err("Invalid Argument"),
            0 => Ok(self),
            _ => Err("Unexpected"),
        }
    }

    pub fn set_csp(mut self, value: usize) -> Param {
        self.par.i_csp = value as c_int;

        self
    }      

    pub fn set_fullrange(mut self, value: usize) -> Param {
        self.par.vui.b_fullrange = value as c_int;

        self
    }

    pub fn set_psy_rd(mut self, value: f32) -> Param {
        self.par.analyse.f_psy_rd = value;

        self
    }

    pub fn set_psy_trellis(mut self, value: f32) -> Param {
        self.par.analyse.f_psy_trellis = value;
        

        self
    }

    pub fn set_colormatrix(mut self, value: usize) -> Param {
        self.par.vui.i_colmatrix = value as c_int;

        self
    }

    pub fn set_dimension(mut self, height: usize, width: usize) -> Param {
        self.par.i_height = height as c_int;
        self.par.i_width = width as c_int;

        self
    }
}

// TODO: Expose a NAL abstraction
pub struct NalData {
    vec: Vec<u8>,
}

impl NalData {
    /*
     * x264 functions return x264_nal_t arrays that are valid only until another
     * function of that kind is called.
     *
     * Always copy the data over.
     *
     * TODO: Consider using Bytes as backing store.
     */
    fn from_nals(c_nals: *mut x264_nal_t, nb_nal: usize) -> NalData {
        let mut data = NalData { vec: Vec::new() };

        for i in 0..nb_nal {
            let nal = unsafe { Box::from_raw(c_nals.add(i)) };

            let payload =
                unsafe { std::slice::from_raw_parts(nal.p_payload, nal.i_payload as usize) };

            data.vec.extend_from_slice(payload);

            mem::forget(nal);
        }

        data
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.vec.as_slice()
    }
}

pub struct Encoder {
    enc: *mut x264_t,
}

impl Encoder {
    pub fn open(par: &mut Param) -> Result<Encoder, &'static str> {
        let enc = unsafe { x264_encoder_open(&mut par.par as *mut x264_param_t) };

        if enc.is_null() {
            Err("Out of Memory")
        } else {
            Ok(Encoder { enc })
        }
    }

    pub fn get_headers(&mut self) -> Result<NalData, &'static str> {
        let mut nb_nal: c_int = 0;
        let mut c_nals: mem::MaybeUninit<*mut x264_nal_t> = mem::MaybeUninit::uninit();

        let bytes = unsafe {
            x264_encoder_headers(self.enc, c_nals.as_mut_ptr(), &mut nb_nal as *mut c_int)
        };

        if bytes < 0 {
            Err("Encoding Headers Failed")
        } else {
            let c_nals = unsafe { c_nals.assume_init() };
            Ok(NalData::from_nals(c_nals, nb_nal as usize))
        }
    }

    pub fn encode<'a, P>(&mut self, pic: P) -> Result<Option<(NalData, i64, i64)>, &'static str>
    where
        P: Into<Option<&'a Picture>>,
    {
        let mut pic_out: mem::MaybeUninit<x264_picture_t> = mem::MaybeUninit::uninit();
        let mut c_nals: mem::MaybeUninit<*mut x264_nal_t> = mem::MaybeUninit::uninit();
        let mut nb_nal: c_int = 0;
        let c_pic = pic
            .into()
            .map_or_else(null, |v| &v.pic as *const x264_picture_t);

        let ret = unsafe {
            x264_encoder_encode(
                self.enc,
                c_nals.as_mut_ptr(),
                &mut nb_nal as *mut c_int,
                c_pic as *mut x264_picture_t,
                pic_out.as_mut_ptr(),
            )
        };
        if ret < 0 {
            Err("Error encoding")
        } else {
            let pic_out = unsafe { pic_out.assume_init() };
            let c_nals = unsafe { c_nals.assume_init() };

            if nb_nal > 0 {
                let data = NalData::from_nals(c_nals, nb_nal as usize);
                Ok(Some((data, pic_out.i_pts, pic_out.i_dts)))
            } else {
                Ok(None)
            }
        }
    }

    pub fn delayed_frames(&self) -> bool {
        unsafe { x264_encoder_delayed_frames(self.enc) != 0 }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { x264_encoder_close(self.enc) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open() {
        let mut par = Param::new().set_dimension(640, 480);

        let mut enc = Encoder::open(&mut par).unwrap();

        let headers = enc.get_headers().unwrap();

        println!("Headers len {}", headers.as_bytes().len());
    }

    #[test]
    fn test_picture() {
        let par = Param::new().set_dimension(640, 480);
        {
            let mut pic = Picture::from_param(&par).unwrap();
            {
                let p = pic.as_mut_slice(0).unwrap();
                p[0] = 1;
            }
            let p = pic.as_slice(0).unwrap();

            assert_eq!(p[0], 1);
        }
    }

    #[test]
    fn test_encode() {
        let mut par = Param::new().set_dimension(640, 480);
        let mut enc = Encoder::open(&mut par).unwrap();
        let mut pic = Picture::from_param(&par).unwrap();

        let headers = enc.get_headers().unwrap();

        println!("Headers len {}", headers.as_bytes().len());

        for pts in 0..5 {
            pic = pic.set_timestamp(pts as i64);
            let ret = enc.encode(&pic).unwrap();

            if let Some((_, pts, dts)) = ret {
                println!("Frame pts {}, dts {}", pts, dts);
            }
        }

        while enc.delayed_frames() {
            let ret = enc.encode(None).unwrap();
            if let Some((_, pts, dts)) = ret {
                println!("Frame pts {}, dts {}", pts, dts);
            }
        }
    }
}
