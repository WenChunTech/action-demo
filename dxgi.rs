use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::ops::Not;
use std::{ptr, slice};

use windows::core::{Error, Interface};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_REFERENCE, D3D_DRIVER_TYPE_WARP,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_9_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_FLAG,
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE,
    D3D11_CREATE_DEVICE_FLAG, D3D11_RESOURCE_MISC_FLAG, D3D11_RESOURCE_MISC_GDI_COMPATIBLE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_CPU_ACCESS_NONE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, IDXGISurface, IDXGISurface1, DXGI_MAPPED_RECT, DXGI_MAP_READ, DXGI_OUTDUPL_DESC,
    DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME, DXGI_OUTPUT_DESC, DXGI_RESOURCE_PRIORITY_MAXIMUM,
};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, BITMAPFILEHEADER, BITMAPINFOHEADER, BI_RGB, HBRUSH, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DrawIconEx, GetCursorInfo, GetIconInfo, GetSystemMetrics, CURSORINFO, CURSOR_SHOWING,
    DI_DEFAULTSIZE, DI_NORMAL, ICONINFO, SM_CXSCREEN, SM_CYSCREEN,
};

const DRIVER_TYPES: [D3D_DRIVER_TYPE; 3] = [
    D3D_DRIVER_TYPE_HARDWARE,
    D3D_DRIVER_TYPE_WARP,
    D3D_DRIVER_TYPE_REFERENCE,
];

const FEATURE_LEVELS: [windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL; 4] = [
    D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_9_1,
];

pub fn adapter1_by_id(id: u32) -> Result<IDXGIAdapter1, Error> {
    unsafe { CreateDXGIFactory1::<IDXGIFactory1>().and_then(|e| e.EnumAdapters1(id)) }
}

pub fn dxgi_output1_by_id_and_adapter1(
    id: u32,
    adapter: &IDXGIAdapter1,
) -> Result<IDXGIOutput1, Error> {
    unsafe { adapter.EnumOutputs(id).and_then(|e| e.cast()) }
}

pub fn dxgi_output_duplication_by_output1(
    dxgi_device: &ID3D11Device,
    dxgi_output1: &IDXGIOutput1,
) -> Result<IDXGIOutputDuplication, Error> {
    unsafe { dxgi_output1.DuplicateOutput(dxgi_device) }
}

pub fn dxgi_device_and_dxgi_device_context() -> Option<(ID3D11Device, ID3D11DeviceContext)> {
    for driver_type in DRIVER_TYPES.iter() {
        let mut device: Option<ID3D11Device> = None;
        let mut immediate_context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL_9_1;
        let hr = unsafe {
            D3D11CreateDevice(
                None,
                *driver_type,
                None,
                D3D11_CREATE_DEVICE_FLAG::default(),
                Some(FEATURE_LEVELS.to_vec().as_ref()),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut immediate_context),
            )
        };
        if hr.is_ok() && device.is_some() && immediate_context.is_some() {
            return Some((unsafe { device.unwrap_unchecked() }, unsafe {
                immediate_context.unwrap_unchecked()
            }));
        }
    }

    None
}

#[derive(Debug)]
pub struct DuplicationContext {
    d3d11_device: ID3D11Device,
    d3d11_device_context: ID3D11DeviceContext,
    timeout_ms: u32,
    dxgi_output: IDXGIOutput1,
    dxgi_output_duplication: IDXGIOutputDuplication,
}

impl DuplicationContext {
    pub fn new(
        d3d11_device: ID3D11Device,
        d3d11_device_context: ID3D11DeviceContext,
        timeout_ms: u32,
        dxgi_output1: IDXGIOutput1,
        dxgi_output_duplication: IDXGIOutputDuplication,
    ) -> Self {
        Self {
            d3d11_device,
            d3d11_device_context,
            timeout_ms,
            dxgi_output: dxgi_output1,
            dxgi_output_duplication,
        }
    }

    /// This is usually used to get the screen's position and size.
    pub fn dxgi_output_desc(&self) -> Result<DXGI_OUTPUT_DESC, Error> {
        unsafe { self.dxgi_output.GetDesc() }
    }

    /// This is usually used to get the screen's pixel width/height and buffer size.
    pub fn dxgi_outdupl_desc(&self) -> DXGI_OUTDUPL_DESC {
        unsafe { self.dxgi_output_duplication.GetDesc() }
    }

