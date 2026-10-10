export const AVATAR_SIZE = 128;
export const MAX_AVATAR_DATA_LENGTH = 96 * 1024;
const MAX_INPUT_BYTES = 5 * 1024 * 1024;

export function userAvatarSource(value: string | null | undefined): string | undefined {
  return value && value.length <= MAX_AVATAR_DATA_LENGTH && /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(value)
    ? value : undefined;
}

export function avatarCrop(width: number, height: number) {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0
    || width > 8192 || height > 8192 || width * height > 16 * 1024 * 1024) throw new Error("avatarInvalid");
  const size = Math.min(width, height);
  return { x: (width - size) / 2, y: (height - size) / 2, size };
}

/** Only the resized PNG is persisted; the original never leaves this window. */
export async function prepareUserAvatar(file: File): Promise<string> {
  if (file.size > MAX_INPUT_BYTES) throw new Error("avatarTooLarge");
  if (!file.size || !["image/png", "image/jpeg", "image/webp"].includes(file.type)) throw new Error("avatarInvalid");
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const crop = avatarCrop(image.naturalWidth, image.naturalHeight);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = AVATAR_SIZE;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("avatarInvalid");
    context.drawImage(image, crop.x, crop.y, crop.size, crop.size, 0, 0, AVATAR_SIZE, AVATAR_SIZE);
    const result = canvas.toDataURL("image/png");
    if (!userAvatarSource(result)) throw new Error("avatarInvalid");
    return result;
  } finally {
    URL.revokeObjectURL(url);
  }
}
