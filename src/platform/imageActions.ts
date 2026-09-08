import { env, platform } from "./index";

/** Copy full-resolution pixels; use the native clipboard where the WebView cannot write images. */
export async function copyImage(src: string): Promise<void> {
  const pixels = (async () => {
    const image = new Image();
    image.src = src;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Canvas unavailable");
    context.drawImage(image, 0, 0);
    return { canvas, context };
  })();
  if (env.isTauri || env.isRemoteWindow) {
    const { invoke } = await import("@tauri-apps/api/core");
    const { canvas, context } = await pixels;
    // The installed plugin accepts JsImage::Rgba without optional PNG decoder features.
    await invoke("plugin:clipboard-manager|write_image", {
      image: {
        rgba: [...context.getImageData(0, 0, canvas.width, canvas.height).data],
        width: canvas.width,
        height: canvas.height,
      },
    });
  } else {
    // Pass a promise immediately so WebKit retains the menu click's clipboard activation.
    const png = pixels.then(({ canvas }) => new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Image encoding failed")), "image/png");
    }));
    await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
  }
}

/** Save original encoded bytes on the client, preserving the source format. */
export async function saveImage(src: string): Promise<void> {
  const blob = await (await fetch(src)).blob();
  const extension = blob.type.split("/")[1]?.replace("jpeg", "jpg").replace("svg+xml", "svg") || "png";
  const filename = `image.${extension}`;
  if (env.isTauri || env.isElectron) {
    const path = await platform.dialog.saveFile({ defaultPath: filename });
    if (path) await platform.transport.invoke("write_bytes_file", {
      path, data: [...new Uint8Array(await blob.arrayBuffer())],
    });
    return;
  }
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
