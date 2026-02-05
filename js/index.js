/**
 * viewarr - Browser-based array/image viewer
 *
 * This module provides a JavaScript API for managing multiple viewer instances,
 * each backed by a Rust/WASM/egui renderer.
 */

// Module-level state
let wasmModule = null;
let wasmInitPromise = null;
const viewers = new Map();

/**
 * Initialize the WASM module (called automatically, idempotent)
 * @returns {Promise<void>}
 */
async function initWasm() {
  if (wasmInitPromise) {
    return wasmInitPromise;
  }

  wasmInitPromise = (async () => {
    // Dynamic import of the wasm-pack generated module
    // When installed as an NPM package, pkg files are in the package root
    const wasm = await import('./viewarr.js');
    // Initialize the WASM module
    await wasm.default();
    wasmModule = wasm;
  })();

  return wasmInitPromise;
}

/**
 * Create a new viewer instance in the specified container.
 *
 * @param {string} containerId - The ID of the HTML element to use as the container.
 *                               This ID is also used to identify the viewer instance.
 * @returns {Promise<void>} Resolves when the viewer is ready.
 * @throws {Error} If the container is not found or initialization fails.
 */
export async function createViewer(containerId) {
  const container = document.getElementById(containerId);
  if (!container) {
    throw new Error(`Container element with id "${containerId}" not found`);
  }

  // Check if viewer already exists for this container
  if (viewers.has(containerId)) {
    console.warn(`Viewer already exists for container "${containerId}"`);
    return;
  }

  // Show loading indicator
  container.innerHTML = '';
  const loadingDiv = document.createElement('div');
  loadingDiv.style.cssText = `
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    font-family: system-ui, -apple-system, sans-serif;
    color: #666;
  `;
  loadingDiv.textContent = 'Loading viewer...';
  container.appendChild(loadingDiv);

  try {
    // Initialize WASM module (idempotent)
    await initWasm();

    // Create canvas element
    const canvas = document.createElement('canvas');
    canvas.id = `${containerId}_canvas`;
    canvas.style.cssText = `
      width: 100%;
      height: 100%;
      display: block;
    `;

    // Prevent native browser drag behavior on the canvas
    // This stops the browser from trying to drag the canvas content as an image
    canvas.addEventListener('dragstart', (e) => {
      e.preventDefault();
    });
    canvas.draggable = false;

    // Also prevent default on mousedown with alt key to avoid browser-specific behaviors
    canvas.addEventListener('mousedown', (e) => {
      if (e.altKey) {
        e.preventDefault();
      }
    });

    // Replace loading indicator with canvas
    container.innerHTML = '';
    container.appendChild(canvas);

    // Create the viewer handle using static factory method
    const handle = await wasmModule.ViewerHandle.create(canvas);

    // Store viewer state
    viewers.set(containerId, {
      handle,
      canvas,
      container
    });

    // Set up MutationObserver to detect container removal (e.g., tab close)
    const mutationObserver = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.removedNodes.forEach((node) => {
          if (node === container || node.contains(container)) {
            console.debug(`Cleaning up viewer ${containerId} because its DOM node went away...`);
            destroyViewer(containerId);
            console.debug(`Done cleaning up viewer ${containerId}.`);
            mutationObserver.disconnect();
          }
        });
      });
    });
    mutationObserver.observe(document.body, { childList: true, subtree: true });
    console.log("Installed mutation observer");

    // Update viewer state to include the observer
    viewers.get(containerId).mutationObserver = mutationObserver;

  } catch (error) {
    // Show error in container
    container.innerHTML = '';
    const errorDiv = document.createElement('div');
    errorDiv.style.cssText = `
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      width: 100%;
      height: 100%;
      font-family: system-ui, -apple-system, sans-serif;
      color: #c00;
      padding: 20px;
      box-sizing: border-box;
      text-align: center;
    `;

    const title = document.createElement('div');
    title.style.fontWeight = 'bold';
    title.style.marginBottom = '10px';
    title.textContent = 'Failed to load viewer';

    const message = document.createElement('div');
    message.style.cssText = `
      font-family: monospace;
      font-size: 12px;
      white-space: pre-wrap;
      word-break: break-word;
      max-width: 100%;
    `;
    message.textContent = error.message || String(error);

    errorDiv.appendChild(title);
    errorDiv.appendChild(message);
    container.appendChild(errorDiv);

    // Log full error to console
    console.error('viewarr initialization failed:', error);

    throw error;
  }
}