    pub fn create_d3d11_texture2d(
        &self,
        d3d11_texture2d_desc: D3D11_TEXTURE2D_DESC,
    ) -> Result<ID3D11Texture2D, Error> {
        // create a readable texture in GPU memory
        let mut d3d11_texture2d: Option<ID3D11Texture2D> = None;
        unsafe {
            self.d3d11_device.CreateTexture2D(
                &d3d11_texture2d_desc,
                None,
                Some(&mut d3d11_texture2d),
            )
        }?;

        match d3d11_texture2d {
            Some(texture2d) => {
                // Lower priorities causes stuff to be needlessly copied from gpu to ram,
                // causing huge ram usage on some systems.
                // https://github.com/bryal/dxgcap-rs/blob/208d93368bc64aed783791242410459c878a10fb/src/lib.rs#L225
                unsafe { texture2d.SetEvictionPriority(DXGI_RESOURCE_PRIORITY_MAXIMUM.0) };

                Ok(texture2d)
            }
            None => Err(Error::from_win32()),
        }
    }

    pub fn desktop_image_data(
        &self,
        d3d11_texture2d: &ID3D11Texture2D,
        dxgi_outdupl_desc: &DXGI_OUTDUPL_DESC,
        locked_rect: &mut DXGI_MAPPED_RECT,
    ) -> Result<(), Error> {
        let d3d11_texture_desc = D3D11_TEXTURE2D_DESC {
            Width: dxgi_outdupl_desc.ModeDesc.Width,
            Height: dxgi_outdupl_desc.ModeDesc.Height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ArraySize: 1,
            BindFlags: 0,
            MiscFlags: 0,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MipLevels: 1,
        };

        let mut texture2d_stating: Option<ID3D11Texture2D> = None;
        unsafe {
            self.d3d11_device.CreateTexture2D(
                &d3d11_texture_desc,
                None,
                Some(&mut texture2d_stating),
            )?
        };

        let texture2d_stating = unsafe {
            texture2d_stating.as_ref().inspect(|&e| {
                self.d3d11_device_context.CopyResource(e, d3d11_texture2d);
            })
        };

        if let Some(texture2d_stating) = texture2d_stating {
            let dxgi_surface = texture2d_stating.cast::<IDXGISurface>()?;
            unsafe { dxgi_surface.Map(locked_rect, DXGI_MAP_READ) }?;
            unsafe { dxgi_surface.Unmap()? };
        }

        Ok(())
    }

    pub fn frame_pointer_shape_info(
        &self,
        pointer_shape_buffer_size: u32,
        pointer_shape_buffer: &mut [u8],
        pointer_shape_buffer_size_required: &mut u32,
        pointer_shape_info: &mut DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    ) -> Result<(), Error> {
        // let pointer_shape_buffer_size = dxgi_outdupl_frame_info.PointerShapeBufferSize;
        // let mut pointer_shape_buffer = vec![0u8; pointer_shape_buffer_size as usize];
        // let mut pointer_shape_buffer_size_required = 0;
        // let mut pointer_shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
        unsafe {
            self.dxgi_output_duplication.GetFramePointerShape(
                pointer_shape_buffer_size,
                pointer_shape_buffer.as_mut_ptr() as *mut c_void,
                pointer_shape_buffer_size_required,
                pointer_shape_info,
            )
        }?;

        Ok(())
    }

    //  Error { code: HRESULT(0x887A0001), message: "应用程序进行了无效的调用。调用的参数或某对象的状态不正确。\r\n启用 D3D 调试层以便通过调试消息查看详细信息。" }
    // Error { code: HRESULT(0x887A0001), message: "应用程序进行了无效的调用。调用的参数或某对象的状态不正确。\r\n启用 D3D 调试层以便通过调试消息查看详细信息。" }
    pub fn acquire_next_frame(
        &self,
        dxgi_outdupl_frame_info: &mut DXGI_OUTDUPL_FRAME_INFO,
        dxgi_resource: &mut Option<IDXGIResource>,
    ) -> Result<ID3D11Texture2D, Error> {
        let hr = unsafe {
            self.dxgi_output_duplication.AcquireNextFrame(
                self.timeout_ms,
                dxgi_outdupl_frame_info,
                dxgi_resource,
            )
        };

        match (hr, dxgi_resource) {
            (Ok(_), Some(resource)) => Ok((*resource).cast()?),
            (Err(e), _) => Err(e),
            (_, None) => Err(Error::from_win32()),
        }
    }

