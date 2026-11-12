export const attempt = (run: () => "a"): unknown => {
  try {
    return run();
  } catch (error) {
    return error.message;
  }
};
