#include <iostream>

#include <d3d11.h>
#include <dxgi.h>
#include <dxgi1_2.h>
#include <synchapi.h>
#include <vector>
#include <winerror.h>
#include <winnt.h>
#include <winuser.h>

#pragma comment(lib, "D3D11.lib")

struct Image {
  std::vector<byte> bytes;
  int width = 0;
  int height = 0;
  int rowPitch = 0;
};

// 获取鼠标信息
bool GetMouseInfo(CURSORINFO &cursorInfo, ICONINFO &iconInfo) {
  cursorInfo.cbSize = sizeof(CURSORINFO);
  if (!GetCursorInfo(&cursorInfo)) {
    return false;
  }
  if (!(cursorInfo.flags & CURSOR_SHOWING)) {
    return false;
  }
  if (!GetIconInfo(cursorInfo.hCursor, &iconInfo)) {
    return false;
  }
  return true;
}

// 绘制鼠标指针
void DrawMousePointer(ID3D11DeviceContext *pContext,
                      D3D11_MAPPED_SUBRESOURCE &res, ID3D11Texture2D *pTexture,
                      const CURSORINFO &cursorInfo, const ICONINFO &iconInfo) {
  // 获取鼠标指针位图数据
  HDC hdc = CreateCompatibleDC(NULL);
  HBITMAP hBitmap = iconInfo.hbmColor ? iconInfo.hbmColor : iconInfo.hbmMask;
  if (!hBitmap) {
    std::cout<<"hBitmap!!!";
    return;
  }
  
  SelectObject(hdc, hBitmap);

  BITMAP bm;
  GetObject(hBitmap, sizeof(BITMAP), &bm);

  // 锁定纹理
  pContext->Map(pTexture, 0, D3D11_MAP_READ, 0, &res);

  // 获取位图数据
  BITMAPINFO bmi = {0};
  bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  bmi.bmiHeader.biWidth = bm.bmWidth;
  bmi.bmiHeader.biHeight = -bm.bmHeight; // 负值表示自上而下的位图
  bmi.bmiHeader.biPlanes = 1;
  bmi.bmiHeader.biBitCount = 32;
  bmi.bmiHeader.biCompression = BI_RGB;

  std::vector<BYTE> bitmapData(bm.bmWidth * bm.bmHeight * 4);
  GetDIBits(hdc, hBitmap, 0, bm.bmHeight, bitmapData.data(), &bmi,
            DIB_RGB_COLORS);

  // 绘制鼠标指针到纹理
  int x = cursorInfo.ptScreenPos.x - iconInfo.xHotspot;
  int y = cursorInfo.ptScreenPos.y - iconInfo.yHotspot;
  BYTE *dest = static_cast<BYTE *>(res.pData);
  for (int row = 0; row < bm.bmHeight; ++row) {
    for (int col = 0; col < bm.bmWidth; ++col) {
      BYTE *srcPixel = bitmapData.data() + (row * bm.bmWidth + col) * 4;
      BYTE *destPixel = dest + (y + row) * res.RowPitch + (x + col) * 4;

      // 处理透明度
      if (srcPixel[3] != 0) { // 如果 alpha 通道不为 0，表示不透明
        memcpy(destPixel, srcPixel, 4);
      }
    }
  }

  // 解锁纹理
  pContext->Unmap(pTexture, 0);

  DeleteDC(hdc);
  DeleteObject(hBitmap);
}