    pub fn capture(&self) -> Result<(), Error> {
        let dxgi_outdupl_desc = self.dxgi_outdupl_desc();
        let mut dxgi_outdupl_frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut dxgi_resource: Option<IDXGIResource> = None;
        let mut pointer_shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();

        for i in 0..2 {
            let mut pointer_shape_buffer = Vec::new();
            let (texture2d, dxgi_pointer_shape_info) = self.acquire_next_frame_with_cursor(
                &mut dxgi_outdupl_frame_info,
                &mut dxgi_resource,
                &mut pointer_shape_buffer,
            )?;

            if dxgi_pointer_shape_info.is_some() {
                pointer_shape_info =
                    dxgi_pointer_shape_info.expect("dxgi_pointer_shape_info is none!!!");
            }

            let d3d11_texture_desc = D3D11_TEXTURE2D_DESC {
                Width: dxgi_outdupl_desc.ModeDesc.Width,
                Height: dxgi_outdupl_desc.ModeDesc.Height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                ArraySize: 1,
                BindFlags: 0,
                MiscFlags: 0,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MipLevels: 1,
            };

            let mut texture2d_stating: Option<ID3D11Texture2D> = None;
            unsafe {
                self.d3d11_device.CreateTexture2D(
                    &d3d11_texture_desc,
                    None,
                    Some(&mut texture2d_stating),
                )?
            };

            let texture2d_stating = unsafe {
                texture2d_stating.as_ref().inspect(|&e| {
                    self.d3d11_device_context.CopyResource(e, &texture2d);
                })
            };

            let mut locked_rect = DXGI_MAPPED_RECT::default();

            if let Some(texture2d_stating) = texture2d_stating {
                let dxgi_surface = texture2d_stating.cast::<IDXGISurface>()?;
                unsafe { dxgi_surface.Map(&mut locked_rect, DXGI_MAP_READ) }?;
                unsafe { dxgi_surface.Unmap()? };
            }

            self.release_frame()?;

            let mut desktop_image_buffer = vec![
                0u8;
                (dxgi_outdupl_desc.ModeDesc.Width * dxgi_outdupl_desc.ModeDesc.Height * 4)
                    as usize
            ];

            if locked_rect.Pitch == dxgi_outdupl_desc.ModeDesc.Width as i32 {
                unsafe {
                    ptr::copy_nonoverlapping(
                        locked_rect.pBits,
                        desktop_image_buffer.as_mut_ptr() as *mut _,
                        desktop_image_buffer.len(),
                    )
                };
            } else {
                // TODO maybe the desktop_image_buffer size need change
                let dest = desktop_image_buffer.as_mut_ptr();
                let line_bytes = dxgi_outdupl_desc.ModeDesc.Width as usize * 4;
                for i in 0..dxgi_outdupl_desc.ModeDesc.Height {
                    let src = unsafe {
                        locked_rect
                            .pBits
                            .offset((i * locked_rect.Pitch as u32) as isize)
                    };

                    let dest = unsafe { dest.offset((i * line_bytes as u32) as isize) };
                    unsafe { ptr::copy_nonoverlapping(src, dest, locked_rect.Pitch as usize) };
                }
            }

            let dxgi_output_desc = self.dxgi_output_desc()?;
            println!("{:?}", dxgi_output_desc);
            println!("{:?}", pointer_shape_info);
            println!("{:?}", dxgi_outdupl_frame_info);

            if dxgi_outdupl_frame_info.PointerPosition.Visible.as_bool() {
                let desktop_image_buffer = draw_mouse(
                    pointer_shape_buffer,
                    dxgi_outdupl_frame_info,
                    pointer_shape_info,
                    dxgi_output_desc,
                    desktop_image_buffer,
                );
                rgba_to_bmp(
                    "filename.bmp",
                    &desktop_image_buffer[..],
                    desktop_image_buffer.len() as u32,
                    dxgi_outdupl_desc.ModeDesc.Width as i32,
                    dxgi_outdupl_desc.ModeDesc.Height as i32,
                );
                continue;
            }
            rgba_to_bmp(
                "filename.bmp",
                &desktop_image_buffer[..],
                desktop_image_buffer.len() as u32,
                dxgi_outdupl_desc.ModeDesc.Width as i32,
                dxgi_outdupl_desc.ModeDesc.Height as i32,
            );

        }

        Ok(())
    }

