/**
 * TypeScript type definitions for viewarr
 */

/**
 * JavaScript TypedArray type names supported by viewarr.
 */
export type ArrayType =
  | 'Int8Array'
  | 'Uint8Array'
  | 'Int16Array'
  | 'Uint16Array'
  | 'Int32Array'
  | 'Uint32Array'
  | 'BigInt64Array'
  | 'BigUint64Array'
  | 'Float32Array'
  | 'Float64Array';

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
 * @param arrayType - JavaScript TypedArray type name for interpreting the buffer.
 * @throws If the viewer is not found or data is invalid.
 */
export function setImageData(
  containerId: string,
  buffer: ArrayBuffer,
  width: number,
  height: number,
  arrayType: ArrayType
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
  destroyViewer: typeof destroyViewer;
  hasViewer: typeof hasViewer;
  getActiveViewers: typeof getActiveViewers;
};

export default viewarr;
