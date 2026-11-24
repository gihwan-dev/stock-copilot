import { useEffect } from "react";

export const useCanvasTheme = (value: string): string => {
  useEffect((): void => undefined, []);
  return value;
};

export const canvasProbe = (): "ok" => "ok";