    pub fn acquire_next_frame_with_cursor(
        &self,
        dxgi_outdupl_frame_info: &mut DXGI_OUTDUPL_FRAME_INFO,
        dxgi_resource: &mut Option<IDXGIResource>,
        pointer_shape_buffer: &mut Vec<u8>,
    ) -> Result<(ID3D11Texture2D, Option<DXGI_OUTDUPL_POINTER_SHAPE_INFO>), Error> {
        unsafe {
            self.dxgi_output_duplication.AcquireNextFrame(
                self.timeout_ms,
                dxgi_outdupl_frame_info,
                dxgi_resource,
            )
        }?;

        // let mouse_position_updated = dxgi_outdupl_frame_info.LastMouseUpdateTime > 0;
        let shape_updated = dxgi_outdupl_frame_info.PointerShapeBufferSize > 0;

        let mut size: u32 = 0;
        let mut pointer_shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();

        let pointer_shape_buffer_size = dxgi_outdupl_frame_info.PointerShapeBufferSize as usize;
        if pointer_shape_buffer.len() < pointer_shape_buffer_size {
            pointer_shape_buffer.resize(pointer_shape_buffer_size, 0);
        }

        if shape_updated {
            unsafe {
                self.dxgi_output_duplication.GetFramePointerShape(
                    pointer_shape_buffer.len() as u32,
                    pointer_shape_buffer.as_mut_ptr() as *mut _,
                    &mut size,
                    &mut pointer_shape_info,
                )
            }?;

            // println!("{:?}", size);
            // println!("{:?}", pointer_shape_buffer.len());
            // println!("HHHHHHHHHHHHHHHHH");

            if let Some(resource) = dxgi_resource {
                return Ok(((*resource).cast()?, Some(pointer_shape_info)));
            }
        } else {
            if let Some(resource) = dxgi_resource {
                return Ok(((*resource).cast()?, None));
            }
        }

        Err(Error::from_win32())
    }

    pub fn capture_desktop_image_with_cursor(
        &self,
        dxgi_outdupl_frame_info: &mut DXGI_OUTDUPL_FRAME_INFO,
        dxgi_resource: &mut Option<IDXGIResource>,
        locked_rect: &mut DXGI_MAPPED_RECT,
        dxgi_outdupl_desc: &DXGI_OUTDUPL_DESC,
    ) -> Result<(), Error> {
        let d3d11_texture2d = self.acquire_next_frame(dxgi_outdupl_frame_info, dxgi_resource)?;

        if dxgi_outdupl_frame_info.PointerPosition.Visible.as_bool() {
            self.desktop_image_data(&d3d11_texture2d, dxgi_outdupl_desc, locked_rect)?;
        } else {
            self.capture_with_draw_cursor(dxgi_outdupl_desc, d3d11_texture2d, locked_rect)?;
        }
        self.release_frame()?;

        Ok(())
    }

