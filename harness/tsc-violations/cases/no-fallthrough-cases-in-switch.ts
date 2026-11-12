export function route(input: "a" | "b", log: (value: "a" | "b") => void): void {
  switch (input) {
    case "a":
      log("a");
    case "b":
      log("b");
  }
}
