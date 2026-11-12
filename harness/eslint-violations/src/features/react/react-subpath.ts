import { useMemo } from "react/index.js";
export const useValue = (): unknown => useMemo(() => "a", []);