    fn capture_with_draw_cursor(
        &self,
        dxgi_outdupl_desc: &DXGI_OUTDUPL_DESC,
        d3d11_texture2d: ID3D11Texture2D,
        locked_rect: &mut DXGI_MAPPED_RECT,
    ) -> Result<(), Error> {
        let mut d3d11_texture_desc = D3D11_TEXTURE2D_DESC {
            Width: dxgi_outdupl_desc.ModeDesc.Width,
            Height: dxgi_outdupl_desc.ModeDesc.Height,
            Format: dxgi_outdupl_desc.ModeDesc.Format,
            ArraySize: 1,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            MiscFlags: D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            CPUAccessFlags: DXGI_CPU_ACCESS_NONE,
            MipLevels: 1,
        };
        let mut d3d11_texture_gdi: Option<ID3D11Texture2D> = None;
        unsafe {
            self.d3d11_device.CreateTexture2D(
                &d3d11_texture_desc,
                None,
                Some(&mut d3d11_texture_gdi),
            )
        }?;

        let mut d3d11_texture_cpu: Option<ID3D11Texture2D> = None;
        d3d11_texture_desc.BindFlags = D3D11_BIND_FLAG::default().0 as u32;
        d3d11_texture_desc.MiscFlags = D3D11_RESOURCE_MISC_FLAG::default().0 as u32;
        d3d11_texture_desc.CPUAccessFlags =
            (D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE).0 as u32;
        d3d11_texture_desc.Usage = D3D11_USAGE_STAGING;
        unsafe {
            self.d3d11_device.CreateTexture2D(
                &d3d11_texture_desc,
                None,
                Some(&mut d3d11_texture_cpu),
            )
        }?;

        let d3d11_texture_gdi = unsafe {
            d3d11_texture_gdi.as_ref().inspect(|&e| {
                self.d3d11_device_context.CopyResource(e, &d3d11_texture2d);
            })
        };

        if let Some(d3d11_texture_gdi) = d3d11_texture_gdi {
            let dxgi_surface1 = d3d11_texture_gdi.cast::<IDXGISurface1>()?;
            let hdc = unsafe { dxgi_surface1.GetDC(false) }?;
            draw_mouse_with_dc(hdc)?;
            unsafe { dxgi_surface1.ReleaseDC(None)? };
            let d3d11_texture_cpu = unsafe {
                d3d11_texture_cpu
                    .as_ref()
                    .inspect(|&e| self.d3d11_device_context.CopyResource(e, d3d11_texture_gdi))
            };
            if let Some(d3d11_texture_cpu) = d3d11_texture_cpu {
                let dxgi_surface_cpu = d3d11_texture_cpu.cast::<IDXGISurface>()?;
                unsafe { dxgi_surface_cpu.Map(locked_rect, DXGI_MAP_READ) }?;
                unsafe { dxgi_surface_cpu.Unmap()? };
            }
        }

        Ok(())
    }

    pub fn capture_monitor(&self) -> Result<(), Error> {
        let dxgi_outdupl_desc = self.dxgi_outdupl_desc();
        let mut dxgi_outdupl_frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut dxgi_resource: Option<IDXGIResource> = None;
        let mut locked_rect = DXGI_MAPPED_RECT::default();

        self.capture_desktop_image_with_cursor(
            &mut dxgi_outdupl_frame_info,
            &mut dxgi_resource,
            &mut locked_rect,
            &dxgi_outdupl_desc,
        )?;

        let img_buffer_size =
            dxgi_outdupl_desc.ModeDesc.Width * dxgi_outdupl_desc.ModeDesc.Height * 4;
        let img_data =
            unsafe { slice::from_raw_parts(locked_rect.pBits, img_buffer_size as usize) };

        rgba_to_bmp(
            "screen.bmp",
            img_data,
            img_buffer_size,
            dxgi_outdupl_desc.ModeDesc.Width as i32,
            dxgi_outdupl_desc.ModeDesc.Height as i32,
        );

        Ok(())
    }

    fn release_frame(&self) -> Result<(), Error> {
        unsafe { self.dxgi_output_duplication.ReleaseFrame() }
    }
}

pub fn rgba_to_bmp(filename: &str, img_data: &[u8], img_buffer_size: u32, width: i32, height: i32) {
    let bf_off_bits = (size_of::<BITMAPFILEHEADER>() + size_of::<BITMAPINFOHEADER>()) as u32;

    let bfh = BITMAPFILEHEADER {
        bfType: 0x4D42,
        bfOffBits: bf_off_bits,
        bfSize: bf_off_bits + img_buffer_size,
        bfReserved1: 0,
        bfReserved2: 0,
    };

    let bih = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: img_buffer_size,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    // let data = unsafe { std::slice::from_raw_parts(img_data, img_size as usize) };

    let mut file = File::create(filename).unwrap();
    file.write_all(struct_to_bytes(&bfh)).unwrap();
    file.write_all(struct_to_bytes(&bih)).unwrap();
    file.write_all(img_data).unwrap();
}

fn struct_to_bytes<T>(s: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((s as *const T) as *const u8, std::mem::size_of::<T>()) }
}

