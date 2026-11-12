export const run = (target: Readonly<{ a: "x" }>): unknown => target.__proto__;
