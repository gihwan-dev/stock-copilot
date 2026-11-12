export const attempt = (run: () => "a"): "a" | "b" => {
  try {
    return run();
  } catch {
    return "b";
  }
};