/**
 * Set image data for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {ArrayBuffer} buffer - The raw pixel data.
 * @param {number} width - Image width in pixels.
 * @param {number} height - Image height in pixels.
 * @param {string} dtype - Data type string (e.g., "f4", "f8", "i2", "u1").
 * @throws {Error} If the viewer is not found or data is invalid.
 */
export function setImageData(containerId, buffer, width, height, dtype) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }

  viewer.handle.setImageData(buffer, width, height, dtype);
}

/**
 * Destroy a viewer instance and clean up resources.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 */
export function destroyViewer(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    return; // Already destroyed or never created
  }
  viewer.handle.destroy();

  // Stop observing mutations
  if (viewer.mutationObserver) {
    viewer.mutationObserver.disconnect();
  }

  // Clear the container
  viewer.container.innerHTML = '';

  // Remove from map
  viewers.delete(containerId);
}

/**
 * Check if a viewer exists for a given container.
 *
 * @param {string} containerId - The ID of the container.
 * @returns {boolean} True if a viewer exists.
 */
export function hasViewer(containerId) {
  return viewers.has(containerId);
}

/**
 * Get all active viewer IDs.
 *
 * @returns {string[]} Array of container IDs with active viewers.
 */
export function getActiveViewers() {
  return Array.from(viewers.keys());
}

// =========================================================================
// Contrast/Bias/Stretch getters and setters
// =========================================================================

/**
 * Get current contrast value for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number} Contrast value (0.0 to 10.0, default 1.0).
 * @throws {Error} If the viewer is not found.
 */
export function getContrast(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getContrast();
}

/**
 * Set contrast value for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} contrast - Contrast value (0.0 to 10.0).
 * @throws {Error} If the viewer is not found.
 */
export function setContrast(containerId, contrast) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setContrast(contrast);
}

/**
 * Get current bias value for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number} Bias value (0.0 to 1.0, default 0.5).
 * @throws {Error} If the viewer is not found.
 */
export function getBias(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getBias();
}

/**
 * Set bias value for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} bias - Bias value (0.0 to 1.0).
 * @throws {Error} If the viewer is not found.
 */
export function setBias(containerId, bias) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setBias(bias);
}

/**
 * Get current stretch mode for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {string} Stretch mode: "linear", "log", or "symmetric".
 * @throws {Error} If the viewer is not found.
 */
export function getStretchMode(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getStretchMode();
}

/**
 * Set stretch mode for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {string} mode - Stretch mode: "linear", "log", or "symmetric".
 * @throws {Error} If the viewer is not found.
 */
export function setStretchMode(containerId, mode) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setStretchMode(mode);
}

/**
 * Get visible image bounds in pixel coordinates.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number[]} Array [xmin, xmax, ymin, ymax] in pixel coordinates.
 * @throws {Error} If the viewer is not found.
 */
export function getViewBounds(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  // Get viewport dimensions from the container
  const rect = viewer.container.getBoundingClientRect();
  const bounds = viewer.handle.getViewBounds(rect.width, rect.height);
  return Array.from(bounds);
}

/**
 * Set view to show specific image bounds.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} xmin - Minimum x coordinate in pixels.
 * @param {number} xmax - Maximum x coordinate in pixels.
 * @param {number} ymin - Minimum y coordinate in pixels.
 * @param {number} ymax - Maximum y coordinate in pixels.
 * @throws {Error} If the viewer is not found.
 */
export function setViewBounds(containerId, xmin, xmax, ymin, ymax) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  // Get viewport dimensions from the container
  const rect = viewer.container.getBoundingClientRect();
  viewer.handle.setViewBounds(xmin, xmax, ymin, ymax, rect.width, rect.height);
}

/**
 * Get the colormap name for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {string} Colormap name (e.g., "Gray", "Inferno", "Magma", "RdBu").
 * @throws {Error} If the viewer is not found.
 */
export function getColormap(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getColormap();
}

/**
 * Get whether the colormap is reversed.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {boolean} True if the colormap is reversed.
 * @throws {Error} If the viewer is not found.
 */
export function getColormapReversed(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getColormapReversed();
}

