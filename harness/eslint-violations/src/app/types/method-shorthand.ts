export type Port = {
  readonly send: (value: "a") => "b";
  receive(value: "a"): "b";
};
