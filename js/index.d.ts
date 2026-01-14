/**
 * TypeScript type definitions for viewarr
 */

/**
 * Create a new viewer instance in the specified container.
 *
 * @param containerId - The ID of the HTML element to use as the container.
 *                      This ID is also used to identify the viewer instance.
 * @returns Resolves when the viewer is ready.
 * @throws If the container is not found or initialization fails.
 */
export function createViewer(containerId: string): Promise<void>;

/**
 * Set image data for a viewer.
 *
 * @param containerId - The ID of the container (viewer instance).
 * @param buffer - The raw pixel data.
 * @param width - Image width in pixels.
 * @param height - Image height in pixels.
 * @param dtype - Data type string matching numpy dtype conventions:
 *   - "i1", "b" -> Int8
 *   - "u1", "B" -> Uint8
 *   - "i2" -> Int16
 *   - "u2" -> Uint16
 *   - "i4" -> Int32
 *   - "u4" -> Uint32
 *   - "i8" -> BigInt64
 *   - "u8" -> BigUint64
 *   - "f4" -> Float32
 *   - "f8" -> Float64 (default)
 * @throws If the viewer is not found or data is invalid.
 */
export function setImageData(
  containerId: string,
  buffer: ArrayBuffer,
  width: number,
  height: number,
  dtype: string
): void;

/**
 * Notify a viewer that its container has been resized.
 * This is typically called automatically by ResizeObserver,
 * but can be called manually if needed.
 *
 * @param containerId - The ID of the container (viewer instance).
 * @param width - New width in pixels.
 * @param height - New height in pixels.
 */
export function notifyResize(
  containerId: string,
  width: number,
  height: number
): void;

/**
 * Destroy a viewer instance and clean up resources.
 *
 * @param containerId - The ID of the container (viewer instance).
 */
export function destroyViewer(containerId: string): void;

/**
 * Check if a viewer exists for a given container.
 *
 * @param containerId - The ID of the container.
 * @returns True if a viewer exists.
 */
export function hasViewer(containerId: string): boolean;

/**
 * Get all active viewer IDs.
 *
 * @returns Array of container IDs with active viewers.
 */
export function getActiveViewers(): string[];

declare const viewarr: {
  createViewer: typeof createViewer;
  setImageData: typeof setImageData;
  notifyResize: typeof notifyResize;
  destroyViewer: typeof destroyViewer;
  hasViewer: typeof hasViewer;
  getActiveViewers: typeof getActiveViewers;
};

export default viewarr;
