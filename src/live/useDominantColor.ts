import { useEffect, useState } from "react";

/** Average colour of an 8x8 album-art sample, or null without art. */
export function useDominantColor(url: string | null | undefined): string | null {
  const [color, setColor] = useState<string | null>(null);

  useEffect(() => {
    setColor(null);
    if (!url) return;
    let cancelled = false;
    const image = new Image();
    image.onload = () => {
      if (cancelled) return;
      const canvas = document.createElement("canvas");
      canvas.width = 8;
      canvas.height = 8;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;
      context.drawImage(image, 0, 0, 8, 8);
      const { data } = context.getImageData(0, 0, 8, 8);
      let red = 0;
      let green = 0;
      let blue = 0;
      let count = 0;
      for (let index = 0; index < data.length; index += 4) {
        if (data[index + 3] === 0) continue;
        red += data[index];
        green += data[index + 1];
        blue += data[index + 2];
        count += 1;
      }
      if (count) {
        setColor(
          `rgb(${Math.round(red / count)}, ${Math.round(green / count)}, ${Math.round(blue / count)})`,
        );
      }
    };
    image.onerror = () => {
      if (!cancelled) setColor(null);
    };
    image.src = url;
    return () => {
      cancelled = true;
    };
  }, [url]);

  return color;
}