// 将RGB格式图片转成bmp格式
void RGBDataSaveAsBmpFile(const char *bmpFile,     // BMP文件名称
                          unsigned char *pRgbData, // 图像数据
                          int width,               // 图像宽度
                          int height,              // 图像高度
                          int biBitCount,          // 位图深度
                          bool flipvertical) // 图像是否需要垂直翻转
{
  int size = 0;
  int bitsPerPixel = 3;
  if (biBitCount == 24) {
    bitsPerPixel = 3;
    size = width * height * bitsPerPixel * sizeof(char); // 每个像素点3个字节
  } else if (biBitCount == 32) {
    bitsPerPixel = 4;
    size = width * height * bitsPerPixel * sizeof(char); // 每个像素点4个字节
  } else
    return;

  // 位图第一部分，文件信息
  BITMAPFILEHEADER bfh;
  bfh.bfType = (WORD)0x4d42; // 图像格式 必须为'BM'格式
  bfh.bfOffBits =
      sizeof(BITMAPFILEHEADER) + sizeof(BITMAPINFOHEADER); // 真正的数据的位置
  bfh.bfSize = size + bfh.bfOffBits;
  bfh.bfReserved1 = 0;
  bfh.bfReserved2 = 0;

  BITMAPINFOHEADER bih;
  bih.biSize = sizeof(BITMAPINFOHEADER);
  bih.biWidth = width;
  if (flipvertical)
    bih.biHeight =
        -height; // BMP图片从最后一个点开始扫描，显示时图片是倒着的，所以用-height，这样图片就正了
  else
    bih.biHeight = height;
  bih.biPlanes = 1;
  bih.biBitCount = biBitCount;
  bih.biCompression = BI_RGB;
  bih.biSizeImage = size;
  bih.biXPelsPerMeter = 0;
  bih.biYPelsPerMeter = 0;
  bih.biClrUsed = 0;
  bih.biClrImportant = 0;
  FILE *fp = NULL;
  fopen_s(&fp, bmpFile, "wb");
  if (!fp)
    return;

  fwrite(&bfh, 8, 1, fp);
  fwrite(&bfh.bfReserved2, sizeof(bfh.bfReserved2), 1, fp);
  fwrite(&bfh.bfOffBits, sizeof(bfh.bfOffBits), 1, fp);
  fwrite(&bih, sizeof(BITMAPINFOHEADER), 1, fp);
  fwrite(pRgbData, size, 1, fp);
  fclose(fp);
}

void PrintDXGIOutputLDesc(const DXGI_OUTDUPL_DESC &desc) {
  std::cout << "ModeDesc.Width: " << desc.ModeDesc.Width << std::endl;
  std::cout << "ModeDesc.Height: " << desc.ModeDesc.Height << std::endl;
  std::cout << "ModeDesc.RefreshRate.Numerator: "
            << desc.ModeDesc.RefreshRate.Numerator << std::endl;
  std::cout << "ModeDesc.RefreshRate.Denominator: "
            << desc.ModeDesc.RefreshRate.Denominator << std::endl;
  std::cout << "ModeDesc.Format: " << desc.ModeDesc.Format << std::endl;
  std::cout << "ModeDesc.ScanlineOrdering: " << desc.ModeDesc.ScanlineOrdering
            << std::endl;
  std::cout << "ModeDesc.Scaling: " << desc.ModeDesc.Scaling << std::endl;
  std::cout << "Rotation: " << desc.Rotation << std::endl;
  std::cout << "DesktopImageInSystemMemory: " << desc.DesktopImageInSystemMemory
            << std::endl;
}

void PrintDXGIOutputDesc(const DXGI_OUTPUT_DESC &desc) {
  std::wcout << L"Device Name: " << desc.DeviceName << std::endl;
  std::wcout << L"Desktop Coordinates: (" << desc.DesktopCoordinates.left
             << L", " << desc.DesktopCoordinates.top << L") - ("
             << desc.DesktopCoordinates.right << L", "
             << desc.DesktopCoordinates.bottom << L")" << std::endl;
  std::wcout << L"Attached to Desktop: "
             << (desc.AttachedToDesktop ? L"Yes" : L"No") << std::endl;
  std::wcout << L"Rotation: " << desc.Rotation << std::endl;
  std::wcout << L"Monitor Handle: " << desc.Monitor << std::endl;
}