/**
 * Get the image value range (vmin, vmax).
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number[]} Array [vmin, vmax].
 * @throws {Error} If the viewer is not found.
 */
export function getValueRange(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  const range = viewer.handle.getValueRange();
  return Array.from(range);
}

/**
 * Set the value range (vmin, vmax) for display scaling.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} vmin - The minimum display value.
 * @param {number} vmax - The maximum display value.
 * @throws {Error} If the viewer is not found.
 */
export function setValueRange(containerId, vmin, vmax) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setValueRange(vmin, vmax);
}

// =========================================================================
// Rotation getters and setters
// =========================================================================

/**
 * Get current rotation angle in degrees (counter-clockwise).
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number} Rotation angle in degrees.
 * @throws {Error} If the viewer is not found.
 */
export function getRotation(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getRotation();
}

/**
 * Set rotation angle in degrees (counter-clockwise).
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} degrees - Rotation angle in degrees.
 * @throws {Error} If the viewer is not found.
 */
export function setRotation(containerId, degrees) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setRotation(degrees);
}

/**
 * Get pivot point in image coordinates.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {number[]} Array [x, y] in image coordinates.
 * @throws {Error} If the viewer is not found.
 */
export function getPivotPoint(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  const pivot = viewer.handle.getPivotPoint();
  return Array.from(pivot);
}

/**
 * Set pivot point in image coordinates.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {number} x - X coordinate in image pixels.
 * @param {number} y - Y coordinate in image pixels.
 * @throws {Error} If the viewer is not found.
 */
export function setPivotPoint(containerId, x, y) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setPivotPoint(x, y);
}

/**
 * Get whether the pivot marker is visible.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @returns {boolean} True if the pivot marker is visible.
 * @throws {Error} If the viewer is not found.
 */
export function getShowPivotMarker(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  return viewer.handle.getShowPivotMarker();
}

/**
 * Set whether to show the pivot marker.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {boolean} show - True to show the pivot marker.
 * @throws {Error} If the viewer is not found.
 */
export function setShowPivotMarker(containerId, show) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.setShowPivotMarker(show);
}

/**
 * Register a callback to be called when the viewer state changes.
 *
 * The callback receives an object with the current state:
 * { contrast, bias, stretchMode, zoom, colormap, colormapReversed, vmin, vmax, xlim, ylim }
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {Function} callback - Callback function to receive state updates.
 * @throws {Error} If the viewer is not found.
 */
export function onStateChange(containerId, callback) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.onStateChange(callback);
}

/**
 * Register a callback to be called when the user clicks in the viewer.
 *
 * The callback receives the click coordinates in data space: { x, y, value }
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @param {Function} callback - Callback function to receive click events.
 * @throws {Error} If the viewer is not found.
 */
export function onClick(containerId, callback) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.onClick(callback);
}

/**
 * Clear all registered callbacks for a viewer.
 *
 * @param {string} containerId - The ID of the container (viewer instance).
 * @throws {Error} If the viewer is not found.
 */
export function clearCallbacks(containerId) {
  const viewer = viewers.get(containerId);
  if (!viewer) {
    throw new Error(`No viewer found for container "${containerId}"`);
  }
  viewer.handle.clearCallbacks();
}

window.viewarr = {
  createViewer,
  setImageData,
  destroyViewer,
  hasViewer,
  getActiveViewers,
  getContrast,
  setContrast,
  getBias,
  setBias,
  getStretchMode,
  setStretchMode,
  getViewBounds,
  setViewBounds,
  getColormap,
  getColormapReversed,
  getValueRange,
  setValueRange,
  getRotation,
  setRotation,
  getPivotPoint,
  setPivotPoint,
  getShowPivotMarker,
  setShowPivotMarker,
  onStateChange,
  onClick,
  clearCallbacks
};

// Default export for convenience
export default {
  createViewer,
  setImageData,
  destroyViewer,
  hasViewer,
  getActiveViewers,
  getContrast,
  setContrast,
  getBias,
  setBias,
  getStretchMode,
  setStretchMode,
  getViewBounds,
  setViewBounds,
  getColormap,
  getColormapReversed,
  getValueRange,
  setValueRange,
  getRotation,
  setRotation,
  getPivotPoint,
  setPivotPoint,
  getShowPivotMarker,
  setShowPivotMarker,
  onStateChange,
  onClick,
  clearCallbacks
};
