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

    // Replace loading indicator with canvas
    container.innerHTML = '';
    container.appendChild(canvas);

    // Create the viewer handle using static factory method
    const handle = await wasmModule.ViewerHandle.create(canvas);

    // Store viewer state
    viewers.set(containerId, {
      handle,
      canvas,
      resizeObserver,
      container
    });

    // Set up MutationObserver to detect container removal (e.g., tab close)
    const mutationObserver = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.removedNodes.forEach((node) => {
          console.debug("removed node", node);
          if (node === container || node.contains(container)) {
            console.debug("contains viewer, destroying");
            destroyViewer(containerId);
            console.debug("canceling observer");
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

  // Stop observing resize
  viewer.resizeObserver.disconnect();

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

window.viewarr = {
  createViewer,
  setImageData,
  notifyResize,
  destroyViewer,
  hasViewer,
  getActiveViewers
};

// Default export for convenience
export default {
  createViewer,
  setImageData,
  notifyResize,
  destroyViewer,
  hasViewer,
  getActiveViewers
};
