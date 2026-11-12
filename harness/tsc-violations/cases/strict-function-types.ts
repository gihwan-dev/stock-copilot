type Narrow = (input: "a") => "a";
type Wide = (input: "a" | "b") => "a";
const narrow: Narrow = (input) => input;
export const wide: Wide = narrow;