pub fn draw_mouse_with_dc(hdc: HDC) -> Result<(), Error> {
    let mut cursor_info = CURSORINFO::default();
    let mut icon_info = ICONINFO::default();
    cursor_info.cbSize = size_of::<CURSORINFO>() as u32;

    if let Err(e) = unsafe { GetCursorInfo(&mut cursor_info) } {
        println!("GetCursorInfo failed: {:?}", e);
    }

    if (cursor_info.flags.0 & CURSOR_SHOWING.0) == 0 {
        println!("Cursor is not showing");
    }

    if let Err(e) = unsafe { GetIconInfo(cursor_info.hCursor, &mut icon_info) } {
        println!("GetIconInfo failed: {:?}", e);
    }

    if let Err(e) = unsafe {
        DrawIconEx(
            hdc,
            cursor_info.ptScreenPos.x,
            cursor_info.ptScreenPos.y,
            cursor_info.hCursor,
            0,
            0,
            0,
            HBRUSH::default(),
            DI_NORMAL | DI_DEFAULTSIZE,
        )
    } {
        println!("DrawIconEx failed: {:?}", e);
        if unsafe { DeleteObject(icon_info.hbmColor) }.as_bool().not()
            || unsafe { DeleteObject(icon_info.hbmMask) }.as_bool().not()
        {
            println!("DeleteObject failed");
        }
    }

    Ok(())
}

pub fn draw_mouse(
    pointer_shape_buffer: Vec<u8>,
    frame_info: DXGI_OUTDUPL_FRAME_INFO,
    pointer_shape_info: DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    dxgi_output_desc: DXGI_OUTPUT_DESC,
    buf: Vec<u8>,
) -> Vec<u8> {
    let desktop_width = width();
    let desktop_height = height();

    // let cursor_width = if frame_info.PointerPosition.Position.x < 0 {
    //     frame_info.PointerPosition.Position.x + pointer_shape_info.Width as i32
    // } else if frame_info.PointerPosition.Position.x + pointer_shape_info.Width as i32
    //     > desktop_width
    // {
    //     desktop_width - pointer_shape_info.Width as i32
    // } else {
    //     pointer_shape_info.Width as i32
    // };

    // let mut cursor_height = if frame_info.PointerPosition.Position.y < 0 {
    //     frame_info.PointerPosition.Position.y + pointer_shape_info.Height as i32
    // } else if frame_info.PointerPosition.Position.y + pointer_shape_info.Height as i32
    //     > desktop_height
    // {
    //     desktop_height - pointer_shape_info.Height as i32
    // } else {
    //     pointer_shape_info.Height as i32
    // };

    let mut cursor_height = pointer_shape_info.Height as i32;
    let cursor_width = pointer_shape_info.Width as i32;

    println!("{} {}", cursor_width, cursor_height);

    let cursor_left = if frame_info.PointerPosition.Position.x < 0 {
        0
    } else {
        frame_info.PointerPosition.Position.x
    };

    let cursor_top = if frame_info.PointerPosition.Position.y < 0 {
        0
    } else {
        frame_info.PointerPosition.Position.y
    };

    let skip_x = if cursor_left < 0 { -cursor_left } else { 0 };

    let skip_y = if cursor_top < 0 { -cursor_top } else { 0 };

    match pointer_shape_info.Type {
        val if val == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 as u32 => {
            let mut buf32 = vec8_to_vec32(buf);
            cursor_height /= 2;
            for row in 0..cursor_height {
                let mut mask = 0x80 >> (skip_x % 8);
                for col in 0..cursor_width {
                    let and_mask = pointer_shape_buffer[((col + skip_x) / 8
                        + (row + skip_y) * pointer_shape_info.Pitch as i32)
                        as usize]
                        & mask;

                    let xor_mask = pointer_shape_buffer[((col + skip_x) / 8
                        + (row + skip_y + cursor_height) * pointer_shape_info.Pitch as i32)
                        as usize]
                        & mask;

                    let and_mask32 = if and_mask != 0 {
                        0xFFFFFFFFu32
                    } else {
                        0xFF000000u32
                    };

                    let xor_mask32 = if xor_mask != 0 {
                        0x00FFFFFF
                    } else {
                        0x00000000
                    };

                    buf32
                        [((cursor_top + row) * desktop_width + cursor_left + col) as usize + 100] =
                        buf32[(row * (pointer_shape_info.Pitch as i32 / 4) + col) as usize]
                            & and_mask32
                            ^ xor_mask32;

                    mask = if mask == 0x01 { 0x80 } else { mask >> 1 };
                }
            }

            return vec32_to_vec8(buf32);
        }
        val if val == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR.0 as u32 => {
            let cursor32 = vec8_to_vec32(pointer_shape_buffer);
            let mut buf32 = vec8_to_vec32(buf);

            for row in 0..cursor_height {
                for col in 0..cursor_width {
                    // println!("*************");
                    // println!("{} {}", row, skip_x);
                    // println!("{} {}", col, cursor_width);
                    // println!("{} {}", cursor_width, cursor_height);

                    let cur_cursor_val = cursor32[(col
                        + skip_x
                        + (row + skip_y) * (pointer_shape_info.Pitch as i32 / 4))
                        as usize];

                    if cur_cursor_val == 0x00000000 {
                        continue;
                    }

                    buf32[((cursor_top + row) * desktop_width + cursor_left + col) as usize] =
                        cur_cursor_val;
                }
            }

            return vec32_to_vec8(buf32);
        }
        val if val == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR.0 as u32 => {
            let cursor32 = vec8_to_vec32(pointer_shape_buffer);
            let mut buf32 = vec8_to_vec32(buf);
            for row in 0..cursor_height {
                for col in 0..cursor_width {
                    let mask_val = 0xFF000000
                        & cursor32[(col
                            + skip_x
                            + (row + skip_y) * (pointer_shape_info.Pitch as i32 / 4))
                            as usize];

                    let cur_cursor_val = cursor32[(col
                        + skip_x
                        + (row + skip_y) * (pointer_shape_info.Pitch as i32 / 4))
                        as usize];
                    if mask_val != 0 {
                        buf32[((cursor_top + row) * desktop_width + cursor_left + col) as usize] =
                            buf32
                                [((cursor_top + row) * desktop_width + cursor_left + col) as usize]
                                ^ cur_cursor_val
                                | 0xFF000000;
                    } else {
                        buf32[((cursor_top + row) * desktop_width + cursor_left + col) as usize] =
                            cur_cursor_val | 0xFF000000;
                    }
                }
            }

            return vec32_to_vec8(buf32);
        }
        _ => {
            return Vec::new();
        }
    }
}

