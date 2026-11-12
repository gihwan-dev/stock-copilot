import { useMemo } from "react";
export const useValue = (mode: "on" | "off"): unknown =>
  mode === "on" ? useMemo(() => "a", []) : "b";