void PrintD3D11Texture2DDesc(const D3D11_TEXTURE2D_DESC &desc) {
  std::cout << "Width: " << desc.Width << std::endl;
  std::cout << "Height: " << desc.Height << std::endl;
  std::cout << "MipLevels: " << desc.MipLevels << std::endl;
  std::cout << "ArraySize: " << desc.ArraySize << std::endl;
  std::cout << "Format: " << desc.Format << std::endl;
  std::cout << "SampleDesc.Count: " << desc.SampleDesc.Count << std::endl;
  std::cout << "SampleDesc.Quality: " << desc.SampleDesc.Quality << std::endl;
  std::cout << "Usage: " << desc.Usage << std::endl;
  std::cout << "BindFlags: " << desc.BindFlags << std::endl;
  std::cout << "CPUAccessFlags: " << desc.CPUAccessFlags << std::endl;
  std::cout << "MiscFlags: " << desc.MiscFlags << std::endl;
}

void PrintD3D11MappedSubresource(
    const D3D11_MAPPED_SUBRESOURCE &mappedResource) {
  std::cout << "pData: " << mappedResource.pData << std::endl;
  std::cout << "RowPitch: " << mappedResource.RowPitch << std::endl;
  std::cout << "DepthPitch: " << mappedResource.DepthPitch << std::endl;
}

void PrintMappedRect(const DXGI_MAPPED_RECT &mappedRect) {
  std::cout << "Pitch: " << mappedRect.Pitch << std::endl;
  std::cout << "pBits: " << static_cast<void *>(mappedRect.pBits) << std::endl;
}

void PrintFrameInfo(const DXGI_OUTDUPL_FRAME_INFO &frameInfo) {
  std::wcout << L"LastPresentTime: " << frameInfo.LastPresentTime.QuadPart
             << std::endl;
  std::wcout << L"LastMouseUpdateTime: "
             << frameInfo.LastMouseUpdateTime.QuadPart << std::endl;
  std::wcout << L"AccumulatedFrames: " << frameInfo.AccumulatedFrames
             << std::endl;
  std::wcout << L"RectsCoalesced: "
             << (frameInfo.RectsCoalesced ? L"TRUE" : L"FALSE") << std::endl;
  std::wcout << L"ProtectedContentMaskedOut: "
             << (frameInfo.ProtectedContentMaskedOut ? L"TRUE" : L"FALSE")
             << std::endl;
  std::wcout << L"PointerPosition: (" << frameInfo.PointerPosition.Position.x
             << L", " << frameInfo.PointerPosition.Position.y << L") "
             << "PointerPosition Visible:"
             << (frameInfo.PointerPosition.Visible ? L"TRUE" : L"FALSE")
             << std::endl;
  std::wcout << L"PointerShapeBufferSize: " << frameInfo.PointerShapeBufferSize
             << std::endl;
  std::wcout << L"TotalMetadataBufferSize: "
             << frameInfo.TotalMetadataBufferSize << std::endl;
}