pub fn vec32_to_vec8(src_buf: Vec<u32>) -> Vec<u8> {
    let buffer: Vec<u8> = unsafe {
        Vec::from_raw_parts(
            src_buf.as_ptr() as *mut u8,
            src_buf.len() * 4,
            src_buf.capacity() * 4,
        )
    };
    std::mem::forget(src_buf);
    buffer
}

pub fn vec8_to_vec32(src_buf: Vec<u8>) -> Vec<u32> {
    let buffer: Vec<u32> = unsafe {
        Vec::from_raw_parts(
            src_buf.as_ptr() as *mut u32,
            src_buf.len() / 4,
            src_buf.capacity() / 4,
        )
    };
    std::mem::forget(src_buf);
    buffer
}

pub fn width() -> i32 {
    unsafe { GetSystemMetrics(SM_CXSCREEN) }
}

pub fn height() -> i32 {
    unsafe { GetSystemMetrics(SM_CYSCREEN) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    #[test]
    fn test_dxgi_screenshot() {
        let dxgi_adapter1 = adapter1_by_id(0).unwrap();
        let dxgi_output1 = dxgi_output1_by_id_and_adapter1(0, &dxgi_adapter1).unwrap();
        let (d3d11_device, d3d11_device_context) = dxgi_device_and_dxgi_device_context().unwrap();
        let dxgi_output_duplication =
            dxgi_output_duplication_by_output1(&d3d11_device, &dxgi_output1).unwrap();
        let duplication_context = DuplicationContext::new(
            d3d11_device,
            d3d11_device_context,
            1000,
            dxgi_output1,
            dxgi_output_duplication,
        );

        thread::spawn(move || {
            for i in 1.. {
                // let _dxgi_mapped_rect = duplication_context.capture_desktop_image().unwrap();
                duplication_context.capture_monitor().unwrap();

                println!("frame: {}", i);
            }
        });

        sleep(Duration::from_secs(1));
    }
}
