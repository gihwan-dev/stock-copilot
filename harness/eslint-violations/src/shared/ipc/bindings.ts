import { z } from "zod";
export const side = (input: string): "buy" => input as "buy";
export const anonymous = z.string().brand();
