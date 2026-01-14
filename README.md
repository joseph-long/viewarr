# viewarr

A browser-based array/image viewer built with Rust, WebAssembly, and egui.

## Features

- Renders numeric arrays as grayscale images with auto-scaling
- Shows original pixel values on hover
- Supports multiple independent viewer instances per page
- Accepts all JavaScript TypedArray types (Int8, Uint8, Int16, Uint16, Int32, Uint32, BigInt64, BigUint64, Float32, Float64)
- Clean vanilla JS API with integration points for reactive frameworks

## Building

### Prerequisites

- [Rust](https://rustup.rs/) with wasm32-unknown-unknown target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

### Build

```bash
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

### Supported Data Types

| dtype | JavaScript Type | Description |
|-------|-----------------|-------------|
| `i1`, `b` | Int8Array | 8-bit signed integer |
| `u1`, `B` | Uint8Array | 8-bit unsigned integer |
| `i2` | Int16Array | 16-bit signed integer |
| `u2` | Uint16Array | 16-bit unsigned integer |
| `i4` | Int32Array | 32-bit signed integer |
| `u4` | Uint32Array | 32-bit unsigned integer |
| `i8` | BigInt64Array | 64-bit signed integer |
| `u8` | BigUint64Array | 64-bit unsigned integer |
| `f4` | Float32Array | 32-bit float |
| `f8` | Float64Array | 64-bit float (default) |

### Container Requirements

- The container element **must have an ID** - this ID is used to identify the viewer instance
- The container should have defined dimensions (width and height)
- A `ResizeObserver` is automatically attached to handle dynamic resizing

### Multiple Viewers

Each container ID creates an independent viewer with its own state:

```javascript
await createViewer('viewer-1');
await createViewer('viewer-2');

setImageData('viewer-1', buffer1, 100, 100, 'f4');
setImageData('viewer-2', buffer2, 200, 200, 'f8');
```

## Integration with JupyterLab

This package is designed to be used as the image viewer backend for [jupyterlab-fitsview](https://github.com/yourusername/jupyterlab-fitsview).

## License

MIT
