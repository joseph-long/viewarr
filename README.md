# viewarr

An array/image viewer built with Rust, WebAssembly, and egui.

## Features

- Re-stretches images with linear, log, or symmetric linear scales
- Adjusts contrast and bias interactively by right-clicking and dragging
- Shows original pixel values on hover
- Supports multiple independent viewer instances per page
- Accepts all JavaScript TypedArray types (Int8, Uint8, Int16, Uint16, Int32, Uint32, BigInt64, BigUint64, Float32, Float64)
- Clean vanilla JS API with integration points for reactive frameworks

## Building

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

### Build

```bash
# Install deps
npm install

# Build the WASM module and JS wrapper
npm run build

# Or for development (faster, larger, with debug symbols)
npm run build:dev
```

The built package will be in `pkg/`.

## Usage

### Installation

For development with a local copy:

```json
{
  "dependencies": {
    "viewarr": "file:../viewarr/pkg"
  }
}
```

### JavaScript API

```javascript
import { createViewer, setImageData, destroyViewer } from 'viewarr';

// Create a viewer in a container element
// The container must have an ID
await createViewer('my-container-id');

// Load image data
// buffer: ArrayBuffer with raw pixel data
// width, height: image dimensions
// dtype: numpy dtype string ("f4", "f8", "i2", "u1", etc.)
setImageData('my-container-id', buffer, width, height, dtype);

// Clean up when done
destroyViewer('my-container-id');
```

### Container Requirements

- The container element **must have an ID** - this ID is used to identify the viewer instance
- The container should have defined dimensions (width and height)
- A `ResizeObserver` is automatically attached to handle dynamic resizing

### Multiple Viewers

Each container ID creates an independent viewer with its own state:

```javascript
await createViewer('viewer-1');
await createViewer('viewer-2');

setImageData('viewer-1', buffer1, 100, 100, 'u16');
setImageData('viewer-2', buffer2, 200, 200, 'f64');
```

## Integration with JupyterLab

This package is designed to be used as the image viewer backend for [jupyterlab-fitsview](https://github.com/joseph-long/jupyterlab-fitsview). It can also be embedded as a widget within a notebook using [pyviewarr](https://github.com/joseph-long/pyviewarr).

## License

MIT
