export const plain = (value: string): string => value;

import { readDecimalForNumberDisplay } from "./money";
export const readForDisplay = readDecimalForNumberDisplay;
export const forbiddenNumber = Number("1");
export const forbiddenRounding = new Intl.NumberFormat();