int main() {

  HRESULT hr;

  ID3D11Device *p_d3dDevice = nullptr;
  IDXGIDevice *p_dxgiDevice = nullptr;
  ID3D11DeviceContext *p_d3dDeviceContext = nullptr;
  D3D_FEATURE_LEVEL featureLevel;

  static const D3D_DRIVER_TYPE driverTypes[] = {D3D_DRIVER_TYPE_HARDWARE,
                                                D3D_DRIVER_TYPE_WARP,
                                                D3D_DRIVER_TYPE_REFERENCE};

  static const D3D_FEATURE_LEVEL featureLevels[] = {
      D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0,
      D3D_FEATURE_LEVEL_9_1};

  for (const auto &driverType : driverTypes) {
    const auto hr = D3D11CreateDevice(
        nullptr, driverType, nullptr, 0, featureLevels,
        static_cast<UINT>(std::size(featureLevels)), D3D11_SDK_VERSION,
        &p_d3dDevice, &featureLevel, &p_d3dDeviceContext);
    if (SUCCEEDED(hr)) {
      break;
    }
    p_d3dDevice->Release();
    p_d3dDeviceContext->Release();
  }

  // hr = D3D11CreateDevice(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, NULL, NULL,
  //                        D3D11_SDK_VERSION, &p_d3dDevice, NULL,
  //                        &p_d3dDeviceContext);

  // if (FAILED(hr)) {
  //   std::cout << "D3D11CreateDevice failed!!!\n";
  //   return hr;
  // }

  hr = p_d3dDevice->QueryInterface(IID_PPV_ARGS(&p_dxgiDevice));
  if (FAILED(hr)) {
    std::cout << "p_d3dDevice->QueryInterface failed!!!\n";
    return hr;
  }

  IDXGIAdapter *p_dxgiAdapter = NULL;
  hr = p_dxgiDevice->GetParent(IID_PPV_ARGS(&p_dxgiAdapter));
  if (FAILED(hr)) {
    std::cout << "p_dxgiDevice->GetParent failed!!!\n";
    return hr;
  }

  IDXGIOutput *p_dxgiOutput = nullptr;
  hr = p_dxgiAdapter->EnumOutputs(0, &p_dxgiOutput);
  if (FAILED(hr)) {
    std::cout << "p_dxgiAdapter->EnumOutputs failed!!!\n";
    return hr;
  }

  DXGI_OUTPUT_DESC m_dxgiOutputDesc;
  hr = p_dxgiOutput->GetDesc(&m_dxgiOutputDesc);
  if (FAILED(hr)) {
    std::cout << "p_dxgiOutput->GetDesc failed!!!\n";
    return hr;
  }

  PrintDXGIOutputDesc(m_dxgiOutputDesc);
  std::cout << "***********************************\n";

  IDXGIOutput1 *p_dxgiOutput1 = nullptr;
  hr = p_dxgiOutput->QueryInterface(IID_PPV_ARGS(&p_dxgiOutput1));
  if (FAILED(hr)) {
    std::cout << "p_dxgiOutput->QueryInterface failed!!!\n";
    return hr;
  }

  IDXGIOutputDuplication *p_dxgiOutputDup = nullptr;
  hr = p_dxgiOutput1->DuplicateOutput(p_d3dDevice, &p_dxgiOutputDup);
  if (FAILED(hr)) {
    std::cout << "p_dxgiOutput1->DuplicateOutput failed!!!\n";
    return hr;
  }

  DXGI_OUTDUPL_DESC m_dxgiOutputLDesc;
  p_dxgiOutputDup->GetDesc(&m_dxgiOutputLDesc);

  PrintDXGIOutputLDesc(m_dxgiOutputLDesc);
  std::cout << "***********************************\n";

  IDXGIResource *p_desktopRes = nullptr;

  DXGI_OUTDUPL_FRAME_INFO frame_info;
  for (int i = 0; i < 2; ++i) {
    hr = p_dxgiOutputDup->AcquireNextFrame(1, &frame_info, &p_desktopRes);
    if (SUCCEEDED(hr) && (frame_info.LastPresentTime.QuadPart == 0)) {
      p_desktopRes->Release();
      p_dxgiOutputDup->ReleaseFrame();
      Sleep(1);
      std::cout << "p_desktopRes->Release and p_dxgiOutputDup->ReleaseFrame \n";
    }
  }

  PrintFrameInfo(frame_info);
  std::cout << "***********************************\n";

  ID3D11Texture2D *p_texture2d = nullptr;
  p_desktopRes->QueryInterface(IID_PPV_ARGS(&p_texture2d));

  ID3D11Texture2D *p_texture2dBuf;
  D3D11_TEXTURE2D_DESC copyBufferDesc;
  copyBufferDesc.Width = m_dxgiOutputLDesc.ModeDesc.Width;
  copyBufferDesc.Height = m_dxgiOutputLDesc.ModeDesc.Height;
  copyBufferDesc.MipLevels = 1;
  copyBufferDesc.ArraySize = 1;
  copyBufferDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  copyBufferDesc.SampleDesc.Count = 1;
  copyBufferDesc.SampleDesc.Quality = 0;
  copyBufferDesc.Usage = D3D11_USAGE_STAGING;
  copyBufferDesc.BindFlags = 0;
  copyBufferDesc.CPUAccessFlags =
      D3D11_CPU_ACCESS_READ;
  copyBufferDesc.MiscFlags = 0;

  hr = p_d3dDevice->CreateTexture2D(&copyBufferDesc, nullptr, &p_texture2dBuf);
  if (FAILED(hr)) {
    std::cout << "p_d3dDevice->CreateTexture2D error!!!";
    return 1;
  }

  PrintD3D11Texture2DDesc(copyBufferDesc);
  std::cout << "***********************************\n";

  if (!p_texture2dBuf) {
    std::cout << "p_texture2dBuf is null!!!\n";
  }

  if (!p_d3dDeviceContext) {
    std::cout << "p_d3dDeviceContext is null !!!\n";
  }

  p_d3dDeviceContext->CopyResource(p_texture2dBuf, p_texture2d);

  if (FAILED(hr)) {
    std::cout << "p_texture2d->QueryInterface error!!!";
  }

  // IDXGISurface1 *copySurface = nullptr;
  // hr = p_texture2dBuf->QueryInterface(IID_PPV_ARGS(&copySurface));
  // if (FAILED(hr)) {
  //   std::cout << "p_texture2d->QueryInterface error!!!";
  // }

  // DXGI_MAPPED_RECT mappedRect;
  // hr = copySurface->Map(&mappedRect, DXGI_MAP_READ);
  // if (FAILED(hr)) {
  //   std::cout << "copySurface->Map error!!!";
  // }

  // IDXGISurface *CopySurface = nullptr;
  // p_texture2dBuf->QueryInterface(IID_PPV_ARGS(&CopySurface));

  // DXGI_MAPPED_RECT MappedSurface;
  // CopySurface->Map(&MappedSurface, DXGI_MAP_READ);

  D3D11_MAPPED_SUBRESOURCE res;
  CURSORINFO cursorInfo;
  ICONINFO iconInfo;
  if (GetMouseInfo(cursorInfo, iconInfo)) {
    std::cout << "Print Cursor!!!\n";
    DrawMousePointer(p_d3dDeviceContext, res, p_texture2dBuf, cursorInfo,
                     iconInfo);
  }


  // PrintMappedRect(mappedRect);
  // copySurface->Unmap();
  // RGBDataSaveAsBmpFile("ScreenShot.bmp", mappedRect.pBits, 1920, 1080, 32,
  //                      true);

  Image image;
  image.width = m_dxgiOutputLDesc.ModeDesc.Width;
  image.height = m_dxgiOutputLDesc.ModeDesc.Height;
  image.rowPitch = res.RowPitch;

  std::cout << image.bytes.size() << " " << image.rowPitch * image.height;

  image.bytes.resize(1000);
  image.bytes.resize(image.rowPitch * image.height);
  memcpy(image.bytes.data(), res.pData, image.bytes.size());
  p_d3dDeviceContext->Unmap(p_texture2d, 0);
  if (!image.bytes.empty()) {
    const char *filename = "screenshot.ppm";
    FILE *fp;
    if (fopen_s(&fp, filename, "wb") == 0) {
      // PPM format: https://en.wikipedia.org/wiki/Netpbm
      fprintf(fp, "P6\n#\n%d %d %d\n", image.width, image.height, 255);
      for (int y = 0; y < image.height; ++y) {
        for (int x = 0; x < image.width; ++x) {
          const auto *p = image.bytes.data() + (image.rowPitch * y) + (4 * x);
          fputc(p[2], fp); // R
          fputc(p[1], fp); // G
          fputc(p[0], fp); // B
        }
      }
      fclose(fp);
    }
  }
}
